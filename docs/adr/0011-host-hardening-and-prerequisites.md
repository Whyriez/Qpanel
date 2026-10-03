# ADR-0011: Hardening Host Linux dan Validasi Prasyarat Sistem

- **Status**: Diterima (Accepted)
- **Tanggal**: 2026-10-03
- **Konteks**: PRD Bagian 4.4, 5.2, 6, Task P2-T5

## Konteks dan Masalah

Panel hosting Nusa Panel mengelola banyak tenant di satu host Linux. Untuk mencegah eskalasi hak akses, kebocoran informasi antar-tenant (misalnya melihat variabel lingkungan, command line argumen, atau proses tenant lain di `/proc`), dan menjamin lingkungan hosting memenuhi seluruh kebutuhan isolasi kernel, host harus melalui proses pengecekan prasyarat diagnostik dan tindakan hardening otomatis.

Berdasarkan PRD:
- **P2-T5**: "Hardening host: opsi `hidepid=2` untuk `/proc`, pembuatan grup `panel-tenants`, pengecekan prasyarat (cgroup v2, ACL, quota) dengan pesan jelas bila tidak terpenuhi."
- **Keamanan**: Operasi hardening dan diagnostik sistem dijalankan oleh daemon berhak akses tinggi (`panel-agent`), sementara `panel-api` beroperasi sebagai unprivileged daemon yang memanggil agent lewat Unix Domain Socket.

Kebutuhan arsitektur:
1. **Opsi `hidepid=2` pada `/proc`**:
   - Memastikan mount `/proc` memiliki opsi `hidepid=2` atau `hidepid=invisible`, sehingga setiap user biasa (tenant) hanya bisa melihat proses miliknya sendiri.
   - Menyediakan kemampuan remount otomatis via agent saat administrator mengaktifkannya (`mount -o remount,hidepid=2 /proc`).
2. **Grup Sistem `panel-tenants`**:
   - Memastikan grup sistem `panel-tenants` tersedia di host (dan dibuat otomatis via `groupadd panel-tenants` jika belum ada).
3. **Pengecekan Prasyarat Menyeluruh (Prerequisites Diagnostic)**:
   - **cgroup v2**: Memastikan controller `cpu`, `memory`, `io`, dan `pids` tersedia dan aktif pada unified hierarchy `/sys/fs/cgroup`.
   - **POSIX ACL**: Memeriksa dukungan ACL pada filesystem tenant (`getfacl /`).
   - **Disk Quota**: Mendeteksi kuota kernel ext4/xfs aktif atau kesiapan fallback scanner direktori rekursif.
   - Pesan diagnostik ramah admin dan informatif dalam bahasa Indonesia dengan petunjuk solusi jika ada prasyarat yang belum terpenuhi.

## Keputusan Arsitektur

### 1. Protokol Bertipe (`panel-proto`)
Penambahan tipe request dan response RPC di `panel-proto`:
- `RequestPayload::HardeningCheck` -> `ResponsePayload::HardeningStatus(Box<HardeningStatus>)`
- `RequestPayload::HardeningApply(HardeningApplyRequest)` -> `ResponsePayload::HardeningApplied(HardeningApplyResponse)`

Struktur `HardeningStatus`:
- `cgroup_v2: PrerequisiteCheckItem`
- `acl: PrerequisiteCheckItem`
- `quota: PrerequisiteCheckItem`
- `hidepid: PrerequisiteCheckItem`
- `tenant_group: PrerequisiteCheckItem`
- `overall_ok: bool`

`HardeningStatus` dibungkus dalam `Box<HardeningStatus>` pada enum `ResponsePayload` untuk mencegah perbedaan ukuran varian enum yang berlebihan (`clippy::large_enum_variant`).

### 2. Modul `HardeningManager` pada `panel-agent`
Dibuat di `crates/panel-agent/src/hardening.rs`:
- Menggunakan `SystemRunner` trait dan path yang dapat dikonfigurasi (`cgroup_root`, `proc_mounts`, `group_file`) untuk isolasi pengujian tanpa modifikasi host.
- Menambahkan biner yang diizinkan (`"mount"`, `"getfacl"`, `"getent"`) ke `ALLOWED_BINARIES` di `crates/panel-agent/src/system.rs`.
- `apply(&self, req: HardeningApplyRequest)`:
  - Membuat grup `panel-tenants` bila belum ada.
  - Me-remount `/proc` dengan opsi `hidepid=2` bila belum aktif.
  - Setiap operasi mutasi dicatat ke audit log agent (`audit::AgentAuditRecord`).

### 3. Integrasi `AgentClient` pada `panel-api`
- Menambahkan metode `check_hardening()` dan `apply_hardening(req)`.
- Pengujian integration test `tests/hardening_ipc_test.rs` memverifikasi seluruh alur: deteksi prasyarat awal yang belum terpenuhi -> eksekusi apply hardening -> validasi pencatatan perintah sistem -> verifikasi status post-hardening yang menjadi 100% compliant (`overall_ok: true`).

## Konsekuensi

- Administrator mendapatkan visibilitas instan atas kesiapan host untuk multi-tenancy yang aman.
- Isolasi `/proc` dan grup `panel-tenants` terstandarisasi.
- Menjadi fondasi kokoh untuk suite isolasi (P2-T7).
