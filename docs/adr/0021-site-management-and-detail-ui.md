# ADR-0021: Arsitektur UI Manajemen Situs dan Detail Proyek

## Status
Accepted

## Konteks
Sesuai PRD bagian 8, bagian 5.5, dan spesifikasi task `P3-T8`, Nusa Panel memerlukan antarmuka pengguna berbasis web (React + TypeScript + Tailwind CSS) untuk mengelola situs web statis yang telah dibangun pada Phase 3 (P3-T1 hingga P3-T7). Antarmuka harus mencakup:
1. **Daftar Situs (`SitesPage`)**: Tabel virtual dan responsif yang menampilkan domain utama, pemilik (tenant), tipe runtime (Static / SPA), sparkline aktivitas lalu lintas (`PulseStrip`), status vhost (`StatusPill`), dan indikator keamanan SSL/HTTPS.
2. **Wizard Pembuatan Situs 4 Langkah (`SiteWizardDialog`)**:
   - Langkah 1: Pemilihan tipe aplikasi (Static HTML/SPA aktif; PHP dan Node.js ditampilkan dengan indikator fase mendatang).
   - Langkah 2: Konfigurasi domain utama (dengan validasi format dan dukungan IDN/Punycode), opsi SPA fallback (`try_files $uri /index.html`), dan opsi paksa enkripsi HTTPS.
   - Langkah 3: Alokasi pemilik tenant dengan pemindaian otomatis Linux UID dan direktori terisolasi (`/home/p_<slug>/`).
   - Langkah 4: Tinjauan parameter konfigurasi lengkap sebelum eksekusi mutasi atomik ke Nginx.
3. **Halaman Detail Situs (`SiteDetailPage`)**:
   - **Tab Ringkasan (Overview)**: Status vhost real-time, docroot path dengan tombol salin, kartu keamanan SSL dengan hitung mundur kedaluwarsa sertifikat, dan metrik konfigurasi.
   - **Tab Domain**: Tabel alias domain, modal tambah alias, konfigurasi mode pengalihan (none, www ↔ non-www, custom URL target), dan penghapusan alias domain non-primer.
   - **Tab SSL**: Status sertifikat X.509, modal penerbitan Let's Encrypt otomatis (dengan opsi staging dan auto-fallback self-signed), pembuat sertifikat self-signed sekali klik, modal unggah custom certificate (fullchain + private key PEM), serta aksi perbarui (renew) dan hapus sertifikat.
   - **Tab Log**: Terminal log monospaced bergaya gelap dengan penyorotan kode status HTTP (2xx, 3xx, 4xx, 5xx), pemformatan otomatis log JSON Nginx (`panel_json`), filter pencarian kata kunci, pembatasan jumlah baris riwayat, dan live streaming via WebSocket (`/ws/logs/{id}`) lengkap dengan status indikator koneksi (terhubung / jeda).
   - **Tab Pengaturan**: Preferensi vhost (Force HTTPS dan SPA Fallback) serta zona bahaya untuk menghapus situs menggunakan dialog pengaman ketik nama resource (`ConfirmDangerDialog`).

## Keputusan Arsitektur

1. **Komponen Modular & Desain Bersistem**:
   - Menggunakan design tokens Nusa Panel (`--bg`, `--surface`, `--raised`, `--line`, `--ink`, `--ink-muted`, `--primary`, dll.) dan font self-hosted ("Plus Jakarta Sans" dan "JetBrains Mono").
   - Memanfaatkan komponen UI yang sudah teruji: `StatusPill`, `PulseStrip`, `EmptyState`, dan `ConfirmDangerDialog`.
2. **Streaming WebSocket Tahan Gangguan**:
   - Menggunakan hook `useEffect` dengan pembersihan event socket dan penanganan pesan JSON (`initial_backlog` dan `log_line`).
   - Terminal log dilengkapi auto-scroll ke bawah saat baris log baru masuk.
3. **Internasionalisasi (i18n)**:
   - Seluruh teks antarmuka disediakan dalam bahasa Indonesia (default) dan bahasa Inggris melalui `id.json` dan `en.json`.
   - Mengikuti aturan sentence case dan kata kerja aktif ("Situs Baru", "Terbitkan Let's Encrypt", "Hapus Situs").
4. **Keamanan Interaksi**:
   - Penghapusan situs dan domain terlindungi oleh `ConfirmDangerDialog` yang mewajibkan input teks konfirmasi persis.
   - Operasi mutasi selalu menyertakan token CSRF dan kredensial cookie aman.

## Konsekuensi
- Ukuran bundel JavaScript hasil build Vite tetap sangat ringkas (~160 KB gzip), jauh di bawah batas target PRD (≤ 250 KB gzip).
- Seluruh acceptance criteria P3-T8 terpenuhi dan divalidasi oleh suite pengujian unit/komponen Vitest.
