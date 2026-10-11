# ADR 0074: Idempotent Installer, Automated Upgrader dengan Rollback, dan Multi-Target Release Pipeline

## Status
Diterima

## Konteks
Nusa Panel dirancang untuk berjalan pada VPS dan server dedicated Linux Ubuntu 22.04 LTS, Ubuntu 24.04 LTS, dan Debian 12. Agar panel siap digunakan di lingkungan produksi (*production-ready*), sistem memerlukan proses instalasi, pembaruan, dan pelepasan yang deterministik, aman, dan tanpa downtime yang tidak terencana:
1. **Instalasi Awal**: Administrator server memerlukan skrip instalasi tunggal (`install.sh`) yang idempoten (dapat dijalankan berulang kali tanpa merusak state yang ada), memverifikasi kompatibilitas kernel cgroup v2, memasang paket dependensi, mengonfigurasi user sistem non-root (`panel-api`) dan unit systemd, menyetel baseline Nginx, mengonfigurasi firewall, serta mencetak token *first-run* sekali pakai.
2. **Pembaruan Sistem (Upgrades)**: Pembaruan versi panel (`panelctl upgrade`) harus menjamin integritas data dengan membuat snapshot keselamatan otomatis (*pre-upgrade safety snapshot*) sebelum menjalankan migrasi skema SQLite. Bila migrasi atau uji integritas pasca-migrasi (`PRAGMA quick_check`) gagal, sistem wajib melakukan pemulihan otomatis (*automatic rollback*) ke kondisi semula.
3. **Pencopotan Pemasangan (Uninstall)**: Diperlukan prosedur pencopotan bersih (`uninstall.sh` dan `panelctl uninstall`) yang menghentikan dan menghapus layanan systemd, menghapus biner, dan memberikan opsi untuk mempertahankan basis data (`--keep-data`) atau membersihkan seluruh data (`--purge`).
4. **Pipeline Rilis Multi-Arsitektur**: Paket rilis harus mendukung arsitektur server umum (`x86_64-unknown-linux-gnu` dan `aarch64-unknown-linux-gnu`), memaketkan biner release, unit systemd, aset statis antarmuka web, serta dokumen checksum SHA-256 (`SHA256SUMS`).

## Keputusan Arsitektur

### 1. Skrip Penginstal Idempoten (`install.sh`)
- Menjalankan 8 tahap instalasi deterministik:
  1. **Pemeriksaan Hak Akses**: Memverifikasi eksekusi dengan `id -u == 0` (root/sudo).
  2. **Validasi OS**: Membaca `/etc/os-release` untuk memastikan distro yang didukung (Ubuntu 22.04/24.04, Debian 12), dengan opsi override `--force-os`.
  3. **Verifikasi cgroup v2 & Kernel**: Memeriksa `/sys/fs/cgroup/cgroup.controllers` untuk memastikan kontrol hierarki CPU/memori aktif.
  4. **Instalasi Paket**: Menginstal dependensi esensial (`nginx`, `nftables`, `curl`, `tar`, `ca-certificates`, `sqlite3`, `systemd`, `acl`, `util-linux`) via `apt-get --no-install-recommends`.
  5. **Pengguna & Direktori Sistem**: Membuat grup `panel`, user sistem tanpa login `panel-api` (`/usr/sbin/nologin`), dan direktori `/etc/panel/nginx`, `/var/lib/panel`, `/var/backups/panel`, `/var/log/panel`, `/run/panel` dengan permission ketat.
  6. **Konfigurasi Baseline Nginx**: Memasang `/etc/panel/nginx/baseline.conf` dan include `/etc/nginx/conf.d/panel.conf` dengan pengujian `nginx -t`.
  7. **Pemasangan Biner & Unit Systemd**: Menyalin biner ke `/usr/local/bin/` (mode `0755`) dan unit service (`panel-agent.service` root dan `panel-api.service` user `panel-api`) ke `/etc/systemd/system/`, lalu mengaktifkan via `systemctl enable --now`.
  8. **Inisialisasi Database & Token First-Run**: Menginisialisasi basis data SQLite dan menghasilkan token bootstrap sekali pakai (`panelctl setup-token generate`) yang kedaluwarsa setelah pendaftaran administrator pertama.
  9. **Panduan & Konfigurasi Firewall**: Mendeteksi UFW/nftables dan membuka port 80, 443, dan 3000 (atau port panel kustom via `--port`).

