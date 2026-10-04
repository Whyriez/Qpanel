# ADR-0025: Template Nginx PHP, Presets Aplikasi, Aturan Keamanan, dan FastCGI Microcache

## Status
Accepted

## Konteks
Sesuai PRD Bagian 4.4, 5.1, dan Bagian 11 (Task `P4-T4`), implementasi virtual host Nginx untuk situs PHP membutuhkan fleksibilitas framework/preset yang tinggi tanpa mengorbankan keamanan multi-tenant, kestabilan server, dan efisiensi throughput:
1. **Dukungan Preset Aplikasi Modern**:
   - Berbagai aplikasi PHP memiliki struktur direktori dan mekanisme front-controller yang berbeda:
     - `generic`: Script PHP standar (`index.php`), mendukung eksekusi file `.php` langsung jika ada di filesystem atau fallback ke `index.php`.
     - `wordpress`: Front-controller standar WordPress (`/index.php?$args`), rewrite ramah permalink, dan pemblokiran ketat eksekusi script PHP di direktori uploads (`/wp-content/uploads/.*\.php$`).
     - `laravel`: Document root terisolasi pada subdirektori `public/`, routing front-controller `index.php?$query_string`, dan proteksi folder framework (`storage/`, `bootstrap/`, `.env`).
     - `custom`: Front-controller kustom yang dapat ditentukan (misal `app.php`), memvalidasi bahwa nama file aman (`^[a-zA-Z0-9_\-]+\.php$`) tanpa traversal atau injeksi.
2. **Hardening Keamanan dan Pemblokiran File Sensitif**:
   - Mencegah kebocoran file konfigurasi, kredensial database, kontrol versi, dan file build: `.env`, `.git`, `.user.ini`, `composer.json`, `composer.lock`, `package.json`, `package-lock.json`, `web.config`, `wp-config.php`, file backup (`.bak`, `.swp`), serta file tersembunyi (`.*`).
   - Mencegah arbitrary code execution di folder upload media.
3. **Batas Upload Dinamis**:
   - `client_max_body_size` yang dapat dikonfigurasi per situs dengan format aman (`\d+[kKmMgG]?`, contoh: `64M`, `128M`), divalidasi ketat untuk mencegah injeksi konfigurasi Nginx.
4. **FastCGI Microcache Opsional**:
   - Zona cache baseline `panel_php_cache` pada `conf.d/20-fastcgi-cache.conf` (`keys_zone=panel_php_cache:10m max_size=256m inactive=60m`).
   - Konfigurasi per situs dengan bypass cerdas: mengabaikan cache saat `POST`, query string dinamis non-GET/HEAD, cookie sesi pengguna (seperti `wordpress_logged_in_*`, `comment_author_*`), authorization headers, dan penambahan header diagnostik `X-Cache-Status $upstream_cache_status`.

## Keputusan Arsitektur

1. **Template Jinja Nginx PHP di `templates/nginx/vhost_php.j2`**:
   - Template mandiri yang memisahkan konfigurasi site PHP dengan static site.
   - Mengimplementasikan routing per preset:
     - Preset `generic`: `try_files $uri $uri/ /index.php$is_args$args;` dengan penanganan file `.php` langsung via `fastcgi_split_path_info`.
     - Preset `wordpress`: `try_files $uri $uri/ /index.php?$args;`, blokir `location ~* /(?:uploads|files)/.*\.php$ { deny all; }`, dan proteksi `wp-config.php`.
     - Preset `laravel`: Memastikan root mengarah ke `public/`, dan routing `try_files $uri $uri/ /index.php?$query_string;`.
     - Preset `custom`: Routing `try_files $uri $uri/ /{{ custom_entry_file }}?$query_string;`.
   - Mengintegrasikan blokir file sensitif bawaan dan include `snippets/deny-sensitive.conf` dan `snippets/security-headers.conf`.
   - Mendukung integrasi SSL otomatis, force HTTPS redirect, domain aliases, dan redirect domain (www vs non-www / domain kustom).

2. **Domain Model dan Validasi Anti-Injeksi di `panel-webserver`**:
   - Menambahkan enum `PhpPreset` (`Generic`, `Wordpress`, `Laravel`, `Custom`).
   - Menambahkan struct `FastCgiCacheConfig`:
     - `enabled: bool`
     - `inactive: String`
     - `valid_200: String`
     - `valid_other: String`
   - Menambahkan struct `PhpVhostContext` dengan validasi ketat:
     - `validate_php_vhost_inputs`: Memeriksa karakter berbahaya (`;`, `{`, `}`, `$`, `\n`, `\r`, `\0`), memvalidasi socket path Unix Domain Socket (`.sock`), memvalidasi format `upload_max_size`, dan memvalidasi `custom_entry_file`.
   - Menambahkan template `vhost_php` ke `NginxTemplateRenderer` dengan method `render_php`.
   - Menambahkan baseline cache zone `conf.d/20-fastcgi-cache.conf` melalui `NginxBaselineConfig::cache_dir` dan `install_baseline`.

3. **Integrasi Pipeline Situs dan FPM di `panel-api`**:
   - `apply_site_config`: Mendeteksi jenis situs (`site.kind == "php"`).
     - Mengekstrak `php_version`, `preset`, `custom_entry_file`, `upload_max_size`, dan `fastcgi_cache` dari metadata/settings situs.
     - Menyusun `PhpVhostContext` dengan socket UDS tenant `/run/panel/php/<user>-<ver>.sock` atau `/run/panel/php/<user>-<ver>-<site_id>.sock`.
     - Melakukan render melalui `NginxTemplateRenderer::render_php`.
   - `create_static_site_handler`:
     - Menghandle pembuatan situs static maupun PHP.
     - Jika `site.kind == "php"`, otomatis memastikan unit master PHP-FPM aktif (`ensure_php_fpm_master`) dan mengonfigurasi pool situs (`apply_php_fpm_pool`) sebelum konfigurasi Nginx diterapkan atomik.
     - Menyediakan placeholder `index.php` yang informatif dan ramah pengguna di docroot situs baru.
   - `delete_site_handler`:
     - Jika situs bertipe PHP, secara bersih menghapus pool FPM situs terkait (`remove_php_fpm_pool`), yang otomatis men-stop master FPM jika sudah tidak memiliki pool tersisa.

## Konsekuensi

### Positif
- **Fleksibilitas Penuh**: Mendukung WordPress, Laravel, CMS kustom, dan PHP klasik tanpa konfigurasi manual dari pengguna.
- **Keamanan Default yang Kuat**: Pemblokiran file sensitif dan larangan eksekusi PHP di folder upload mencegah eksploitasi umum pada aplikasi PHP/WordPress.
- **Kinerja Optimal**: FastCGI microcache bawaan memberikan throughput ribuan request/detik untuk request publik/anonim tanpa membebani PHP worker.
- **Kerapian Arsitektural**: Terintegrasi bersih dengan siklus hidup master PHP-FPM tenant (ADR-0024) dan pipeline staging atomik Nginx (ADR-0016).

### Kompromi & Mitigasi
- Direktori cache `/var/cache/nginx/panel` membutuhkan alokasi memori/disk yang terkontrol: dibatasi maksimum `256m` dengan retensi `inactive=60m` secara baseline.
- Preset Laravel membutuhkan document root berakhiran `public/`: API dan template secara otomatis menyesuaikan path tanpa intervensi manual dari pengguna.
