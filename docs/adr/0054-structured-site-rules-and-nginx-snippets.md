# ADR 0054: Structured Site Rules, Security Headers, and Nginx Snippets (P8-T3)

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5 (Manajemen Web & Routing) dan Bagian 8 (Fase 8 P8-T3: Aturan Situs / Site Rules):
1. **Kebutuhan Routing & Keamanan Tingkat Situs**:
   - Pengguna membutuhkan kontrol granular atas konfigurasi HTTP/HTTPS tanpa harus mengedit file konfigurasi Nginx secara manual.
   - Sembilan fitur aturan yang didefinisikan dalam PRD:
     1. **Redirect terstruktur**: Status HTTP 301 (Moved Permanently) dan 302 (Found), mode pencocokan (Exact, Prefix, Regex), pencegahan perulangan (loop detection: target path tidak boleh sama dengan source path), dan pelestarian query string (`$is_args$args`).
     2. **Header kustom & Keamanan**: Security headers standar (HSTS dengan opsi subdomains dan preload, X-Frame-Options DENY/SAMEORIGIN, X-Content-Type-Options nosniff, Referrer-Policy, Permissions-Policy, Content-Security-Policy) serta pasangan key-value header kustom (`add_header ... always;`).
     3. **Preset CORS**: Disabled, AllowAll (`*`), SameOrigin, dan Custom origins dengan penanganan preflight request `OPTIONS` (status 204 dan header Access-Control-* lengkap).
     4. **Proteksi Hotlink**: Validasi referer (`valid_referers none blocked server_names <domains>`), ekstensi berkas yang dilindungi (gambar, media, font), opsi izinkan referer kosong, dan aksi deny (403) atau pengalihan ke URL placeholder.
     5. **Basic Auth**: Proteksi path tertentu atau global, realm kustom, berkas htpasswd yang dideploy ke `/etc/panel/nginx/auth/site_<id>.htpasswd` dengan permission aman `0640`.
     6. **Kontrol Akses IP**: Scope global atau path spesifik, aturan allow/deny untuk IPv4/IPv6 dan CIDR, dengan opsi default allow (blacklist) atau default deny (whitelist).
     7. **Mode Pemeliharaan (Maintenance)**: Toggle on/off instan, bypass daftar IP administrator/pengembang, respons 503 Service Unavailable dengan header `Retry-After` dan pesan kustom atau halaman khusus.
     8. **Halaman Kesalahan Kustom (Custom Error Pages)**: Pemetaan kode status HTTP (400, 401, 403, 404, 500, 502, 503, 504) ke berkas dokumen internal (`internal;`).
     9. **Snippet Nginx**: Pintu darurat untuk konfigurasi tingkat lanjut, dibatasi ketat khusus pengguna dengan role `Admin`, dilengkapi pencegahan direktif berbahaya (denylist) dan validasi sintaks `nginx -t`.

---

## Keputusan Arsitektur

### 1. Model Domain & Validasi Ketat (`crates/panel-core/src/site_rules.rs`)
- Mendefinisikan struktur data serde yang kuat:
  - `SiteRulesConfig`, `StructuredRedirect`, `CustomHeader`, `SecurityHeadersConfig`, `CorsConfig`, `HotlinkConfig`, `BasicAuthConfig`, `BasicAuthUser`, `IpAccessConfig`, `IpAccessRule`, `MaintenanceConfig`, `CustomErrorPage`, `NginxSnippetConfig`.
- Menolak nilai mentah yang berbahaya:
  - Header name/value diperiksa terhadap karakter CRLF injection (`\r`, `\n`) dan karakter titik koma unescaped.
  - Alamat IP dan CIDR divalidasi ketat terhadap format IPv4/IPv6 standar.
  - Path URL wajib berawalan `/` dan bebas dari directory traversal atau karakter newline.
  - Snippet Nginx divalidasi terhadap denylist direktif berbahaya: `user `, `root /`, `alias /`, `load_module`, `include `, `access_log /dev/`, `error_log /dev/`, `lua_`, `perl_`, dan metadata IP cloud SSRF (`169.254.169.254`).

### 2. Password Hashing Bcrypt untuk htpasswd Nginx (`crates/panel-core/src/auth.rs`)
- Nginx `ngx_http_auth_basic_module` mendukung format crypt tradisional (DES, MD5 apr1, SHA-1, dan bcrypt `$2b$` / `$2y$`). Nginx **tidak** mendukung algoritma Argon2id.
- Ditambahkan dependency `bcrypt = "0.17"` pada workspace.
- Fungsi `hash_basic_auth_password(password: &str) -> Result<String, CoreError>` menghasilkan format hash bcrypt `$2b$` dengan cost factor standar 10 yang kompatibel penuh dengan Nginx `auth_basic_user_file`.

