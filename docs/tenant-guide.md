# Panduan Pengguna & Tenant Nusa Panel

Selamat datang di Panduan Pengguna (Tenant) Nusa Panel. Dokumen ini dirancang untuk pemilik situs web, pengembang aplikasi, dan webmaster dalam mengelola aplikasi web, domain, sertifikat SSL, basis data, dan tugas otomatis di Nusa Panel.

---

## 1. Memulai & Masuk ke Nusa Panel

### 1.1 Masuk Pertama Kali & Keamanan Akun
1. Buka peramban dan akses alamat server Nusa Panel Anda (misalnya `http://panel.domainanda.com:17890`).
2. Masukkan **Username** dan **Password** yang diberikan oleh Administrator sistem.
3. **Autentikasi Dua Faktor (TOTP 2FA)**:
   - Sangat disarankan untuk langsung mengaktifkan 2FA di menu **Profil Pengguna** > **Keamanan**.
   - Pindai kode QR menggunakan aplikasi autentikator (Google Authenticator, Aegis, atau 1Password), lalu masukkan kode 6-digit untuk verifikasi.
   - Simpan kode pemulihan (*backup codes*) darurat di tempat yang aman.

### 1.2 Dashboard Ringkasan
Dashboard utama menampilkan ringkasan penggunaan sumber daya akun Anda:
- **CPU & RAM Usage**: Penggunaan komputasi secara real-time yang diisolasi dalam cgroup tenant Anda.
- **Disk Quota**: Kapasitas penyimpanan terpakai berbanding batas kuota yang dialokasikan paket hosting Anda.
- **Daftar Situs Aktif**: Status kesehatan situs (*Healthy*, *Stopped*, *Error*).
- **Notifikasi Sistem**: Pemberitahuan perpanjangan SSL, deployment Git, atau peringatan batas kuota.

---

## 2. Manajemen Situs Web (Sites)

Nusa Panel mendukung berbagai tipe runtime modern dengan isolasi pengguna Linux independen (`p_<username>`).

### 2.1 Membuat Situs Baru
1. Masuk ke menu **Situs Web** (**Sites**), lalu klik tombol **+ Buat Situs Baru**.
2. Masukkan **Nama Domain** utama (contoh: `app.perusahaan.com`).
3. Pilih **Tipe Runtime**:
   - **Static Site**: Untuk situs HTML/CSS/JS statis, landing page, atau hasil build JAMstack (Astro, Vite, Hugo).
   - **PHP-FPM**: Untuk CMS (WordPress, Laravel, Drupal) dengan pilihan versi PHP (PHP 8.1, 8.2, 8.3).
   - **Node.js**: Untuk aplikasi backend Express, NestJS, atau fullstack SSR seperti Next.js dan Nuxt.js.
   - **Python**: Untuk aplikasi WSGI/ASGI seperti Django, FastAPI, atau Flask (dikelola via Gunicorn/Uvicorn).
   - **Reverse Proxy**: Untuk meneruskan trafik ke kontainer lokal atau layanan internal yang sudah berjalan di port tertentu (contoh: `http://127.0.0.1:8080`).
4. Tentukan **Root Direktori Dokumen** (default: `public` untuk Laravel/Node.js, atau `/` untuk WordPress).
5. Klik **Simpan & Terapkan**. Panel akan secara otomatis mengonfigurasi virtual host Nginx, alokasi direktori, dan isolasi proses.

### 2.2 Mengelola Runtime Node.js & Python
Untuk runtime berbasis aplikasi proses:
- **Startup Script**: Tentukan entrypoint aplikasi (contoh: `server.js`, `index.js`, atau `app:app` untuk ASGI).
- **Environment Variables**: Tambahkan variabel lingkungan (misalnya `NODE_ENV=production`, `DATABASE_URL=...`). Variabel ini disimpan secara terenkripsi di vault Nusa Panel.
- **Process Manager**: Nusa Panel mengelola siklus hidup proses secara otomatis melalui unit systemd tenant dengan kebijakan restart otomatis jika terjadi kegagalan (*crash*).

