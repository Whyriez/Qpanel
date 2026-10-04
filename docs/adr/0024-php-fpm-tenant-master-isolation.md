# ADR-0024: Master PHP-FPM Per-(Tenant, Versi) dan Isolasi Kolam (Pool) Situs

## Status
Accepted

## Konteks
Sesuai PRD Bagian 4.4, 4.5, 5.1, dan Bagian 11 (Task `P4-T3`), model eksekusi PHP pada Nusa Panel dirancang dengan prinsip isolasi multi-tenant yang ketat dan efisiensi memori tingkat tinggi:
1. **Model Tradisional vs Model Nusa Panel**:
   - Model hosting tradisional menjalankan master PHP-FPM global sebagai `root`, dengan pool per-tenant yang diatur via `user = p_tenant`. Kelemahan: kegagalan master PHP berdampak global, dan isolasi cgroup resource limits per tenant tidak dapat diberlakukan secara native per-proses master.
   - Nusa Panel menerapkan arsitektur **Master PHP-FPM per-(tenant, versi)**. Setiap master process berjalan langsung di bawah identitas Linux tenant (`User = p_<slug>`, `Group = p_<slug>`) di dalam cgroup slice tenant (`Slice = panel-u<uid>.slice`).
2. **Sandbox Systemd yang Kuat**:
   - Mengisolasi master FPM tenant agar tidak dapat membaca file tenant lain, tidak dapat memodifikasi sistem root, dan dibatasi hanya pada resource serta path yang diizinkan.
3. **Efisiensi RAM dan Konservasi Idle**:
   - Tenant yang tidak aktif atau situs tanpa traffic tidak boleh membuang alokasi RAM. Pengaturan default kolam adalah `pm = ondemand` dengan `pm.process_idle_timeout = 10s`.
   - Ketika semua kolam situs dalam suatu master dihapus, service master FPM otomatis dimatikan dan dinonaktifkan (`systemctl stop` & `systemctl disable`).
4. **Pencegahan Konfigurasi Rusak (Atomic Staging & Rollback)**:
   - Kesalahan konfigurasi atau sintaks tidak boleh merusak master atau mematikan situs-situs lain milik tenant tersebut.

## Keputusan Arsitektur

1. **Abstraksi Konfigurasi dan Unit Systemd di `panel-runtime`**:
   - `PhpFpmMasterConfig`: Mengelola direktori konfigurasi (`/etc/panel/php/<user>-<ver>/`), path master socket (`/run/panel/php/<user>-<ver>.sock`), nama unit systemd (`panel-fpm@<user>-<ver>.service`), dan slice cgroup (`panel-u<uid>.slice`).
   - Generator unit systemd instantiated:
     - `User = p_<slug>`, `Group = p_<slug>`
     - `Slice = panel-u<uid>.slice`
     - Sandboxing flags: `NoNewPrivileges=true`, `ProtectSystem=strict`, `TemporaryFileSystem=/home:ro`, `BindPaths=/home/p_<slug>`, `ReadWritePaths=/run/panel/php /var/log/panel/p_<slug> /home/p_<slug>/tmp`, `PrivateTmp=true`, `PrivateDevices=true`, `ProtectKernel*`, `ProtectControlGroups=true`, `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`.
     - Multi-tenant socket directory safety: `/run/panel/php` tidak diserahkan ke `RuntimeDirectory=panel/php` pada unit template systemd agar systemd tidak melakukan `chown` direktori bersama ke satu tenant tertentu. Agent membuat dan mengelola izin direktori `/run/panel/php` dengan mode `0755`.
   - `PhpFpmPoolConfig`: Mengelola konfigurasi pool situs (`/etc/panel/php/<user>-<ver>/pool.d/<pool_name>.conf`):
     - `pm = ondemand`, `pm.process_idle_timeout = 10s`, `pm.max_children` disesuaikan dari plan.
     - Unix domain socket: mode `0660`, owner `<user>`, group `www-data`.
     - Hardened defaults: `open_basedir = <docroot>:/home/<user>/tmp:/tmp`, `disable_functions` daftar aman, direktori sesi/tmp per-tenant, slowlog aktif, `/fpm-status` dan `/fpm-ping`.

