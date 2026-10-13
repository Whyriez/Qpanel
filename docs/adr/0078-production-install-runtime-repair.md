# ADR 0078: Perbaikan instalasi dan runtime produksi v0.1.1

Status: diterima untuk implementasi lokal; acceptance Linux belum dijalankan.
Task: P10-T6 (revisi), berdasarkan `FIX_V0.1.1.md`.

## Konteks

Instalasi v0.1.0 memiliki perbedaan identity systemd, database relatif, lifecycle
runtime yang tumpang tindih, proxy panel yang belum dipasang dan baseline GLIBC
yang lebih baru daripada distro target. Pengguna mengizinkan perubahan lokal dan
melarang pengujian langsung di VM pada sesi ini.

## Keputusan

- API berjalan sebagai `panel-api:panel`, di `127.0.0.1:7700`. Default database
  absolut ditetapkan di `panel-core::production` dan dipakai API serta CLI.
  Konfigurasi production menolak bind publik dan database relatif. Unit systemd
  menetapkan environment production; konfigurasi database lokal eksplisit hanya
  untuk development. Kesalahan konfigurasi tidak lagi ditutupi default diam-diam.
  `PANEL_PUBLIC_SCHEME=http` membuat cookie default dapat digunakan pada URL
  fresh install HTTP; konfigurasi eksplisit `auth.cookie_secure` tetap dihormati.
- Hanya agent memiliki `RuntimeDirectory=panel`, mode `0750 root:panel`.
  Agent menetapkan owner/group/mode socket `root:panel 0660` sebelum menerima
  koneksi dan gagal startup bila pengamanan gagal. Peer UID daemon produksi
  berasal dari account `panel-api`; CLI memeriksa RPC menggunakan identity itu.
- Installer/bootstrap memakai PATH sistem deterministik. CLI doctor juga mencari
  utility di direktori sistem, dan memakai `nginx -v` karena `--version` tidak
  didukung Nginx. Probe IPC memiliki timeout dan memvalidasi respons protokol.
- Entry point panel adalah site Nginx milik panel pada `17890`, dapat diubah
  melalui `--port` atau `PANEL_PUBLIC_PORT`. Port tersimpan untuk pemasangan
  ulang. Port `7700` dan port yang digunakan layanan lain ditolak.
- Candidate Nginx diuji melalui salinan konfigurasi utama dengan include
  sites-enabled diarahkan ke staging. Site tenant tetap ikut validasi. Layout
  include global custom yang tidak dikenali ditolak tanpa mengganti nginx.conf.
- Upgrade produksi memakai installer dari paket lengkap sebelum CLI membuka
  pool SQLite. Snapshot mencakup database/state, config panel, unit, proxy dan
  binary lama. Kegagalan mengembalikan artefak serta database panel dan status
  layanan sebelumnya; file tenant dan analytics tidak diganti. Snapshot tetap
  tersedia di `/var/backups/panel/install-*` untuk audit/pemulihan.
- Migrasi/init dan CLI yang memakai database production default dijalankan
  sebagai `panel-api`, termasuk ketika administrator memanggilnya sebagai root.
  Ownership SQLite direkonsiliasi sebelum layanan dijalankan.
- Firewall hanya menerima penambahan port publik di UFW/firewalld aktif.
  nftables custom mendapat peringatan; tidak ada flush atau pembukaan port 7700.
- Release x86_64 dibangun di Ubuntu 22.04; ARM menggunakan image cross 0.2.5
  yang dipin. Kedua binary target wajib lolos pemeriksaan symbol GLIBC <= 2.35
  sebelum dipaketkan. Template Nginx disertakan di paket release.
- Uninstall hanya mencopot site proxy panel; konfigurasi Nginx tenant/default,
  instalasi Nginx dan aturan firewall pengguna dipertahankan.

## Validasi dan batasan

Regression lokal menggunakan mock utility dan render template tanpa menjalankan
agent, API atau layanan host. Pemeriksaan Rust lokal hanya membuktikan target
Windows; blok `cfg(unix)` dan behavior systemd/Nginx sebenarnya membutuhkan Linux.
Harness VM disediakan untuk penggunaan manual di VM disposable dan tidak
dijalankan pada sesi ini. Fresh Debian 12, Ubuntu 22.04, reboot, rollback nyata,
akses LAN serta binary release GLIBC belum dinyatakan lulus.