### 2.3 Kontrol Status Situs
Di halaman detail situs, Anda memiliki kontrol langsung:
- **Restart**: Memuat ulang proses backend dan mengosongkan cache opcode.
- **Stop / Start**: Menonaktifkan sementara situs tanpa menghapus data.
- **Maintenance Mode**: Menampilkan halaman pemeliharaan ramah pengunjung (HTTP 503) dengan opsi mengecualikan IP tertentu (misalnya IP kantor Anda).

---

## 3. Domain & Sertifikat SSL/TLS

### 3.1 Menghubungkan Domain
1. Pastikan Anda telah mengarahkan DNS record domain Anda ke alamat IP server Nusa Panel:
   - **A Record**: `app.domainanda.com` -> `IP.Server.Anda`
   - **AAAA Record** (opsional): Jika server mendukung IPv6.
2. Tunggu propagasi DNS (biasanya 1 hingga 15 menit).

### 3.2 Sertifikat Otomatis Let's Encrypt
Nusa Panel terintegrasi langsung dengan Let's Encrypt untuk sertifikat SSL gratis:
1. Pada detail situs, buka tab **SSL / TLS**.
2. Pilih opsi **Let's Encrypt (Otomatis)**.
3. Klik tombol **Terbitkan Sertifikat**.
4. Panel akan memvalidasi kepemilikan domain via *ACME HTTP-01 challenge*, mengunduh sertifikat, dan memasangnya ke konfigurasi Nginx.
5. **Perpanjangan Otomatis**: Sertifikat akan diperpanjang secara otomatis 30 hari sebelum masa berlaku habis.

### 3.3 Custom SSL Certificate
Jika Anda memiliki sertifikat Commercial SSL (seperti Wildcard atau EV):
1. Pilih opsi **Custom Certificate**.
2. Masukkan isi sertifikat publik (`.crt` / `.pem`) termasuk rantai CA (*CA Bundle / Intermediate*).
3. Masukkan Private Key (`.key`) rahasia Anda.
4. Klik **Terapkan Sertifikat**.

### 3.4 Pengalihan HTTPS & HSTS
- **Force HTTPS**: Aktifkan sakelar ini agar seluruh trafik HTTP (port 80) diarahkan secara permanen (HTTP 301) ke HTTPS (port 443).
- **HSTS (HTTP Strict Transport Security)**: Tambahkan proteksi browser untuk mewajibkan koneksi terenkripsi.

---

## 4. Web File Manager

Web File Manager memungkinkan Anda mengelola file situs secara langsung dari peramban tanpa memerlukan klien FTP pihak ketiga.

### 4.1 Navigasi & Operasi File
- **Jalur Akses**: Seluruh file tersimpan di `/home/p_<username>/sites/<site_name>/`.
- **Unggah & Unduh**: Drag-and-drop file atau folder secara langsung ke area peramban.
- **Ekstraksi Arsip**: Unggah arsip `.zip`, `.tar.gz`, lalu klik kanan > **Ekstrak** langsung di server.
- **Kompresi**: Pilih file/folder yang ingin diarsipkan untuk diunduh sebagai satu file zip.
- **Editor Berkas Terpadu**: Buka berkas kode (HTML, PHP, JS, JSON, .env) untuk mengedit secara langsung dengan fitur penomoran baris dan penyorotan sintaks (*syntax highlighting*).

### 4.2 Izin Akses Berkas (Permissions)
- Berkas web standar harus memiliki hak akses `0644` (file) dan `0755` (direktori).
- Anda dapat mengubah hak akses (*chmod*) langsung melalui menu konteks file.
- Kepemilikan berkas (*chown*) selalu dikunci ke user tenant Anda untuk mencegah eskalasi hak akses.

---

## 5. Akses SSH & Web Terminal

