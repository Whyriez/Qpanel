# ADR 0042: Audit Trail Sesi Terminal & Kebijakan Privasi Perekaman Keystroke

## Status

Accepted

## Konteks

Sesuai spesifikasi PRD bagian 7 dan requirement Phase 6 (P6-T4):
> *"Audit: catat awal/akhir sesi (tenant, folder, durasi, IP). Perekaman keystroke tidak aktif secara default."*

Akses terminal interaktif web memberikan kemampuan eksekusi perintah langsung di dalam lingkungan terisolasi tenant (`p_<user>`). Untuk keperluan tata kelola keamanan, akuntabilitas sistem, dan kepatuhan audit operasional, sistem harus:
1. **Mencatat Siklus Hidup Sesi**: Mencatat secara persisten momen pembukaan dan penutupan setiap sesi terminal, mencakup identitas aktor, tenant terasosiasi, direktori kerja awal (`cwd`/folder), alamat IP klien pemanggil, PID shell proses, dan durasi sesi dalam detik.
2. **Menjamin Keamanan & Privasi Pengguna (Keystroke Recording Guardrail)**: Sesuai prinsip *least-privilege* dan perlindungan privasi, perekaman masukan keyboard mentah (*keystroke logging*) berisiko membocorkan data sensitif pengguna (seperti kata sandi, token API, kredensial SSH, atau konten berkas rahasia). Oleh karena itu, perekaman keystroke dinonaktifkan secara bawaan (*disabled by default*), dan stream input biner hanya dialirkan secara *in-memory* langsung ke descriptor PTY host tanpa pernah disimpan ke basis data atau disk log.
3. **Pencegahan Emisi Audit Ganda (*Duplicate Emission Guard*)**: Penutupan sesi PTY dapat dipicu oleh beberapa kejadian konkuren (misalnya: penutupan tab/REST API manual, shell process exit normal, atau kegagalan koneksi streaming agent). Audit penutupan sesi harus dicatat tepat satu kali (*exactly-once*).
4. **Isolasi RBAC pada Riwayat Audit**: Tenant biasa hanya boleh melihat log audit terminal miliknya sendiri, sementara administrator panel dapat memantau riwayat audit seluruh tenant atau memfilter riwayat per tenant.

## Keputusan Arsitektur

### 1. Model Data Audit & Metadata Sesi di `TerminalHub`

Struktur `ActiveTerminalSession` diperkaya dengan metadata kontekstual:
- `user_id: i64`: ID pengguna panel yang membuka sesi.
- `username: String`: Nama akun pengguna panel (contoh: `alice`).
- `linux_username: String`: Identitas tenant Linux terisolasi (contoh: `p_alice`).
- `cwd: String`: Direktori kerja awal saat sesi terminal diinisialisasi.
- `client_ip: Arc<RwLock<Option<String>>>`: IP address klien yang terhubung (diekstraksi dari HTTP/WebSocket header `X-Forwarded-For`, `X-Real-IP`, atau socket peer).
- `closed_audit_emitted: Arc<AtomicBool>`: Flag atomik untuk menjamin deduplikasi audit penutupan.

### 2. Pencatatan Awal Sesi (`terminal.session_start`)

Saat `POST /api/v1/terminal/sessions` dipanggil:
- Header permintaan diperiksa untuk mengekstrak alamat IP klien melalui utilitas `crate::audit::extract_client_ip(&headers)`.
- Sesi PTY baru di-spawn di agent host, dan instance `TerminalHub` menyimpan metadata sesi.
- AuditWriter memancarkan entri audit terstruktur:
  - `action`: `"terminal.session_start"`
  - `target_id`: UUID `session_id`
  - `actor_id`: `user_id` & `username`
  - `client_ip`: Alamat IP klien
  - `details_json`:
    ```json
    {
      "session_id": "...",
      "tenant_user_id": 12,
      "tenant": "p_alice",
      "folder": "sites/example.com",
      "cols": 80,
      "rows": 24,
      "pid": 12345,
      "keystroke_recording": false
    }
    ```

### 3. Pencatatan Akhir Sesi (`terminal.session_end`) & Kalkulasi Durasi

