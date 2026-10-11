# Panduan Administrator Nusa Panel

Selamat datang di Panduan Administrator Nusa Panel. Dokumen ini menjelaskan tata cara instalasi, konfigurasi, pemeliharaan, dan pengoperasian server web hosting Nusa Panel di lingkungan produksi.

---

## 1. Gambaran Umum Arsitektur

Nusa Panel dirancang dengan prinsip **keamanan tingkat tinggi**, **kinerja maksimal**, dan **penggunaan memori minimal** (RAM target < 65 MB saat idle). Sistem terdiri dari beberapa komponen utama:

```
[ Klien Web / Peramban ]
        │  HTTPS (Port 443 / 3000)
        ▼
[ Nginx Reverse Proxy ] ───► [ panel-api (Non-Root, Port 3000) ]
                                    │
                                    │  UNIX Domain Socket (/run/panel/agent.sock)
                                    ▼
                             [ panel-agent (Root Daemon Privileged) ]
                                    │
                                    ├─── systemd (Unit, Timer, Slice cgroup v2)
                                    ├─── Nginx vhost generator (/etc/panel/nginx)
                                    ├─── nftables (Loopback isolation filter)
                                    └─── Linux Users & ACL (p_<tenant>, UID 1000+)
```

- **`panel-api`**: Service API HTTP (Axum) yang berjalan di bawah pengguna sistem non-root `panel-api`. Bertanggung jawab atas autentikasi, otorisasi RBAC, penjadwalan job latar belakang, dan penyajian antarmuka web.
- **`panel-agent`**: Daemon pengelola sistem yang berjalan sebagai root. **Hanya menerima perintah RPC bertipe** melalui soket UNIX domain terisolasi (`/run/panel/agent.sock`). Tidak pernah menerima perintah shell bebas.
- **`panelctl`**: Utilitas command-line (CLI) untuk administrator host (bootstrap, diagnosa dokter, rekonsiliasi, snapshot backup, dan upgrade).

---

## 2. Persyaratan Sistem & Instalasi

### 2.1 Persyaratan Minimum
- **Sistem Operasi**: Ubuntu 24.04 LTS (Direkomendasikan), Ubuntu 22.04 LTS, atau Debian 12.
- **Arsitektur CPU**: x86_64 atau aarch64 (ARM64).
- **RAM**: Minimal 1 GB (Rekomendasi: 2 GB+ untuk multi-tenant).
- **Penyimpanan**: Minimal 10 GB ruang disk bebas.
- **Kernel**: cgroup v2 controller aktif (`/sys/fs/cgroup/cgroup.controllers`).

### 2.2 Instalasi Otomatis via `install.sh`
Jalankan penginstal idempoten sebagai root:
```bash
# 1. Unduh atau clone repositori
git clone https://github.com/nusa-panel/panel.git /opt/panel
cd /opt/panel

# 2. Jalankan skrip instalasi
sudo ./install.sh
```

Opsi instalasi tambahan:
- `--port <PORT>`: Mengubah port web Nusa Panel (default: 3000).
- `--unattended`: Menjalankan instalasi tanpa konfirmasi interaktif.
- `--force-os`: Mengabaikan verifikasi ketat distro jika menguji OS non-standar.

### 2.3 Bootstrap Akun Administrator Pertama
Setelah instalasi selesai, skrip mencetak **First-Run Setup Token**:
```
Alamat Web Panel : http://<SERVER_IP>:3000
Token Pengaturan : abcdef0123456789...
```
1. Buka URL tersebut di peramban web Anda.
2. Masukkan Token Pengaturan.
3. Buat nama pengguna administrator dan kata sandi yang kuat (minimal 8 karakter).
4. Aktifkan Autentikasi Dua Faktor (TOTP 2FA) dan simpan kode pemulihan darurat.

Alternatif pembuatan admin melalui CLI:
```bash
panelctl admin create --username admin --email admin@example.com
```

---

## 3. Manajemen Paket Hosting (Plans)

