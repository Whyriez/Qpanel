# ADR-0023: Manajemen Ekstensi PHP Terkurasi dan Preferensi Per-Situs

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.1 dan Bagian 11 (Task `P4-T2`), Nusa Panel membutuhkan kemampuan pengelolaan ekstensi PHP secara aman, terisolasi, dan terstruktur:
1. **Daftar Ekstensi Terkurasi**: Menyediakan 16 ekstensi PHP standar yang paling umum dibutuhkan aplikasi web modern: `mbstring`, `curl`, `gd`, `intl`, `zip`, `mysql`, `pgsql`, `redis`, `imagick`, `opcache`, `xml`, `bcmath`, `soap`, `sqlite3`, `bz2`, `readline`.
2. **Pengelolaan Per-Versi (Admin Only)**:
   - Memasang ekstensi baru (`apt-get install`) dan mencopot ekstensi (`apt-get remove --purge`).
   - Mengaktifkan dan menonaktifkan ekstensi yang telah terpasang (`phpenmod`, `phpdismod`).
   - Ekstensi dasar/inti (`curl`, `mbstring`, `mysql`, `opcache`, `xml`, `zip`) dilindungi agar tidak dapat dicopot secara tidak sengaja karena merupakan dependensi fondasi runtime PHP.
3. **Preferensi Ekstensi Per-Situs (Tenant / Admin)**:
   - Tenant dapat menentukan preferensi ekstensi yang aktif untuk situs mereka.
   - Preferensi disimpan secara terisolasi pada `sites.settings_json` (`{"php": {"extensions": [...]}}`).
   - Seluruh nama ekstensi divalidasi ketat terhadap daftar kurasi untuk mencegah eksekusi kode berbahaya atau manipulasi konfigurasi.

## Keputusan Arsitektur

1. **Katalog Terkurasi di `panel-runtime`**:
   - `CURATED_PHP_EXTENSIONS`: Konstanta statis berisi 16 definisi ekstensi (`PhpExtensionDefinition`), memuat metadata nama, kategori, deskripsi, nama paket per versi, nama file `.ini`, dan status modul inti (`is_base`).
   - `find_curated_extension(name)`: Fungsi lookup dengan pembersihan whitespace dan lowercase matching, menolak nama ekstensi di luar daftar resmi (`RuntimeError::UnsupportedPhpExtension`).

2. **Protokol IPC Berbasis Tipe di `panel-proto`**:
   - Mendefinisikan DTO: `PhpExtensionDto`, `PhpExtensionListRequest`, `PhpExtensionListResponse`, `PhpExtensionInstallRequest`, `PhpExtensionToggleRequest`, `PhpExtensionRemoveRequest`, dan `PhpExtensionActionResponse`.
   - Menambahkan varian payload yang sesuai pada `RequestPayload` dan `ResponsePayload` lengkap dengan tes serialisasi round-trip.

3. **Eksekusi Aman Host di `panel-agent`**:
   - Menambahkan utilitas Debian resmi `phpenmod` dan `phpdismod` ke `ALLOWED_BINARIES`.
   - Deteksi status terpasang: Memeriksa keberadaan file `/etc/php/{ver}/mods-available/{ini}.ini` dan status via `dpkg-query`.
   - Deteksi status aktif: Memeriksa symlink modul pada direktori SAPI `/etc/php/{ver}/fpm/conf.d/` dan `/etc/php/{ver}/cli/conf.d/`.
   - Pemasangan: Menjalankan `apt-get install -y --no-install-recommends php{ver}-{pkg}` dan mengaktifkannya dengan `phpenmod -v {ver} -s ALL {mod}`.
   - Toggle: Menjalankan `phpenmod` untuk mengaktifkan atau `phpdismod` untuk menonaktifkan.
   - Pencopotan: Memastikan modul bukan modul dasar, menonaktifkan symlink terlebih dahulu via `phpdismod`, kemudian menghapus paket via `apt-get remove -y --purge` dan `autoremove`.

4. **REST API dan Background Jobs di `panel-api`**:
   - Handlers background jobs terdaftar di `JobRegistry`:
     - `php.extension.install`: Memasang paket ekstensi di host dengan pelaporan progress bertahap.
     - `php.extension.remove`: Mencopot paket ekstensi dengan validasi non-base.
   - Endpoint REST:
     - `GET /api/v1/php/versions/{version}/extensions`: Menampilkan seluruh 16 ekstensi beserta status terpasang dan aktif.
     - `POST /api/v1/php/versions/{version}/extensions/{extension}/install`: Memicu job instalasi ekstensi (Admin + CSRF).
     - `POST /api/v1/php/versions/{version}/extensions/{extension}/toggle`: Mengaktifkan/menonaktifkan ekstensi secara sinkron (Admin + CSRF).
     - `POST /api/v1/php/versions/{version}/extensions/{extension}/remove`: Memicu job pencopotan ekstensi (Admin + CSRF).
     - `GET /api/v1/sites/{id}/php/extensions`: Mendapatkan preferensi ekstensi per-situs (Tenant/Admin).
     - `PUT /api/v1/sites/{id}/php/extensions`: Menyimpan preferensi ekstensi per-situs ke `settings_json` dengan validasi whitelist ketat (Tenant/Admin + CSRF).

5. **Antarmuka Pengguna Frontend (`web/`)**:
   - Komponen modal `PhpExtensionsModal.tsx`:
     - Menampilkan daftar ekstensi dengan pencarian instan dan filter chip per kategori (Cache, Database, Graphics, dll).
     - Badge visual: Modul Inti, Terpasang & Aktif, Terpasang (Nonaktif), dan Belum Terpasang.
     - Kontrol aksi: Tombol Pasang, Toggle Aktif/Nonaktif, dan Copot dengan konfirmasi bahaya.
   - Tombol "Ekstensi" pada setiap baris versi PHP yang terpasang di `RuntimePage.tsx`.
   - Dukungan lokalisasi dwibahasa penuh pada `id.json` dan `en.json`.

## Konsekuensi
- Admin dapat mengelola paket ekstensi host tanpa perlu akses SSH langsung.
- Tenant dapat mengonfigurasi preferensi ekstensi per situs mereka secara mandiri dan aman.
- Seluruh acceptance criteria task `P4-T2` terpenuhi dengan verifikasi pengujian unit dan integrasi.
