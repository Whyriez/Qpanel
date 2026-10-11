# ADR 0070: Pengoptimal Server dan Profil Tuning Performa Host

## Konteks
Panel hosting berjalan di beragam lingkungan perangkat keras virtual dan fisik, mulai dari VPS bersumber daya kecil (1 vCPU, 512MB RAM) hingga server bare-metal / dedicated berkapasitas besar (> 16 vCPU, > 32GB RAM). Konfigurasi default layanan inti (Nginx, Kernel sysctl, PHP-FPM, dan runtime aplikasi Node.js/Python) seringkali tidak optimal:
- Pada server kecil, alokasi memori berlebihan dapat memicu Out-Of-Memory (OOM Killer) dan mematikan layanan penting.
- Pada server besar, limit bawaan (seperti `somaxconn = 128`, worker connection rendah, file descriptor terbatas) menciptakan bottleneck throughput dan koneksi tertolak (*connection drops*).

PRD Bab 11 (Task P10-T2) mensyaratkan:
1. Profil tuning otomatis berdasarkan spesifikasi server host:
   - `Micro` (< 2 GB RAM): Profil hemat memori dan konservatif untuk VPS kecil.
   - `Standard` (2 - 4 GB RAM): Profil seimbang untuk beban hosting web umum.
   - `HighPerformance` (4 - 16 GB RAM): Profil agresif untuk lalu lintas tinggi dan throughput maksimal.
   - `DedicatedLarge` (> 16 GB RAM): Profil enterprise untuk bare-metal dengan concurrency masif.
2. Parameter tuning lintas layer:
   - Nginx: `worker_processes`, `worker_connections`, `worker_rlimit_nofile`, `client_body_buffer_size`, `fastcgi_buffers`, `open_file_cache`.
   - Sysctl kernel: `net.core.somaxconn`, `net.ipv4.tcp_max_syn_backlog`, `net.ipv4.tcp_tw_reuse`, `net.ipv4.ip_local_port_range`, `vm.swappiness`, `fs.file-max`, `net.core.netdev_max_backlog`.
   - PHP-FPM default pool: `pm`, `max_children`, `process_idle_timeout`, `max_requests`.
   - Runtime hints: jumlah worker cluster Node.js, batas memori V8 (`--max-old-space-size`), worker Gunicorn/Uvicorn Python.
3. Apply atomik dengan validasi sintaks:
   - Menulis konfigurasi ke `/etc/sysctl.d/99-panel-tuning.conf` dan `/etc/panel/nginx/conf.d/tuning.conf`.
   - Menjalankan uji sintaks `nginx -t`. Jika gagal, otomatis melakukan rollback seketika ke snapshot sebelumnya tanpa downtime.
4. Rollback manual dan snapshot backup:
   - Mencatat cadangan file dan metadata profil sebelumnya di `/var/lib/panel/optimizer/backup/`.
   - Memungkinkan admin mengembalikan konfigurasi kapan saja.
5. REST API & Web UI:
   - REST API dengan otorisasi admin (`ROLE_ADMIN`) dan audit logging asinkron.
   - Kartu Web UI informatif dengan KPI hardware, rekomendasi otomatis, tab visual konfigurasi, dan modal konfirmasi.

## Keputusan Arsitektur

### 1. Model Komunikasi & Protokol RPC (`panel-proto`)
- Menambahkan tipe `OptimizerProfileType` (`Micro`, `Standard`, `HighPerformance`, `DedicatedLarge`, `Custom`).
- Menambahkan struct konfigurasi tuning: `ServerHardwareSpecs`, `NginxTuningConfig`, `SysctlTuningConfig`, `PhpFpmTuningConfig`, `AppRuntimeHints`, `OptimizerProfile`.
- Menambahkan request dan response RPC: `OptimizerGetStatusRequest`, `OptimizerStatusResponse`, `OptimizerApplyRequest`, `OptimizerApplyResponse`, `OptimizerRollbackRequest`, `OptimizerRollbackResponse`.
- Memperluas enum payload `RequestPayload` dan `ResponsePayload` di `panel-proto`.