Paket hosting menentukan batas kuota dan sumber daya untuk tenant:
- **Disk Quota**: Batas kapasitas penyimpanan direktori situs dan berkas (ext4 project quota / xfs).
- **Max Databases**: Batas jumlah basis data MariaDB/PostgreSQL yang dapat dibuat tenant.
- **Max Processes**: Batas jumlah proses bersamaan pada cgroup slice tenant (`TasksMax`).
- **Memory Limit**: Batas alokasi memori RAM per-tenant (`MemoryMax` pada `panel-u<uid>.slice`).
- **CPU Weight**: Bobot prioritas CPU relatif (`CPUWeight` 1–10000).
- **Max Cron Jobs**: Batas jumlah jadwal tugas cron otomatis per situs.

Pengaturan paket dapat dikelola melalui menu **Pengaturan > Paket Hosting** di antarmuka web.

---

## 4. Manajemen Runtime dan Biner

Nusa Panel mendukung isolasi multi-versi untuk tiga mesin runtime utama:

### 4.1 PHP (PHP-FPM)
- Versi yang didukung: PHP 8.1, PHP 8.2, PHP 8.3, dan PHP 8.4 (Ondřej Surý PPA).
- Setiap tenant memiliki master FPM dan pool terisolasi di bawah UID tenant sendiri.
- Pengaturan `php.ini` terkurasi: `memory_limit`, `upload_max_filesize`, `max_execution_time`, `opcache`.
- Ekstensi dapat dipasang dan diaktifkan langsung melalui UI: `mysqli`, `pgsql`, `redis`, `gd`, `intl`, `curl`, `mbstring`, `zip`, `bcmath`.

### 4.2 Node.js
- Mendukung multi-versi Node.js (Node 18 LTS, Node 20 LTS, Node 22 LTS).
- Manajemen paket: `npm`, `yarn`, `pnpm` (via Corepack).
- Mode aplikasi: Express, Next.js Standalone SSR, dan generic background processes.

### 4.3 Python
- Dikelola menggunakan toolchain modern `uv` di `/opt/panel/runtimes/python/`.
- Versi yang didukung: Python 3.10, 3.11, 3.12, 3.13.
- Isolasi virtualenv otomatis (`.venv`) di direktori home tenant.
- Preset WSGI/ASGI: Django, FastAPI (Uvicorn), Flask, dan Generic.

---

## 5. Manajemen Server Basis Data

Nusa Panel mendukung dua mesin basis data relasional:
- **MariaDB** (port default: 3306)
- **PostgreSQL** (port default: 5432)

Fitur untuk Administrator:
- **Status Layanan**: Memantau status service, uptime, dan versi aktif.
- **Kontrol Layanan**: Tombol satu-klik untuk Mulai, Hentikan, dan Restart layanan database via systemd.
- **Instalasi On-Demand**: Bila mesin database belum terpasang, admin dapat memicunya langsung dari menu **Runtime > Basis Data**.

---

## 6. Keandalan & Pemeliharaan Sistem

### 6.1 Diagnostik Kesehatan: `panelctl doctor`
Jalankan diagnosa mandiri untuk memeriksa 8 kategori kesehatan sistem:
```bash
panelctl doctor
```
Untuk memperbaiki isu yang terdeteksi secara otomatis:
```bash
panelctl doctor --fix
```
Aksi perbaikan otomatis mencakup pembuatan direktori kerja yang hilang, restart Nginx yang down, perbaikan WAL database SQLite, dan pemulihan pekerjaan yang macet (*interrupted jobs*).

### 6.2 Penyelarasan Host: `panelctl reconcile`
Mendeteksi ketidaksinkronan (*drift*) antara data di SQLite dan konfigurasi fisik host:
```bash
# Uji tanpa mengubah berkas (simulasi):
panelctl reconcile --dry-run

# Terapkan perbaikan sinkronisasi langsung:
panelctl reconcile
```

### 6.3 Cadangan & Snapshot Sistem: `panelctl backup`
Membuat snapshot komprehensif yang mencakup basis data panel (`panel.db`), konfigurasi global (`panel.toml`), dan seluruh virtual host Nginx:
```bash
# 1. Buat cadangan baru
panelctl backup create

# 2. Lihat daftar cadangan yang ada
panelctl backup list

# 3. Verifikasi integritas dan checksum SHA-256 berkas arsip
panelctl backup verify --path /var/backups/panel/panel-backup-xxxx.tar.gz

# 4. Pulihkan dari cadangan (otomatis membuat safety snapshot sebelum menimpa)
panelctl backup restore --path /var/backups/panel/panel-backup-xxxx.tar.gz
```

