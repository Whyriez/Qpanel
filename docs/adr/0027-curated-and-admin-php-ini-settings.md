# ADR-0027: Pengaturan Curated dan Admin Raw php.ini Terisolasi Per-Pool

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.1 dan Bagian 11 (Task `P4-T6`), pengguna memerlukan kemampuan untuk mengatur konfigurasi `php.ini` per-situs secara aman dan terisolasi:
1. **Curated Directives untuk Tenant/User Biasa**:
   - Direktif umum seperti `memory_limit`, `upload_max_filesize`, `post_max_size`, `max_execution_time`, `max_input_vars`, `display_errors`, `date.timezone`, dan pengaturan OPcache (`opcache.enable`, `opcache.memory_consumption`, `opcache.validate_timestamps`, `opcache.revalidate_freq`).
   - Direktif ini harus dibatasi secara ketat oleh batas paket hosting (plan limits) tenant, misalnya `memory_limit` tidak boleh melebihi jatah RAM paket, `upload_max_filesize` tidak boleh melebihi `upload_limit_mb` paket, dan `post_max_size >= upload_max_filesize`.
2. **Raw php.ini Editor Khusus Admin**:
   - Admin server memerlukan fleksibilitas untuk menambahkan direktif raw `php.ini`, namun tetap tidak boleh mengompromikan isolasi sistem.
   - Direktif berbahaya atau yang dikelola oleh panel (seperti `open_basedir`, `extension`, `zend_extension`, `user`, `group`, direktif berkas internal) wajib ditolak oleh allowlist parser.
3. **Isolasi Konfigurasi & Reload Instan**:
   - Pengaturan diterapkan per-pool FPM melalui `php_admin_value` dan `php_admin_flag` pada file konfigurasi pool FPM tenant (`/etc/panel/php/<user>-<version>/pool.d/<site>.conf`).
   - Perubahan langsung dimuat ulang dengan mengirim sinyal reload ke service master FPM tenant (`systemctl reload panel-fpm@<user>-<version>.service`) tanpa restart service dan tanpa downtime.

## Keputusan Arsitektur

1. **Model dan Validasi Terpusat di `panel-core` (`crates/panel-core/src/php.rs`)**:
   - `PhpSettings`: Menyimpan 11 direktif terkurasi dengan nilai default standar produksi.
   - `PhpSettingsLimits`: Menyimpan batas maksimum direktif yang diturunkan dari paket langganan tenant (`memory_limit_mb`, `upload_limit_mb`, `max_execution_time`, `max_input_vars`, `opcache_memory_consumption_mb`).
   - `PhpSettings::validate(&self, limits: &PhpSettingsLimits)`: Menjamin nilai berada dalam rentang aman dan mematuhi batas paket langganan, termasuk validasi format nama timezone IANA (`date_timezone`).
   - `PhpSettings::to_ini_directives(&self)`: Mengonversi struct ke pasangan key-value direktif PHP.
   - `parse_admin_ini(raw: &str) -> Result<BTreeMap<String, String>, CoreError>`: Mem-parsing baris konfigurasi INI raw maksimal 8 KiB dengan sistem allowlist ketat (`allow_url_fopen`, `display_startup_errors`, `default_socket_timeout`, `error_reporting`, `disable_functions`), menolak injeksi syntax, karakter berbahaya, atau percobaan escape isolasi.

2. **Perluasan Protokol IPC dan Rendering Pool (`panel-proto`, `panel-runtime`, `panel-agent`)**:
   - `PhpFpmPoolApplyRequest` diperkaya dengan field:
     - `settings: Option<PhpSettings>`
     - `admin_ini: Option<String>`
   - `PhpFpmPoolConfig` di `panel-runtime` mendukung `ini_directives: BTreeMap<String, String>`. Konfigurasi pool merender direktif boolean sebagai `php_admin_flag[k] = on/off` dan nilai lainnya sebagai `php_admin_value[k] = v`.
   - `apply_php_fpm_pool` di `panel-agent` menggabungkan curated settings dan admin INI, melakukan validasi sintaks FPM staging (`php-fpm<v> -t`), dan melakukan reload atomik pada unit systemd `panel-fpm@<user>-<version>.service`.

3. **API Endpoints dan RBAC di `panel-api` (`crates/panel-api/src/php.rs`)**:
   - `GET /api/v1/sites/{id}/php/settings`:
     - Mengembalikan konfigurasi PHP aktif, batas paket langganan tenant terkait, dan field `is_admin`.
     - Field `admin_ini` hanya disertakan jika pemanggil memiliki role `admin`.
   - `PUT /api/v1/sites/{id}/php/settings`:
     - Memvalidasi batas curated settings terhadap paket tenant di database.
     - Menolak payload yang berisi `admin_ini` jika pemanggil bukan admin (HTTP 403 Forbidden).
     - Menyimpan konfigurasi dalam kolom `settings_json` pada tabel `sites`.
     - Mengirim RPC `apply_php_fpm_pool` dengan `reload: true` ke daemon agent.
     - Mencatat audit event `site.php_settings_updated`.
   - `switch_site_php_version_handler`: Diperbarui untuk mempertahankan curated settings dan admin ini yang tersimpan saat memindahkan pool ke versi PHP baru.

4. **Komponen Antarmuka Web (`web/src/components/sites/PhpSettingsCard.tsx` & `SiteDetailPage.tsx`)**:
   - Tab "Pengaturan PHP" khusus pada situs dengan tipe PHP (`site.kind === 'php'`).
   - Kartu kontrol intuitif untuk batas memori, batas upload file, ukuran POST, batas eksekusi script, timezone selector, dan switch OPcache.
   - Indikator batas paket (plan limit badge) yang jelas agar tenant memahami batas maksimal langganan mereka.
   - Blok editor teks monospace untuk Raw `php.ini` yang hanya muncul bagi akun administrator.
   - Validasi interaktif client-side dengan notifikasi toast Bahasa Indonesia.

## Konsekuensi & Keamanan
- **Keamanan Isolasi**: Pengaturan ditulis menggunakan `php_admin_value` dan `php_admin_flag` sehingga skrip PHP pengguna (`ini_set()`) tidak dapat melanggar batas memori atau membuka isolasi direktori.
- **Batasan Sumber Daya**: Tenant tidak dapat mengatur `memory_limit` di atas RAM yang dialokasikan dalam paket mereka.
- **Ketersediaan Layanan**: Perubahan konfigurasi pool diterapkan melalui `systemctl reload` pada master FPM tenant, memproses konfigurasi baru secara instan pada worker berikutnya tanpa downtime pada worker yang sedang melayani request.