### 2. Modul Engine Agent (`panel-agent::optimizer`)
- Dibuat `OptimizerManager`:
  - `detect_hardware()`: Membaca informasi CPU (`/proc/cpuinfo`), memori RAM dan Swap (`/proc/meminfo`), versi kernel (`/proc/version`), dan distro sistem operasi (`/etc/os-release`).
  - `recommend_profile_type()`: Logika pemilihan tingkatan profil berbasis RAM dan vCPU.
  - `generate_profile_for_type()`: Generator profil deterministik dengan nilai-nilai optimal sesuai tier hardware.
  - `render_sysctl_conf()` & `render_nginx_tuning_conf()`: Pembangkit file konfigurasi teks.
  - `take_backup_snapshot()`: Pencadangan berkas sysctl dan Nginx ke `/var/lib/panel/optimizer/backup/` lengkap dengan `meta.json`.
  - `apply()`: Menulis konfigurasi, memvalidasi dengan `nginx -t`, memicu rollback otomatis jika sintaks salah, menerapkan sysctl via `sysctl --system`, me-reload Nginx via `systemctl reload nginx` (fallback `nginx -s reload`), serta menyimpan riwayat state di `state.json`.
  - `rollback()`: Mengembalikan file konfigurasi sysctl dan Nginx dari snapshot cadangan dan me-reload layanan.
- Menambahkan `"sysctl"` ke daftar `ALLOWED_BINARIES` di `crates/panel-agent/src/system.rs`.

### 3. REST API & Audit Logging (`panel-api::optimizer`)
- Dibuat modul `crates/panel-api/src/optimizer.rs` yang mengimplementasikan OpenAPI DTOs:
  - `ServerHardwareSpecsDto`, `NginxTuningConfigDto`, `SysctlTuningConfigDto`, `PhpFpmTuningConfigDto`, `AppRuntimeHintsDto`, `OptimizerProfileDto`, `OptimizerStatusResponseDto`, `ApplyOptimizerRequestDto`, `OptimizerActionResponseDto`.
- Handlers dengan otorisasi berbasis peran (Admin-only):
  - `GET /api/v1/system/optimizer`: Mengembalikan spesifikasi hardware host, profil aktif, rekomendasi sistem, dan perbandingan konfigurasi.
  - `POST /api/v1/system/optimizer/apply`: Menerapkan profil tuning yang dipilih.
  - `POST /api/v1/system/optimizer/rollback`: Mengembalikan konfigurasi ke snapshot cadangan terakhir.
- Audit Logging: Setiap tindakan apply dan rollback dicatat ke sistem audit log panel (`action = "optimizer.apply"`, `target = profile_name`).

### 4. Antarmuka Pengguna Frontend
- DTO types terintegrasi di `web/src/types/api.ts` dan fungsi HTTP API di `web/src/api/client.ts`.
- Dibuat komponen `web/src/components/system/ServerOptimizerCard.tsx`:
  - 4 Kartu KPI Hardware: vCPU cores & model, RAM total & Swap, OS Distro & Kernel release, Status profil tuning aktif.
  - Banner Rekomendasi Cerdas: Menjelaskan profil yang disarankan dan alasan teknisnya berdasarkan kapasitas RAM/vCPU.
  - Pemilih Profil Kartu Interaktif: Pilihan kartu visual untuk 4 tingkatan profil dengan label "Aktif" dan "Disarankan".
  - Tab Rincian Parameter Profil: Menampilkan spesifikasi teknis untuk Nginx, Kernel sysctl, PHP-FPM, dan runtime hints (Node.js/Python).
  - Modal Konfirmasi Protektif: Konfirmasi dialog sebelum menerapkan tuning dan pemulihan cadangan.
- Dibuat halaman `web/src/pages/SettingsPage.tsx` dan dipasang ke navigasi `'settings'` di `web/src/App.tsx`.

## Konsekuensi

### Positif
- Server secara mandiri mengonfigurasi batas resource kernel dan web server optimal tanpa konfigurasi manual yang rawan kesalahan.
- Proteksi rollback otomatis seketika bila `nginx -t` gagal memastikan ketiadaan downtime akibat salah konfigurasi.
- Kemampuan rollback manual dari snapshot backup memberikan kepastian operasional bagi administrator.
- Seluruh tindakan tuning tercatat secara rapi di audit trail panel.
- UI responsif, elegan, dan informatif memenuhi standar WCAG 2.2 AA.

### Netral / Pertimbangan
- Pada lingkungan pengujian virtual atau container non-root, eksekusi perintah `sysctl --system` ditangani secara halus (graceful warning) agar tidak memutus pipeline.
