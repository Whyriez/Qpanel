# ADR 0018: Domain Aliases, Redirects, and Force HTTPS

## Status
Accepted

## Context
Setelah penyediaan situs statis dasar selesai di P3-T4, sebuah website hosting memerlukan kapabilitas pengaturan domain tingkat lanjut (P3-T5):
1. **Alias dan Subdomain**: Satu website dapat merespons beberapa domain alias atau subdomain sekaligus (`server_name example.com www.example.com app.example.com;`).
2. **Aturan Pengalihan (Redirect Rules)**:
   - Pengalihan domain sekunder ke domain utama (`to_primary`).
   - Normalisasi `www` ke `non-www` (`to_non_www`) atau sebaliknya (`to_www`).
   - Pengalihan kustom ke URL eksternal dengan status code HTTP 301 (permanen) atau 302 (sementara).
3. **Enforce HTTPS (Force HTTPS)**: Opsi pengalihan lalu lintas port 80 (HTTP) ke port 443 (HTTPS) secara global per situs ketika sertifikat SSL terpasang.
4. **Keamanan Anti-Injeksi**: Parameter target pengalihan rawan injeksi direktif konfigurasi Nginx apabila menerima input sembarang yang mengandung tanda titik koma (`;`), kurung kurawal (`{}`), karakter baris baru, atau variabel Nginx arbitrer.

## Decision
1. **Pencegahan Injeksi Konfigurasi Nginx (`panel-webserver`)**:
   - Diimplementasikan validator ketat `validate_redirect_target()` pada `crates/panel-webserver/src/template.rs`.
   - Melarang karakter kontrol, pemisah baris (`\n`, `\r`, `\0`), spasi, petik ganda/tunggal, dan karakter terminasi Nginx (`;`, `{`, `}`).
   - Penggunaan variabel `$` dibatasi hanya pada token aman yang diizinkan: `$request_uri`, `$host`, dan `$scheme`. Pola variabel lainnya ditolak keras.
   - `DomainRedirectConfig` ditambahkan ke `StaticVhostContext` untuk merender blok server pengalihan tersendiri pada port 80 dan port 443.

2. **Pengelolaan Domain Sekunder & Redirect di `panel-api`**:
   - Disediakan endpoint RESTful:
     - `GET /api/v1/sites/{id}/domains`: Menampilkan seluruh domain yang terhubung ke situs.
     - `POST /api/v1/sites/{id}/domains`: Mendaftarkan domain/subdomain/alias baru ke situs dengan validasi RFC & Punycode serta pengecekan keunikan global.
     - `PATCH /api/v1/sites/{id}/domains/{domain_id}`: Memperbarui mode redirect (`none`, `to_primary`, `to_www`, `to_non_www`, `custom`), target redirect kustom, atau mempromosikan domain sekunder menjadi domain utama (`is_primary: true`).
     - `DELETE /api/v1/sites/{id}/domains/{domain_id}`: Menghapus domain sekunder dari situs (domain utama dicegah untuk dihapus).
     - `POST /api/v1/sites/{id}/force-https`: Mengaktifkan atau menonaktifkan pengalihan HTTP ke HTTPS.
   - Setiap mutasi domain memicu kompilasi ulang konfigurasi vhost (`recompile_and_apply_site_vhost`) dan reload Nginx secara atomik melalui IPC agent.
   - Setiap aksi diverifikasi hak aksesnya (RBAC isolasi tenant) dan dicatat ke `audit_logs` (`site.domain.add`, `site.domain.update`, `site.domain.delete`, `site.force_https`).

3. **Pengujian Integrasi**:
   - Dibuat rangkaian pengujian integrasi `crates/panel-api/tests/domain_redirect_api_test.rs` yang menguji penambahan alias, subdomain, konfigurasi redirect 301/302, validasi pencegahan injeksi, promosi primary domain, proteksi penghapusan primary domain, dan toggle force-HTTPS.

## Consequences
- Konfigurasi Nginx tersinkronisasi otomatis dengan status database domain dan situs secara atomik.
- Pengguna dapat mengelola pemetaan multi-domain dan normalisasi URL SEO tanpa risiko kegagalan sintaks Nginx atau eksploitasi injeksi konfigurasi.