Ketika sesi ditutup (melalui `DELETE /api/v1/terminal/sessions/{id}` atau keluarnya proses PTY di loop pembaca background):
- Fungsi pembantu `TerminalHub::emit_session_end_audit(...)` dieksekusi.
- Menggunakan `closed_audit_emitted.swap(true, Ordering::SeqCst)`. Jika nilai sebelumnya sudah `true`, pemancaran diabaikan sehingga mencegah duplikasi log antara API handler dan background loop.
- Durasi sesi dihitung secara deterministik:
  $$\text{duration\_secs} = \max(0, \text{now\_timestamp} - \text{created\_at\_timestamp})$$
- AuditWriter memancarkan entri audit terstruktur:
  - `action`: `"terminal.session_end"`
  - `target_id`: UUID `session_id`
  - `details_json`:
    ```json
    {
      "session_id": "...",
      "tenant_user_id": 12,
      "tenant": "p_alice",
      "folder": "sites/example.com",
      "duration_secs": 42,
      "exit_code": 0,
      "reason": "manual_close" // atau "process_exit" / "agent_read_error"
    }
    ```

### 4. Kebijakan Privasi Keystroke (Zero Keystroke Leakage)

- Frame biner input yang dikirimkan klien melalui WebSocket (`/ws/terminal/{id}`) diteruskan langsung melalui saluran memory IPC gRPC/Unix socket ke file descriptor master PTY host agent.
- Tidak ada modul perantara yang melakukan penyalinan data input ke basis data, tabel audit, maupun berkas log teks di disk.
- Semua entri audit terminal secara eksplisit menyatakan flag `"keystroke_recording": false`.

### 5. API Endpoint Riwayat Audit Terminal (`GET /api/v1/terminal/audit`)

- Disediakan endpoint terdedikasi `GET /api/v1/terminal/audit` dengan parameter kueri `page`, `per_page`, dan `tenant`.
- Membaca rekaman audit dengan filter action `terminal.%` dari database PostgreSQL/SQLite.
- Melakukan transformasi field `details_json` menjadi model DTO terstruktur `TerminalAuditItem`:
  - `id`: ID audit log
  - `session_id`: UUID sesi
  - `actor_username`: Pengguna pembuka sesi
  - `action`: `"terminal.session_start"` atau `"terminal.session_end"`
  - `tenant`: Identitas tenant terisolasi (`linux_username`)
  - `folder`: Direktori kerja (`cwd`)
  - `duration_secs`: Durasi aktual sesi (pada event `session_end`)
  - `ip_address`: Alamat IP klien
  - `status`: Status eksekusi
  - `keystroke_recording`: Boolean penanda privasi keystroke
  - `created_at`: Waktu timestamp kejadian
- **Isolasi RBAC**:
  - Pengguna dengan role `Tenant` dibatasi secara ketat hanya dapat melihat log miliknya sendiri (filter otomatis berdasarkan `auth_user.id`).
  - Pengguna dengan role `Admin` dapat melihat seluruh log audit atau memfilter berdasarkan username tenant tertentu.

## Konsekuensi

### Positif
- Memenuhi seluruh kriteria acceptance P6-T4 dan PRD bagian 7 secara penuh dan teruji melalui integration tests.
- Rekaman audit lengkap (tenant, folder, durasi, IP, start/end) memberikan visibilitas dan akuntabilitas kepatuhan keamanan yang komprehensif.
- Deduplikasi atomik (`swap CAS guard`) menjamin tidak ada log penutupan ganda meskipun terjadi race condition antara penutupan REST API dan terminasi proses shell.
- Privasi pengguna terjaga penuh tanpa ada kebocoran keystroke masukan sensitif ke basis data.
- Teruji 100% pada integration test suite `terminal_audit_test.rs` dan zero warning pada `cargo clippy`.

### Negatif / Mitigasi
- Rekaman audit detail memerlukan penyimpanan tambahan di tabel `audit_logs`. Mitigasi: Tabel audit sudah dirancang dengan pengindeksan waktu (`created_at`) dan mekanisme rotasi/pembersihan berkala yang didukung oleh sistem database panel.
