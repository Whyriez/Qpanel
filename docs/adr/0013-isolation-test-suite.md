# ADR-0013: Test Suite Isolasi Cross-Tenant (Cross-Tenant Isolation Suite)

- **Status**: Diterima (Accepted)
- **Tanggal**: 2026-10-03
- **Konteks**: PRD Bagian 5 (5.1, 5.2, 5.3), Bagian 10, Task P2-T7

## Konteks dan Masalah

Prinsip dasar Nusa Panel adalah isolasi ketat antar-tenant di level kernel Linux dan filesystem, tanpa saling memengaruhi (*noisy neighbor* atau eskalasi akses). Sebelum melangkah ke Phase 3 (Site engine, Nginx, PHP, domain), seluruh batas keamanan multi-tenant wajib dibuktikan dengan suite pengujian isolasi terpadu (`tests/isolation/`).

Vektor isolasi kritis yang wajib diuji sesuai PRD:
1. **Cross-tenant read/write/list rejection**:
   - Direktori home setiap tenant dibuat dengan izin `0700` (`rwx------`) milik `p_<slug>:panel-tenants`.
   - ACL POSIX hanya menambahkan traversal web server `u:www-data:--x`.
   - Tenant A (`p_alice`, UID 10001) sama sekali tidak boleh membaca, menulis, ataupun me-list isi direktori tenant B (`p_bob`, UID 10002).
2. **Symlink escape prevention**:
   - Jika tenant A membuat symlink di direktori home yang mengarah ke file tenant B atau file sensitif sistem (`/etc/shadow`), dereferensi ataupun akses via API/file manager wajib ditolak.
3. **`/proc` hidepid process isolation**:
   - Mount `/proc` dengan opsi `hidepid=2` (invisible) menyembunyikan proses milik user lain. Tenant A tidak boleh melihat argumen command line, environment variables, ataupun keberadaan PID tenant B.
4. **Cgroup v2 slice memory confinement & OOM isolation**:
   - Alokasi memori tenant dibatasi oleh `MemoryMax` dan `MemoryHigh` pada slice `panel-u<uid>.slice`.
   - Jika proses tenant melampaui `MemoryMax`, OOM killer kernel terkurung secara lokal di dalam slice tenant tersebut dan tidak boleh mengorbankan proses tenant lain atau daemon sistem.
5. **Cgroup v2 task limit confinement (Fork bomb protection)**:
   - Alokasi proses dibatasi oleh `TasksMax` (cgroup v2 `pids.max`).
   - Upaya serangan fork-bomb oleh tenant A akan gagal dengan `EAGAIN` (`Resource temporarily unavailable`) saat mencapai `TasksMax`, sementara tenant B dan proses sistem tetap dapat membuat proses baru secara normal.
6. **Tenant lifecycle suspension**:
   - Suspend tenant membekukan slice (`cgroup.freeze = 1`), mematikan proses aktif via SIGTERM, mengunci akun di shadow (`usermod -L`), dan mengarahkan shell ke `/usr/sbin/nologin`.

## Keputusan Arsitektur

### 1. Crate Rust `isolation-tests` (`tests/isolation/`)
Dibuat sebagai anggota workspace Cargo:
- `cross_fs.rs`: Mengevaluasi matriks izin POSIX DAC dan ACL, membuktikan bahwa usaha akses read/write/list dari tenant silang menghasilkan `PermissionDenied`.
- `symlink_escape.rs`: Memvalidasi batas jalur (*path confinement*) anti-traversal (`..`) dan pencegahan lolos symlink ke luar root home tenant.
- `proc_hidepid.rs`: Parser opsi mount `/proc` (`hidepid=2` / `hidepid=invisible`) dan verifikasi filter visibilitas proses antar UID.
- `cgroup_confinement.rs`: Validasi generator drop-in systemd slice (`MemoryMax`, `MemoryHigh`, `CPUQuota`, `TasksMax`, `IOWeight`), simulasi penegakan batas tugas (fork limit), dan pengurungan OOM killer.
- `suspend_isolation.rs`: Validasi siklus hidup pembekuan cgroup (`cgroup.freeze`), pembersihan proses, dan penolakan spawn proses saat akun berstatus suspended.

### 2. Skrip Pengujian Kernel Nyata di Dev VM (`tests/isolation/vm_runner.sh`)
Skrip otomasi bash untuk dijalankan langsung di lingkungan Ubuntu 24.04 dev VM sebagai root:
- Membuat dua user sistem Linux nyata (`p_iso_alice` UID 55001 dan `p_iso_bob` UID 55002) dalam grup `panel-tenants`.
- Menerapkan direktori home `0700` dan ACL `www-data`.
- Menjalankan uji baca/tulis/list nyata via `su -s /bin/bash p_iso_alice` terhadap file rahasia milik `p_iso_bob` dan memvalidasi respons `Permission denied`.
- Menguji symlink escape nyata dan verifikasi penolakan akses.
- Memverifikasi isolasi `/proc` di mana proses sleeper milik Bob tidak dapat diinspeksi oleh Alice.
- Memverifikasi penegakan limit proses cgroup v2.
- Melakukan pembersihan menyeluruh (*cleanup trap*) setelah pengujian selesai.

### 3. Integrasi Tooling (`justfile`)
- Ditambahkan perintah `just test-isolation` untuk menjalankan seluruh suite pengujian isolasi secara cepat dan idempoten.

## Konsekuensi

- Seluruh batas keamanan Phase 2 (P2-T1 sampai P2-T7) telah terverifikasi secara komprehensif.
- Memenuhi acceptance criteria Phase 2: isolasi 100% hijau, limit plan terbukti menahan proses, dan suspend menghentikan seluruh service tenant.
- Siap melangkah ke Phase 3 (Site engine, Nginx, PHP, domain).