### 6.4 Pembaruan Panel: `panelctl upgrade`
Memperbarui versi panel secara aman dengan migrasi database otomatis dan proteksi rollback:
```bash
# Simulasi pembaruan:
panelctl upgrade --dry-run

# Jalankan pembaruan nyata dari bundel rilis baru:
panelctl upgrade --binary-dir /tmp/nusa-release/bin
```
Bila migrasi database gagal atau integritas file korup, sistem secara otomatis melakukan rollback ke kondisi sebelum pembaruan.

### 6.5 Validasi Profil RAM: `panelctl memory-profile`
Memvalidasi 6 target penggunaan memori RAM sesuai Bagian 9 PRD:
```bash
panelctl memory-profile
# atau ekspor laporan markdown resmi:
panelctl memory-profile --export-report docs/benchmarks/memory-profile-report.md
```

---

## 7. Optimasi Kinerja Server

Nusa Panel menyediakan modul penalaan otomatis (*Server Optimizer*) yang menyesuaikan konfigurasi dengan spesifikasi perangkat keras server:
- **Profil Micro** (< 2 GB RAM): Penalaan agresif hemat memori, worker minimal, buffer kompak.
- **Profil Standard** (2–4 GB RAM): Keseimbangan ideal untuk VPS umum (2 CPU, 2-4 GB RAM).
- **Profil HighPerformance** (4–16 GB RAM): Caching lebih besar, socket queue tinggi, FastCGI buffer optimal.
- **Profil DedicatedLarge** (> 16 GB RAM): Skalabilitas penuh untuk server dedicated lalu lintas tinggi.

Parameter yang dioptimalkan meliputi:
- `worker_processes`, `worker_connections`, `open_file_cache` Nginx
- Kernel sysctl: `somaxconn`, `tcp_max_syn_backlog`, `tcp_tw_reuse`, `swappiness`
- Pola worker pool default PHP-FPM (`pm.max_children`, `process_idle_timeout`)

Dapat diterapkan melalui tab **Pengaturan > Optimizer Server** di Web UI atau via API `/api/v1/system/optimizer/apply`.

---

## 8. Observabilitas & Analitik Keamanan

### 8.1 Insight Engine (Deteksi Anomali)
Mesin analitik otomatis mendeteksi 8 kondisi anomali tanpa konfigurasi manual:
- Lonjakan error HTTP 5xx (`five_xx_spike`)
- Regresi latensi setelah rilis Git deployment (`deploy_latency_regression`)
- Lonjakan lalu lintas bot/crawler (`bot_spike`)
- Lonjakan lalu lintas ekstrem (`traffic_spike`)
- URL/Endpoint yang mengalami perlambatan rata-rata > 1 detik (`slow_endpoint`)
- Rasio cache hit rendah (`low_cache_hit`)
- Batas maksimal proses anak worker PHP tercapai (`fpm_max_children_reached`)
- Peringatan kuota kapasitas penyimpanan disk mendekati batas (`disk_quota_warning`)

### 8.2 Analitik Keamanan & Pemblokiran IP dengan TTL
- **Deteksi Probing Path**: Mengidentifikasi pemindaian jalur sensitif seperti `wp-login.php`, `xmlrpc.php`, `.env`, `phpmyadmin`.
- **Deteksi Brute Force**: Mengidentifikasi lonjakan request gagal pada endpoint login.
- **Top Threat IPs**: Memberikan skor risiko (*Threat Score*) untuk IP yang mencurigakan.
- **Blokir IP dengan TTL**: Memungkinkan pemblokiran IP langsung dari UI dengan durasi fleksibel (1 jam, 6 jam, 24 jam, 7 hari, atau permanen). Nginx disinkronkan secara atomik via `deny <ip>;`.

---

## 9. Prosedur Pencopotan Pemasangan (Uninstall)

Bila Anda ingin mencopot instalasi Nusa Panel dari server:
```bash
# Opsi 1: Pertahankan basis data dan berkas cadangan (default)
sudo ./uninstall.sh

# Opsi 2: Bersihkan seluruh data, konfigurasi, dan pengguna sistem secara total
sudo ./uninstall.sh --purge -y
```
Alternatif via CLI:
```bash
panelctl uninstall --purge --yes
```
