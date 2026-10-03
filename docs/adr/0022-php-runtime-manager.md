# ADR-0022: Manajer Runtime PHP dan Kebijakan Masking Layanan FPM OS

## Status
Accepted

## Konteks
Sesuai PRD Bagian 6 (Fase 4: PHP & Database), spesifikasi task `P4-T1` mengharuskan Nusa Panel menyediakan kemampuan manajemen runtime multi-versi PHP (7.4, 8.0, 8.1, 8.2, 8.3, 8.4) pada Ubuntu 24.04:
1. Menambahkan repositori Sury PHP PPA (`ppa:ondrej/php`) dan pembaruan index apt secara aman.
2. Memasang paket runtime dasar per versi: `phpX.Y-fpm`, `phpX.Y-cli`, `phpX.Y-common`, `phpX.Y-opcache`, serta ekstensi umum (`curl`, `mbstring`, `xml`, `zip`, `mysql`).
3. Menghapus paket runtime PHP jika tidak lagi dibutuhkan.
4. **Persyaratan Kritis PRD Bagian 6**: Service default OS `phpX.Y-fpm.service` **harus segera di-mask** (`systemctl mask`) saat instalasi agar tidak pernah berjalan secara global atau sebagai root. Panel nantinya (pada `P4-T3`) akan membuat dan mengelola master FPM per-tenant secara terisolasi.
5. Operasi instalasi, konfigurasi repo, dan penghapusan dijalankan sebagai background job terdistribusi dengan pelaporan progress bertahap (0-100%) dan pembatalan jika diperlukan.
6. Mencegah penghapusan versi PHP yang sedang aktif digunakan oleh situs di database.

## Keputusan Arsitektur

1. **Abstraksi Tipe Kuat di `panel-runtime`**:
   - Enum `PhpVersion` dengan parsing ketat (`PhpVersion::parse`) dan whitelist resmi: `7.4`, `8.0`, `8.1`, `8.2`, `8.3`, `8.4`.
   - String interpolation atau input mentah dari shell dilarang keras, mencegah potensi command injection.
   - Definisi nama paket, binary paths (`/usr/sbin/php-fpmX.Y`, `/usr/bin/phpX.Y`), dan nama service (`phpX.Y-fpm.service`) terpusat dalam satu modul.

2. **Protokol IPC Typed di `panel-proto`**:
   - Menambahkan payload: `PhpRepoSetupRequest`, `PhpRepoSetupResponse`, `PhpRuntimeListRequest`, `PhpRuntimeListResponse`, `PhpRuntimeInstallRequest`, `PhpRuntimeRemoveRequest`, `PhpRuntimeActionResponse`.
   - `panel-api` tidak pernah memanggil root atau command shell secara langsung, melainkan mengirim RPC terstruktur ke `panel-agent`.

3. **Eksekusi Aman dan Masking di `panel-agent`**:
   - Menambahkan binary yang diizinkan ke `ALLOWED_BINARIES`: `add-apt-repository`, `apt-get`, `dpkg-query`.
   - Alur instalasi PHP:
     1. `apt-get install -y --no-install-recommends <packages>`
     2. `systemctl stop phpX.Y-fpm.service` (jika berjalan)
     3. `systemctl mask phpX.Y-fpm.service` (kritis)
     4. Verifikasi status `systemctl is-enabled` menghasilkan `masked`.
   - Alur penghapusan PHP:
     1. `systemctl unmask phpX.Y-fpm.service`
     2. `apt-get remove -y --purge <packages>`

4. **Background Jobs dan Validasi Dependensi di `panel-api`**:
   - Handlers background jobs terdaftar di `JobRegistry`:
     - `php.repo.setup`: Menjalankan setup PPA Sury dan `apt-get update`.
     - `php.install`: Memasang paket PHP dan melakukan masking service FPM.
     - `php.remove`: Memeriksa penggunaan pada tabel `sites` (`count_by_runtime_and_version`). Jika terdapat situs yang menggunakan versi tersebut, job dibatalkan dengan pesan informatif dalam Bahasa Indonesia.
   - Endpoint REST:
     - `GET /api/v1/php/versions`: Daftar versi PHP yang didukung beserta status instalasi dan masking.
     - `POST /api/v1/php/repo/setup`: Memicu job penyiapan PPA Sury (hanya Admin + CSRF).
     - `POST /api/v1/php/versions/{version}/install`: Memicu job instalasi runtime (hanya Admin + CSRF).
     - `POST /api/v1/php/versions/{version}/remove`: Memicu job pencopotan runtime dengan pre-check database (hanya Admin + CSRF).

5. **Antarmuka Web Pengguna (`RuntimePage.tsx`)**:
   - Desain premium sesuai design token Nusa Panel.
   - Kartu status repositori PPA Sury dengan aksi sekali klik.
   - Banner penjelasan arsitektur keamanan masking service OS FPM.
   - Tabel/daftar versi PHP dengan badge status (Terpasang / Tersedia, Dimask), path biner (CLI & FPM), dan dialog konfirmasi bahaya sebelum mencopot versi.
   - Integrasi ke navigasi utama `Shell.tsx` dan dukungan penuh internasionalisasi (`id.json` & `en.json`).

## Konsekuensi
- Layanan FPM bawaan OS tidak akan pernah berjalan di port default atau mengeksekusi script dengan privilese root.
- Seluruh instalasi versi PHP berjalan secara non-blocking di latar belakang tanpa risiko timeout HTTP gateway.
- Seluruh acceptance criteria `P4-T1` terpenuhi dan terbukti melalui tes unit di `panel-runtime`, tes IPC di `panel-agent`, tes integrasi di `panel-api`, serta tes komponen di `web/`.
