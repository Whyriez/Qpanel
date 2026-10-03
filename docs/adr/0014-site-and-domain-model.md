# ADR-0014: Model Website dan Domain serta Validasi IDN/Punycode

- **Status**: Diterima (Accepted)
- **Tanggal**: 2026-10-03
- **Konteks**: PRD Bagian 4.4, 5.1, Task P3-T1

## Konteks dan Masalah

Pada awal Phase 3, Nusa Panel membutuhkan model data inti dan validasi untuk situs web (*sites*) dan domain (*domains*) sebagai fondasi seluruh engine hosting (Nginx, SSL, PHP, dsb.):
1. **Model Website (`sites`)**:
   - Terikat dengan akun tenant (`user_id`).
   - Memiliki jenis runtime (`kind`): `static`, `php`, `nodejs`, `python`, `proxy`.
   - Menentukan direktori dokumen (`docroot`), runtime pilihan, versi runtime, status operasional (`active`, `disabled`, `suspended`), serta konfigurasi fleksibel dalam format JSON (`settings_json`).
2. **Model Domain (`domains`)**:
   - Mendukung domain primer (*primary domain*) dan alias domain per situs.
   - Kebijakan redirect (`redirect_mode`): `none`, `to_primary`, `to_www`, `to_non_www`, `custom`.
   - Status domain: `active`, `pending_ssl`, `disabled`.
3. **Validasi Domain dan IDN (Internationalized Domain Names)**:
   - Domain dapat menggunakan karakter non-ASCII (misal `münchen.de`, `café.com`, `toko.id`).
   - Sesuai standar RFC 1034, 1035, dan 5890, domain harus dinormalisasi menjadi representasi ASCII Punycode (`xn--...`) agar aman disimpan di filesystem, Nginx server block, sertifikat ACME, dan query DNS.
   - Perlindungan anti-injeksi: Karakter berbahaya seperti titik-koma (`;`), kutip (`"`, `'`), kurung kurawal (`{`, `}`), spasi/whitespace, newline, dan karakter kontrol harus ditolak secara tegas di level domain parsing sebelum mencapai Nginx atau shell.
4. **Keunikan Domain Global**:
   - Satu domain (baik dalam bentuk ASCII punycode maupun Unicode) hanya boleh diklaim oleh satu situs di seluruh sistem untuk mencegah konflik vhost server block.

## Keputusan Arsitektur

### 1. Penambahan Dependensi `idna`
- Menambahkan dependensi `idna = "1.0"` pada workspace Cargo untuk konversi standar IDNA/Punycode.
- Dependensi ini aman, berkinerja tinggi, dan merupakan pustaka standar de-facto pada ekosistem Rust (digunakan oleh proyek Servo dan `url`).

### 2. Modul `DomainName` pada `panel-core` (`crates/panel-core/src/domain.rs`)
- `DomainName::parse(raw: &str) -> Result<DomainName, DomainValidationError>`:
  - Memeriksa batas panjang: total panjang ASCII <= 253 karakter, panjang tiap label 1-63 karakter.
  - Memastikan tidak ada spasi, karakter kontrol, ataupun karakter injeksi Nginx/shell.
  - Memastikan TLD tidak seluruhnya berupa angka (aturan ICANN / RFC 1123).
  - Melakukan konversi IDN Punycode via `idna::domain_to_ascii` (menghasilkan representasi ASCII lowercase) dan `idna::domain_to_unicode` (untuk tampilan antarmuka pengguna).
  - Menyimpan representasi `ascii` (contoh: `xn--mnchen-3ya.de`) dan `unicode` (contoh: `münchen.de`).

### 3. Migrasi Database `20261003000004_sites_domains.sql`
- Tabel `sites`:
  - `user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE`
  - `kind TEXT NOT NULL CHECK(kind IN ('static', 'php', 'nodejs', 'python', 'proxy'))`
  - `status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'disabled', 'suspended'))`
- Tabel `domains`:
  - `site_id INTEGER NOT NULL REFERENCES sites(id) ON DELETE CASCADE`
  - `name TEXT NOT NULL UNIQUE` (memaksa keunikan domain di level indeks database)
  - `unicode_name TEXT NOT NULL`
  - `is_primary INTEGER NOT NULL DEFAULT 0 CHECK(is_primary IN (0, 1))`
  - `redirect_mode TEXT NOT NULL DEFAULT 'none' CHECK(redirect_mode IN ('none', 'to_primary', 'to_www', 'to_non_www', 'custom'))`

### 4. Repositori pada `panel-db`
- `SqliteSiteRepository`: Operasi CRUD website, update status, dan penghitungan jumlah situs per tenant terhadap limit plan.
- `SqliteDomainRepository`:
  - Operasi CRUD domain dan pengecekan ketersediaan global `is_domain_available`.
  - Promosi domain primer yang atomik (`set_primary` mendemosi domain primer sebelumnya dalam satu transaksi SQLite).
  - Penghapusan situs otomatis menghapus seluruh domain terikat secara *cascade*.

## Konsekuensi

- Penyimpanan domain konsisten dan aman dari injeksi konfigurasi Nginx.
- Representasi Punycode menjamin kompatibilitas penuh dengan sistem sertifikat ACME (Let's Encrypt) dan Nginx vhost di Phase 3 berikutnya.
- Penghitungan batasan kuota `max_sites` dan `max_domains` dari paket hosting dapat diverifikasi secara akurat.
