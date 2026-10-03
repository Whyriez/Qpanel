# ADR-0004: Sistem Penulis dan API Audit Log

- **Status**: Diterima (Accepted)
- **Tanggal**: 2026-10-03
- **Konteks**: PRD Bagian 4.3(8), 4.6, 5.2, Task P1-T6

## Konteks dan Masalah

Sesuai aturan keras arsitektur Nusa Panel (PRD 4.3 butir 8):
> "Setiap command agent menulis audit record (siapa, apa, hasil)."

Dan dalam keamanan panel kontrol hosting (PRD 4.6, 5.2):
- Setiap tindakan autentikasi (login berhasil/gagal, logout, ganti password, aktivasi/deaktivasi 2FA) harus dicatat secara terstruktur untuk audit forensik dan mitigasi brute-force.
- Operasi administratif (pengubahan status akun tenant, pengubahan konfigurasi global server) wajib memiliki jejak audit transparan.
- Penyewa (tenant) berhak melihat histori log audit terkait akunnya sendiri, namun **sama sekali tidak boleh** melihat log audit milik tenant lain atau aktivitas global sistem server (isolasi multi-tenant).
- Penulisan log audit di API tidak boleh memblokir atau menggagalkan alur transaksi pengguna jika penulisan log mengalami kegagalan sekunder (non-fatal async logging).

## Keputusan Arsitektur

### 1. Repositori Audit Log Dinamis & Aman (`panel-db`)
- Diperluas dengan filter kueri terstruktur `AuditLogFilter`:
  - `user_id: Option<i64>`
  - `actor_username: Option<String>`
  - `action: Option<String>`
  - `target_resource: Option<String>`
  - `status: Option<String>` ('success' | 'failure')
  - `from_time: Option<i64>`, `to_time: Option<i64>`
- Memanfaatkan `sqlx::QueryBuilder::<Sqlite>` dengan parameter binding aman (`push_bind`). Tidak ada interpolasi string SQL sesuai aturan `AGENTS.md` nomor 4.
- Menyediakan metode:
  - `find_by_id(&self, id: i64) -> Result<Option<AuditLog>, DbError>`
  - `list_filtered(&self, filter: &AuditLogFilter, limit: i64, offset: i64) -> Result<Vec<AuditLog>, DbError>`
  - `count_filtered(&self, filter: &AuditLogFilter) -> Result<i64, DbError>`

### 2. Pencatatan Audit di Agent Root (`panel-agent`)
- Daemon agent root menerima permintaan RPC bertipe (`Ping`, `Version`, dan lifecycle operasi sistem).
- Modul `panel_agent::audit` mendefinisikan `AgentAuditRecord` dan fungsi `record_audit`.
- Setiap permintaan IPC yang diproses dicatat ke tracing terstruktur (`target: "panel_agent::audit"`) dengan aktor, nama aksi, resource target, status eksekusi (`success` atau `error`), durasi/request_id, memenuhi aturan PRD 4.3(8).

### 3. Penulis Audit API (`AuditWriter` di `panel-api`)
- Struct `AuditWriter` membungkus `SqliteAuditLogRepository`:
  - `record`: Penulisan sinkron (awaiting database).
  - `record_async` & `log_async`: Penulisan asinkron via `tokio::spawn` agar latensi respons klien tetap optimal dan kegagalan pencatatan audit log tidak merusak respons fungsional pengguna.
- Helper `extract_client_ip`: Mengekstraksi IP klien secara aman dari header `X-Forwarded-For` atau `X-Real-IP`.

### 4. Integrasi Penulisan Audit pada Operasi Kunci
- Autentikasi (`crates/panel-api/src/auth/handlers.rs`):
  - Login gagal (user tidak ditemukan, password salah, akun terkunci, 2FA salah) dicatat dengan status `failure` beserta alamat IP klien dan alasan kegagalan.
  - Login berhasil dicatat dengan status `success`, `user_id`, dan IP klien.
  - Logout dicatat dengan status `success`.
  - Pergantian password dicatat (gagal bila password lama salah, dan berhasil).
  - Pengaktifan dan penonaktifan 2FA TOTP dicatat.
- Operasi Admin (`crates/panel-api/src/admin/handlers.rs`):
  - Perubahan status akun tenant (`user.status_update`).
  - Perubahan konfigurasi sistem (`setting.update`).

### 5. Rute HTTP API & Otorisasi RBAC
- Rute yang disediakan:
  - `GET /api/v1/audit/logs`: Kueri daftar audit log dengan filter (`action`, `actor`, `status`, `target`, rentang tanggal) dan paginasi (`page`, `per_page`).
  - `GET /api/v1/audit/logs/{id}`: Detail log audit berdasarkan ID.
- Penegakan Kebijakan RBAC:
  - Administrator dapat melihat semua log sistem atau memfilter berdasarkan `user_id` mana pun.
  - Tenant biasa (`Role::User`) otomatis di-scope hanya pada log miliknya (`user_id = Some(auth_user.id)`).
  - Percobaan tenant biasa untuk meminta log tenant lain baik via filter `?user_id=...` maupun via `GET /api/v1/audit/logs/{id}` ditolak dengan `403 Forbidden` (`Action::ViewAuditLogs { target_user_id }`).
  - Rute tanpa autentikasi ditolak dengan `401 Unauthorized`.