2. **Protokol IPC di `panel-proto`**:
   - Menambahkan struktur pesan RPC:
     - `PhpFpmMasterEnsureRequest` / `PhpFpmMasterEnsureResponse`
     - `PhpFpmPoolApplyRequest` / `PhpFpmPoolApplyResponse`
     - `PhpFpmPoolRemoveRequest` / `PhpFpmPoolRemoveResponse`
     - `PhpFpmMasterStatusRequest` / `PhpFpmMasterStatusResponse`
     - `PhpFpmMasterReloadRequest` / `PhpFpmMasterRestartRequest` / `PhpFpmMasterActionResponse`
   - Terdaftar pada `RequestPayload` dan `ResponsePayload` lengkap dengan tes serialisasi JSON roundtrip.

3. **Manajemen Lifecycle di `panel-agent`**:
   - Menambahkan whitelist binary `php-fpm7.4` .. `php-fpm8.4` dan `systemd-analyze` ke `ALLOWED_BINARIES`.
   - `ensure_php_fpm_master`: Memastikan unit systemd master terpasang, direktori konfigurasi dan log tenant ada dengan kepemilikan yang tepat (`chown` ke UID tenant), dan `/run/panel/php` memiliki mode aman `0755`.
   - `apply_php_fpm_pool`:
     - Menulis konfigurasi kolam ke `<pool>.conf.staging`.
     - Menjalankan uji sintaks: `php-fpm<ver> -t -y <master_conf>`.
     - Jika sintaks gagal, file staging dihapus seketika dan mengembalikan error tanpa merusak konfigurasi aktif.
     - Jika sintaks valid, memindahkan file staging secara atomik ke path target konfigurasi kolam.
     - Jika service belum berjalan, menjalankan `systemctl enable --now`. Jika sudah berjalan, menjalankan `systemctl reload`.
   - `remove_php_fpm_pool`:
     - Menghapus konfigurasi kolam.
     - Memeriksa sisa kolam yang ada pada direktori `pool.d/`. Jika 0, mengeksekusi `systemctl stop` dan `systemctl disable` untuk menghemat RAM host.
   - `reload_php_fpm_master` & `restart_php_fpm_master`: Melakukan reload (`systemctl reload`) atau restart (`systemctl restart`) master FPM.

4. **REST API & RBAC di `panel-api`**:
   - Endpoint:
     - `GET /api/v1/php/masters`: Menampilkan daftar master tenant (Admin melihat semua; tenant hanya melihat masternya sendiri).
     - `GET /api/v1/php/masters/{username}/{version}/status`: Mengambil status service systemd dan jumlah pool aktif.
     - `POST /api/v1/php/masters/{username}/{version}/reload`: Me-reload master FPM (Memerlukan autentikasi, otorisasi kepemilikan tenant atau admin, dan proteksi CSRF).
     - `POST /api/v1/php/masters/{username}/{version}/restart`: Me-restart master FPM (Memerlukan autentikasi, otorisasi kepemilikan tenant atau admin, dan proteksi CSRF).
   - Seluruh mutasi master FPM mencatat riwayat audit pada `AuditWriter`.

## Konsekuensi
- Isolasi keamanan antar-tenant setara dengan containerisasi ringan berkat systemd sandboxing dan cgroup v2.
- Penggunaan RAM idle minimal berkat `pm = ondemand` dan auto-shutdown master FPM yang tidak memiliki pool aktif.
- Pembaruan konfigurasi pool situs dijamin aman terhadap downtime akibat kesalahan sintaks berkat mekanisme atomic staging dan rollback.
