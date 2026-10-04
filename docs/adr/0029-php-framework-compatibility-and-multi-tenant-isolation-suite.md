# ADR-0029: Suite Pengujian Kompatibilitas Framework PHP dan Isolasi Multi-Tenant

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.1 dan Bagian 11 (Task `P4-T8`), arsitektur PHP multi-versi terisolasi memerlukan pembuktian kepatuhan yang ketat sebelum dirilis ke produksi:
1. **Kompatibilitas Preset Framework Populer**:
   - **WordPress**:
     - Perlindungan terhadap file konfigurasi sensitif (`wp-config.php`).
     - Pemblokiran eksekusi file script `.php` di dalam direktori unggahan (`/wp-content/uploads/`).
     - Dukungan rewrite URL permalinks (`try_files $uri $uri/ /index.php?$args`).
   - **Laravel**:
     - Penegakan docroot ke subdirektori publik (`public/`).
     - Perlindungan mutlak terhadap file konfigurasi lingkungan (`.env`), direktori penyimpanan paket (`vendor/`), dan berkas internal framework lainnya.
     - Penanganan front-controller (`try_files $uri $uri/ /index.php?$query_string`).
   - **Plain PHP / Generic**:
     - Eksekusi script standar PHP dengan index fallback dan pemblokiran file tersembunyi (`.*`).
2. **Konkurensi Multi-Tenant & Multi-Versi**:
   - Mendukung skenario di mana Tenant A (Alice) menggunakan PHP 8.1 dan Tenant B (Bob) menggunakan PHP 8.3 secara bersamaan tanpa interferensi socket atau environment.
3. **Perlindungan Ganda (*Defense in Depth*) Isolasi File Antar-Tenant**:
   - **Lapisan 1 (Kernel POSIX DAC)**: Direktori home setiap tenant di-chmod `0700` milik `p_<slug>:panel-tenants`. Script PHP worker berjalan di bawah UID/GID tenant tersebut sehingga kernel memblokir pembacaan ke `/home/<tenant-lain>`.
   - **Lapisan 2 (PHP Runtime `open_basedir`)**: Direktori dipersempit hanya ke direktori home tenant dan direktori sistem wajib (`/tmp`, `/var/tmp`), diterapkan via `php_admin_value` yang bersifat *immutable* sehingga tidak dapat di-override oleh fungsi `ini_set()` dalam script pengguna.
4. **Efisiensi Sumber Daya (*Zero Idle Workers*)**:
   - Model `pm = ondemand` memastikan jika tidak ada request yang masuk, tidak ada worker PHP-FPM yang berjalan, menghasilkan footprint 0 RAM idle.

## Keputusan Arsitektur

1. **Suite Pengujian Unit & Integrasi Isolasi (`tests/isolation/src/php_isolation.rs`)**:
   - Membangun model evaluasi keamanan formal:
     - `evaluate_open_basedir`: Menguji apakah upaya traversal path atau symlink ke luar batas diizinkan atau ditolak.
     - `evaluate_php_worker_cross_read`: Mensimulasikan akses 3 lapis (Home Traversal DAC -> File Access DAC -> `open_basedir`).
     - `evaluate_fpm_socket_access`: Memastikan hak akses socket UDS `0660` hanya bisa diakses oleh `www-data` dan owner tenant.
   - Skenario pengujian yang tercakup:
     - `test_wordpress_preset_vhost_and_security_rules`: Memastikan blok regex Nginx untuk `wp-config.php` dan `uploads/*.php` aktif.
     - `test_laravel_preset_vhost_and_docroot_containment`: Memastikan docroot `public/` dan proteksi `.env`/`vendor`.
     - `test_plain_php_generic_preset`: Verifikasi konfigurasi standar PHP.
     - `test_concurrent_multi_tenant_multi_version_php_fpm`: Verifikasi konfigurasi pool Alice (8.1) dan Bob (8.3) terisolasi secara sempurna.
     - `test_cross_tenant_file_read_prevention`: Membuktikan Alice tidak dapat membaca file Bob.
     - `test_open_basedir_enforces_php_admin_value_immutability`: Memastikan direktif `open_basedir` ditulis dengan prefix `php_admin_value`.

2. **Integration Test API Level (`crates/panel-api/tests/php_compatibility_and_isolation_test.rs`)**:
   - Menguji pembuatan situs WordPress dan Laravel melalui REST API.
   - Memverifikasi penerapan preset dan konfigurasi vhost pada filesystem Nginx dan FPM.
   - Memverifikasi pengoperasian simultan dua penyewa dengan versi runtime berbeda.

3. **Verifikasi Real-Kernel di VM (`tests/isolation/vm_runner.sh`)**:
   - Menambahkan Step 7/7 pada skrip runner dev VM Ubuntu 24.04:
     - Membuat user Linux terisolasi `p_iso_alice` dan `p_iso_bob`.
     - Menulis file kredensial rahasia di direktori masing-masing.
     - Menjalankan perintah PHP CLI sebagai Bob dengan pembatasan `open_basedir` dan membuktikan bahwa percobaan pembacaan `wp-config.php` milik Alice menghasilkan penolakan izin (*Permission denied*).

## Konsekuensi

### Positif
- **Jaminan Keamanan Tingkat Tinggi**: Kombinasi POSIX DAC dan `open_basedir` menjamin situs yang terinfeksi malware tidak dapat menyusup ke data pelanggan lain (*shared hosting bypass prevention*).
- **Kompatibilitas Luas**: Dua CMS/framework terbesar di ekosistem web (WordPress & Laravel) terbukti berjalan mulus dengan standar keamanan terbaik.
- **Otomatisasi Uji VM**: Tim pengembang dapat memvalidasi seluruh perilaku isolasi nyata dengan menjalankan satu skrip di dev VM (`sudo ./tests/isolation/vm_runner.sh`).
