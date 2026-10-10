# ADR 0047: Node.js Site Sandboxing, Port Allocation, and Process Lifecycle Management

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.1 dan Spesifikasi Fase 7 (P7-T2 "Site Node"), Nusa Panel harus mendukung hosting aplikasi Node.js mandiri secara terisolasi, aman, dan berkinerja tinggi:
1. **Direktori Kerja Aplikasi (`app_dir`)**: Aplikasi berjalan dari direktori tenant yang ditentukan (`/home/<user>/sites/<domain>/app` atau custom path), bukan direktori global.
2. **Isolasi Proses dan Resource**: Setiap situs Node.js harus memiliki systemd unit sendiri (`site-<id>.service`) yang berjalan di bawah UID tenant dan terikat ke tenant cgroup slice (`panel-u<uid>.slice`).
3. **Alokasi Port Dinamis & Unik**: Port aplikasi dialokasikan secara otomatis dalam rentang `20000..=29999`, dijamin unik di seluruh basis data situs (`sites.settings_json`), dan hanya terikat pada loopback (`127.0.0.1`).
4. **Sandboxing Ketat Sesuai PRD 5.1**: Unit systemd harus menerapkan direktif pengerasan Linux: `ProtectSystem=strict`, `ProtectHome=read-only` (dengan write allowance untuk app directory), `PrivateTmp=true`, `NoNewPrivileges=true`, `CapabilityBoundingSet=`, `LimitNOFILE=65535`, serta `Restart=on-failure` (dengan `RestartSec=2`).
5. **Reverse Proxy Nginx & Proteksi SSRF**: Nginx mengarahkan traffic HTTP/HTTPS ke `http://127.0.0.1:<port>` dengan dukungan upgrade WebSocket (`$http_upgrade` dan `$connection_upgrade`). Template memvalidasi upstream URL anti-SSRF agar tidak bisa diarahkan ke jaringan privat eksternal atau alamat berbahaya.
6. **Pemeriksaan Kesehatan Lokal (Health Probe)**: Agent melakukan probe HTTP lokal ke `http://127.0.0.1:<port>/` untuk membedakan proses yang aktif (running) tetapi macet (unhealthy) vs aktif dan sehat (healthy).
7. **Siklus Hidup Proses & Eksekusi Perintah**: Mendukung aksi proses `start`, `stop`, `restart`, `redeploy` (eksekusi npm install lalu restart), serta eksekusi job tenant (`npm install`, `npm run build`, custom scripts) yang dibatasi waktu dan kuota slice.

## Keputusan Arsitektur

### 1. Generator Unit Systemd Sandboxed (`panel-agent::node_site`)
- Berkas unit dibuat di `/etc/systemd/system/site-<id>.service` dengan izin `0644`.
- Menjalankan binary Node.js terisolasi dari `/opt/panel/runtimes/node/<versi>/bin/node` atau npm shim.
- Direktif keamanan PRD 5.1:
  - `User=<username>`, `Group=<username>`, `Slice=panel-u<uid>.slice`
  - `WorkingDirectory=<app_dir>`
  - `EnvironmentFile=-/etc/panel/env/site-<id>.env` (berkas env permissions `0600` milik tenant)
  - `ProtectSystem=strict`, `ProtectHome=read-only`, `ReadWritePaths=<app_dir>`
  - `PrivateTmp=true`, `NoNewPrivileges=true`, `CapabilityBoundingSet=`
  - `LimitNOFILE=65535`, `Restart=on-failure`, `RestartSec=2`
- Mendukung entrypoint bertipe:
  - `NodeScript`: `node <path>`
  - `NpmStart`: `npm start`
  - `NpmCustom`: `npm run <subcmd>`

### 2. Alokasi Port Loopback Unik (`20000..=29999`)
- Modul `crates/panel-api/src/node_site.rs` menyediakan fungsi `allocate_node_site_port`:
  - Mengambil seluruh port yang tercatat di kolom `sites.settings_json` pada database SQLite.
  - Menguji pengikatan socket TCP lokal (`TcpListener::bind("127.0.0.1:<port>")`) untuk memastikan port tidak sedang digunakan oleh proses lain di host.
  - Menghasilkan port unprivileged pertama yang bebas dalam rentang `20000..=29999`.