### 5.1 Menambahkan Kunci Publik SSH (SSH Public Key)
Demi keamanan maksimal, autentikasi SSH berbasis kata sandi dinonaktifkan secara bawaan:
1. Masuk ke **Pengaturan Akun** > **Kunci SSH**.
2. Klik **+ Tambah Kunci SSH**.
3. Tempel kunci publik Anda (format `ssh-ed25519 AAAAC3...` atau `ssh-rsa AAAAB3...`).
4. Beri label pengenal (misalnya *Laptop MacBook Pro*).
5. Klik **Simpan**. Kunci akan otomatis ditambahkan ke `~/.ssh/authorized_keys` akun Anda.
6. Anda sekarang dapat terhubung melalui terminal lokal:
   ```bash
   ssh -p 22 p_<username>@ip-server-anda
   ```

### 5.2 Web Terminal Terintegrasi
Nusa Panel menyediakan terminal berbasis web interaktif:
1. Buka menu **Terminal** di navigasi samping.
2. Terminal akan langsung membuka sesi shell Bash di bawah hak akses user tenant Anda.
3. Anda dapat menjalankan perintah utilitas seperti `git`, `composer`, `npm`, `node`, `python3`, `wp-cli`, dll.
4. Sesi dilindungi oleh isolasi cgroup v2; Anda tidak dapat melihat proses tenant lain (*hidepid=2*).

---

## 6. Manajemen Basis Data (Databases)

Nusa Panel mendukung basis data relasional **MariaDB / MySQL** dan **PostgreSQL**.

### 6.1 Membuat Database Baru
1. Buka menu **Basis Data** (**Databases**).
2. Klik **+ Buat Database**.
3. Pilih tipe mesin: **MariaDB** atau **PostgreSQL**.
4. Masukkan nama database (otomatis diberi prefix tenant, contoh: `p_user_blogdb`).
5. Buat **User Database** baru atau pilih user yang sudah ada.
6. Masukkan atau generate kata sandi acak yang kuat.
7. Klik **Buat Database**.

### 6.2 Mengelola Akses & User
- Satu pengguna basis data dapat diberikan hak akses ke beberapa database.
- Hak akses default mencakup `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`, `DROP`, `INDEX`, `ALTER`.
- Host koneksi database dibatasi ke `127.0.0.1` atau `localhost` untuk keamanan maksimum.

### 6.3 Backup & Restore Database
- **Ekspor / Dump**: Klik tombol **Ekspor** pada database untuk mengunduh salinan dump SQL (.sql.gz) secara instan.
- **Impor**: Unggah file `.sql` untuk memulihkan skema dan data tabel.
- **Alat Web**: Akses phpMyAdmin atau Adminer langsung dari tautan pintas aman yang disediakan di panel jika diaktifkan oleh Administrator.

---

## 7. Penjadwal Tugas (Cron Jobs)

Jalankan perintah berkala untuk sinkronisasi data, backup berkala, pembersihan cache, atau cron CMS.

### 7.1 Menambahkan Cron Job
1. Masuk ke menu **Cron Jobs** > **+ Tambah Tugas Baru**.
2. Pilih jadwal:
   - **Pilihan Cepat**: Setiap Menit (`* * * * *`), Setiap Jam (`0 * * * *`), Setiap Hari pukul 00:00 (`0 0 * * *`), atau Mingguan.
   - **Kustom**: Masukkan format ekspresi cron 5-kolom standar (`menit jam tgl bln hari`).
3. Masukkan **Perintah**:
   - Contoh Laravel Scheduler:
     ```bash
     /usr/bin/php8.2 /home/p_user/sites/app/artisan schedule:run >> /dev/null 2>&1
     ```
   - Contoh WordPress Cron:
     ```bash
     /usr/bin/php8.2 /home/p_user/sites/blog/wp-cron.php
     ```
4. Klik **Simpan**.