### 2. Automated Upgrader dengan Rollback (`panelctl upgrade`)
- Sub-perintah `panelctl upgrade` diimplementasikan di `crates/panel-cli/src/commands/upgrade.rs`.
- Alur kerja pembaruan:
  - **Simulasi (Dry Run)**: Mendukung `--dry-run` untuk memverifikasi kesiapan basis data tanpa mengubah berkas atau data.
  - **Snapshot Keselamatan**: Mengambil snapshot `.tar.gz` database dan konfigurasi sebelum pembaruan (`create_system_backup`). Jika snapshot gagal, pembaruan dibatalkan demi keamanan.
  - **Migrasi Skema**: Menjalankan migrasi database (`run_migrations`).
  - **Verifikasi Integritas**: Memvalidasi database pasca-migrasi dengan `PRAGMA quick_check(1)`.
  - **Automatic Rollback**: Jika migrasi gagal atau database korup, sistem otomatis memulihkan data dari snapshot keselamatan (`restore_system_backup`).
  - **Penggantian Biner Atomik**: Jika direktori biner baru disediakan (`--binary-dir`), biner diganti secara atomik melalui penulisan temporary file dan rename (`rename`).
  - **Restart Layanan**: Me-restart layanan systemd `panel-agent` dan `panel-api` pasca pembaruan.

### 3. Skrip dan Perintah Pencopotan (`uninstall.sh` dan `panelctl uninstall`)
- Menghentikan dan menonaktifkan unit systemd `panel-api.service` dan `panel-agent.service`.
- Menghapus unit file dari `/etc/systemd/system/` dan menjalankan `systemctl daemon-reload`.
- Menghapus biner dari `/usr/local/bin/` dan soket runtime `/run/panel`.
- Opsi retensi data:
  - Secara default (`--keep-data`), direktori data `/var/lib/panel` dan cadangan `/var/backups/panel` dipertahankan untuk mencegah kehilangan data tidak sengaja.
  - Dengan flag `--purge`, seluruh direktori data, cadangan, konfigurasi, user `panel-api`, dan grup `panel` dihapus bersih dari sistem host.

### 4. Release Pipeline & Multi-Target Packaging
- Skrip pengemasan lokal/CI: `scripts/build-release.sh`.
- Pipeline GitHub Actions: `.github/workflows/release.yml` yang terpicu pada tag git `v*`.
- Target kompilasi:
  - `x86_64-unknown-linux-gnu` (Intel/AMD)
  - `aarch64-unknown-linux-gnu` (ARM64 / Graviton / Ampere)
- Setiap paket rilis mencakup:
  - Biner rilis teroptimasi: `panel-agent`, `panel-api`, `panelctl`
  - Aset statis antarmuka web hasil build Vite/React
  - Berkas unit systemd hardened
  - Skrip penginstal `install.sh` dan pencopot `uninstall.sh`
  - Berkas checksum terpadu `SHA256SUMS`

## Konsekuensi
- **Positif**:
  - Instalasi panel di server Ubuntu/Debian baru dapat diselesaikan secara otomatis dalam waktu < 5 menit.
  - Administrator terlindungi dari kegagalan migrasi dan korupsi data berkat mekanisme automatic rollback.
  - Pembaruan versi dapat dijalankan tanpa manual query SQL atau resiko inkonsistensi skema.
  - Dukungan native multi-arsitektur x86_64 dan ARM64 memperluas fleksibilitas deployment VPS murah.
- **Batasan**:
  - Pada sistem selain Ubuntu 22.04/24.04 dan Debian 12, installer mensyaratkan flag `--force-os` karena perbedaan penamaan paket dan konfigurasi systemd/ACL.