### 3. Konfigurasi Vhost Nginx & WebSocket Upgrade (`panel-webserver`)
- Template `NODEJS_VHOST_TEMPLATE` menyalurkan request melalui reverse proxy ke upstream URL.
- Baseline Nginx menyertakan `/etc/panel/nginx/conf.d/01-websocket.conf` yang mendefinisikan:
  ```nginx
  map $http_upgrade $connection_upgrade {
      default upgrade;
      ''      close;
  }
  ```
- Blok lokasi proxy menyertakan header WebSocket:
  ```nginx
  proxy_http_version 1.1;
  proxy_set_header Upgrade $http_upgrade;
  proxy_set_header Connection $connection_upgrade;
  proxy_set_header Host $host;
  proxy_set_header X-Real-IP $remote_addr;
  proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
  proxy_set_header X-Forwarded-Proto $scheme;
  ```
- Validasi upstream URL menerapkan SSRF guard ketat: hanya mengizinkan `http://127.0.0.1:<port>`, `http://localhost:<port>`, atau unix socket `unix:...`.

### 4. Siklus Hidup Proses dan Health Check Probe
- Endpoint REST API:
  - `GET /api/v1/sites/{id}/node/status`: Membaca status service dari `systemctl show` (ActiveState, SubState, MainPID, MemoryCurrent, CPUUsageNSec) dan melakukan TCP socket HTTP GET probe ke port aplikasi.
  - `POST /api/v1/sites/{id}/node/action`: Menangani aksi `start`, `stop`, `restart`, `redeploy`. Pada aksi `redeploy`, agent menjalankan instalasi paket (`npm install`) terlebih dahulu sebelum merestart unit.
  - `POST /api/v1/sites/{id}/node/job`: Menjalankan perintah tenant (`npm install`, `npm run build`, script kustom) di bawah UID tenant dan cgroup slice menggunakan `systemd-run` dengan batas timeout.
  - `GET /api/v1/sites/{id}/node/settings`: Mengambil konfigurasi port, app directory, entrypoint, auto-restart, dan environment variables.
  - `PUT /api/v1/sites/{id}/node/settings`: Memperbarui pengaturan, menyimpan ke database, dan otomatis merekompilasi vhost serta me-reload konfigurasi.

### 5. Antarmuka Web (UI) & Aksesibilitas
- `SiteWizardDialog`:
  - Opsi tipe situs "Node.js Application" dibuka dan diaktifkan.
  - Pengguna dapat memilih versi Node.js yang terpasang dan menentukan file entrypoint.
- `SiteDetailPage`:
  - Tab baru `[Pengaturan Node.js]` muncul otomatis bila situs bertipe `nodejs`.
  - Komponen `NodeStatusCard` menampilkan metrik real-time: Nama unit, Status aktif, Indikator Sehat/Sakit, Port terikat, PID, Pemakaian RAM, Waktu CPU, dan Uptime.
  - Tombol aksi cepat: Mulai, Hentikan, Muat Ulang, dan Deploy Ulang.
  - Runner perintah tenant dengan terminal output box untuk melihat exit code, durasi, stdout, dan stderr.
  - Komponen `NodeSettingsCard` untuk mengubah versi Node, port aplikasi, direktori kerja, entrypoint, dan tabel key-value variabel lingkungan yang tersimpan aman.

## Konsekuensi
- **Positif**:
  - Aplikasi Node.js berjalan dengan isolasi kernel cgroup slice dan sandboxing ketat, mencegah eksfiltrasi berkas sistem host atau privilege escalation.
  - Dukungan WebSocket bawaan memungkinkan aplikasi interaktif modern (Socket.IO, Next.js, Nuxt, Express WS) berjalan tanpa konfigurasi manual.
  - Pengembang memiliki visibilitas penuh atas status proses dan kesehatan aplikasi langsung dari dashboard tanpa harus SSH ke server.
  - Semua aksi proses dan eksekusi skrip dicatat ke audit log sistem untuk akuntabilitas.
- **Batasan**:
  - Aplikasi Node.js yang mendengarkan port non-loopback akan ditolak oleh template demi keamanan multi-tenant. Aplikasi wajib menggunakan port yang diberikan oleh environment variable `PORT` atau berkas environment panel.