### 7.2 Pemantauan Riwayat Eksekusi
- Panel mencatat riwayat eksekusi cron terbaru, durasi waktu berjalan, serta kode status keluar (*exit code*).
- Jika perintah gagal (exit code non-zero), pesan error akan terekam di tab riwayat log cron.

---

## 8. Site Rules (Aturan Pengalihan & Keamanan Nginx)

Anda dapat menyesuaikan perilaku web server per situs tanpa harus mengedit file konfigurasi Nginx mentah.

### 8.1 Redirect Rules (Pengalihan URL)
- Buat aturan pengalihan URL dengan tipe **301 (Permanent)** atau **302 (Temporary)**.
- Contoh: Arahkan `/lama-promo` ke `/promo-terbaru`.

### 8.2 Custom HTTP Headers
- Tambahkan header respons kustom seperti:
  - `Access-Control-Allow-Origin: *` (CORS)
  - `X-Frame-Options: SAMEORIGIN`
  - `X-Content-Type-Options: nosniff`

### 8.3 Pembatasan Akses IP (IP Whitelist / Blacklist)
- Lindungi direktori sensitif (misalnya `/admin` atau `/wp-login.php`).
- Atur kebijakan: **Hanya Izinkan IP Tertentu** (*Allowlist*) atau **Blokir IP Tertentu** (*Denylist*).

### 8.4 Rate Limiting Tingkat Situs
- Batasi jumlah permintaan per detik untuk mencegah serangan brute-force atau scraping agresif (misalnya maksimal 20 req/detik per IP klien).

---

## 9. Git Deployment Otomatis & Webhook

Otomatiskan alur kerja rilis dari repositori Git (GitHub, GitLab, Gitea, atau Bitbucket).

### 9.1 Konfigurasi Git Deployment
1. Di halaman detail situs, buka tab **Git Deploy**.
2. Masukkan **Repository URL** (contoh: `git@github.com:organisasi/aplikasi.git` atau URL HTTPS).
3. Tentukan **Branch** target (contoh: `main` atau `production`).
4. Jika repositori bersifat privat, tambahkan Deploy Key publik yang dihasilkan oleh Nusa Panel ke pengaturan repository Anda di GitHub/GitLab.
5. Tuliskan **Build Script** pasca-pull:
   ```bash
   composer install --no-dev --optimize-autoloader
   php artisan migrate --force
   php artisan config:cache
   php artisan route:cache
   ```
6. Klik **Simpan & Deploy Pertama Kali**.

### 9.2 Auto-Deploy via Webhook
1. Nusa Panel menyediakan **Webhook URL** dan **Webhook Secret**.
2. Masuk ke pengaturan webhook repositori Git Anda (misal GitHub Settings > Webhooks).
3. Masukkan Webhook URL Nusa Panel dan Secret token.
4. Pilih event **Push Events**.
5. Setiap kali Anda melakukan `git push` ke branch target, Nusa Panel akan otomatis menarik kode terbaru, menjalankan build script, dan memuat ulang layanan.

---

## 10. Observabilitas & Log Viewer

### 10.1 Pemantauan Metrik PulseStrip
- **Grafik Real-time**: Pantau grafik CPU, penggunaan RAM fisik, operasi disk I/O, serta transfer bandwidth masuk/keluar secara langsung.
- **Status Kode HTTP**: Visualisasi persentase respons server (2xx Sukses, 3xx Redirect, 4xx Client Error, 5xx Server Error).

### 10.2 Penampil Log (Log Viewer)
Jangan biarkan bug aplikasi luput dari perhatian:
1. Buka tab **Logs** pada situs web Anda.
2. Pilih jenis log:
   - **Access Log**: Melihat histori permintaan pengunjung (IP, timestamp, path URL, User-Agent, response code).
   - **Error Log**: Melihat pesan error internal server Nginx.
   - **App Log**: Melihat output `stdout` dan `stderr` dari aplikasi Node.js, Python, atau PHP worker.
3. Fitur **Live Tail**: Saksikan pesan log masuk secara real-time saat aplikasi sedang diakses.