### 3. Kompiler Direktif Nginx Terintegrasi (`crates/panel-webserver/src/rules.rs`)
- Modul `rules.rs` memproses `SiteRulesConfig` menjadi blok string Nginx murni (`generate_site_rules_nginx_block`).
- Blok ini diinjeksikan secara aman ke dalam seluruh 5 context template vhost (`StaticVhostContext`, `PhpVhostContext`, `NodejsVhostContext`, `PythonVhostContext`, `ReverseProxyVhostContext`) melalui field `site_rules_block: Option<String>`.
- Menggunakan placeholder Jinja `{{ site_rules_block }}` sebelum penanganan blok routing aplikasi utama, memastikan aturan maintenance, basic auth, dan security headers dievaluasi dengan urutan prioritas yang benar di server block Nginx.

### 4. IPC Protobuf & Manajemen htpasswd di Agent (`panel-proto` & `panel-agent`)
- Pesan IPC baru:
  - `SiteHtpasswdDeployRequest` / `SiteHtpasswdDeployResponse`: Menulis berkas `/etc/panel/nginx/auth/site_<id>.htpasswd` secara atomik dengan izin akses `0640` milik user root dan group `www-data`.
  - `SiteHtpasswdRemoveRequest` / `SiteHtpasswdRemoveResponse`: Menghapus berkas htpasswd saat basic auth dinonaktifkan atau dihapus.
- Agent memverifikasi site ID numerik valid dan tidak memperbolehkan path traversal.

### 5. API Atomik, Otorisasi RBAC, dan Validasi Snippet (`crates/panel-api`)
- Handler:
  - `GET /api/v1/sites/{id}/rules`: Mengambil konfigurasi aturan situs dari `settings_json["site_rules"]`.
  - `PUT /api/v1/sites/{id}/rules`: Memperbarui aturan situs.
    - Tenant non-admin dilarang mengubah atau mengaktifkan `nginx_snippet` (HTTP 403 Forbidden).
    - Memvalidasi snippet terhadap denylist.
    - Menerapkan htpasswd deploy/remove ke agent bila basic auth diubah.
    - Mengompilasi ulang vhost Nginx dan mengujinya melalui `nginx -t` secara atomik dengan rollback otomatis bila gagal.
    - Mencatat audit log.
  - `POST /api/v1/sites/{id}/rules/hash-password`: Memfasilitasi hashing password bcrypt untuk htpasswd.
  - `POST /api/v1/sites/{id}/rules/validate-snippet`: Pengujian sintaks dan denylist snippet secara terisolasi.

### 6. Antarmuka Pengguna Frontend Modern (`web/src/components/sites/SiteRulesTab.tsx`)
- Tab `Aturan` (`rules`) terintegrasi langsung pada `SiteDetailPage.tsx`.
- 9 sub-tab navigasi pill yang intuitif: Redirect, Header, CORS, Hotlink, Basic Auth, Akses IP, Maintenance, Halaman Error, dan Snippet Nginx.
- Indikator khusus "Khusus Administrator" dan perlindungan form dinonaktifkan untuk tenant reguler pada tab Snippet Nginx.
- Uji snippet Nginx real-time dengan feedback syntax error Nginx.
- Sepenuhnya dilokalisasi dwibahasa (Bahasa Indonesia dan English) serta memenuhi standar aksesibilitas WCAG 2.2 AA.

---

## Konsekuensi

### Positif
- Arsitektur aman: semua direktif Nginx dihasilkan dari generator terstruktur tanpa konkatenasi string bebas dari input klien.
- Perlindungan injeksi konfigurasi: denylist direktif berbahaya dan validasi `nginx -t` menjamin stabilitas web server master.
- Isolasi otorisasi: pengguna tenant memiliki antarmuka aman tanpa risiko merusak konfigurasi sistem, sementara Admin tetap memiliki fleksibilitas snippet penuh.
- Atomik: jika ada kesalahan konfigurasi atau sertifikat, vhost di-rollback otomatis ke versi sebelumnya tanpa downtime.

### Netral
- Penambahan dependency `bcrypt = "0.17"` pada workspace khusus untuk pembuatan berkas `htpasswd`.
