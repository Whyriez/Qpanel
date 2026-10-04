# ADR-0026: Pergantian Versi PHP Situs Zero-Downtime dan Automatic Rollback

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.1 dan Bagian 11 (Task `P4-T5`), platform hosting multi-tenant modern memerlukan kemampuan untuk mengganti versi runtime PHP per situs secara independen tanpa menimbulkan downtime bagi pengunjung website (*zero-downtime cutover*).

Pada arsitektur Vpanel:
1. **Isolasi Master PHP-FPM Multi-Tenant**:
   - Setiap tenant memiliki satu proses master PHP-FPM per versi PHP yang aktif (`panel-fpm@<linux_username>-<version>.service`).
   - Nginx berkomunikasi dengan pool tenant via Unix Domain Socket terisolasi: `/run/panel/php/<linux_username>-<version>.sock`.
2. **Kebutuhan Zero-Downtime**:
   - Jika versi PHP diubah secara langsung pada konfigurasi Nginx sebelum pool baru siap, request pengunjung akan mengalami error `502 Bad Gateway`.
   - Oleh karena itu, master pool target harus disiapkan, dimulai, dan diverifikasi kesehatannya sebelum socket upstream Nginx dialihkan.
3. **Kebutuhan Automatic Rollback**:
   - Jika service target gagal start (misal konfigurasi bermasalah atau crash saat startup) atau reload Nginx gagal, sistem harus secara otomatis mengembalikan konfigurasi vhost Nginx ke socket lama, membatalkan pool target, dan mencatat audit log tanpa menimbulkan downtime bagi situs yang berjalan.
4. **Efisiensi Memori (0 Worker Idle)**:
   - Setelah cutover sukses, pool lama harus dibersihkan (`remove_php_fpm_pool`). Jika master versi lama tidak lagi memiliki pool yang tersisa, service master lama otomatis dihentikan dan didisable, membebaskan seluruh alokasi memori ke 0 workers.

## Keputusan Arsitektur

1. **Protokol IPC Health Check PHP-FPM (`panel-proto` & `panel-agent`)**:
   - Menambahkan payload IPC:
     - `RequestPayload::PhpFpmHealthCheck(PhpFpmHealthCheckRequest)`: Membawa informasi `username`, `version`, dan `pool_name`.
     - `ResponsePayload::PhpFpmHealthChecked(PhpFpmHealthCheckResponse)`: Mengembalikan status boolean `healthy`, `service_active`, `socket_exists`, `pool_exists`, dan `message`.
   - Mengimplementasikan `check_php_fpm_health` di `crates/panel-agent/src/php.rs`:
     - Memverifikasi status aktif systemd unit (`systemctl is-active --quiet panel-fpm@<user>-<version>.service`).
     - Memeriksa keberadaan file socket `/run/panel/php/<user>-<version>.sock` dan file konfigurasi pool.
     - Menyediakan metode IPC client `AgentClient::check_php_fpm_health` di `panel-api`.

2. **Orkestrasi Zero-Downtime Cutover dan Rollback (`panel-api`)**:
   - Endpoint: `POST /api/v1/sites/{id}/php/version` (dan `PUT`).
   - Handler: `switch_site_php_version_handler` dengan alur transaksional yang ketat:
     - **Validasi**:
       - Hanya situs bertipe `site.kind == "php"` yang diizinkan (error 400 jika situs statis/proxy).
       - Target versi PHP harus valid dan berbeda dengan versi aktif (error 400).
       - Target versi PHP harus terpasang di sistem (`list_php_runtimes`) dan binary FPM tersedia (error 400).
     - **Step 1: Persiapan Master & Pool Target**:
       - Memastikan master service target aktif via `ensure_php_fpm_master`.
       - Membuat konfigurasi pool target via `apply_php_fpm_pool`.
     - **Step 2: Cutover Socket Nginx**:
       - Melakukan re-render template vhost Nginx PHP atomik mengarah ke socket target `/run/panel/php/<user>-<target_version>.sock`.
       - Menjalankan debounced Nginx reload.
     - **Step 3: Post-Cutover Health Check**:
       - Menguji kesehatan master target via `check_php_fpm_health`.
     - **Step 4: Automatic Rollback saat Terjadi Kegagalan**:
       - Jika cutover atau health check gagal:
         1. Rollback vhost Nginx atomik ke socket versi lama (`/run/panel/php/<user>-<old_version>.sock`).
         2. Hapus pool yang sempat dibuat pada target master (`remove_php_fpm_pool`).
         3. Catat audit event kegagalan dan tindakan rollback.
         4. Kembalikan respons HTTP 500 informatif dengan rincian error dan status `rolled_back_to: old_version`.
     - **Step 5: Komit & Pembersihan Pool Lama**:
       - Jika health check sukses:
         1. Hapus pool dari master lama (`remove_php_fpm_pool`), yang secara otomatis menghentikan master lama jika pool tersisa adalah 0.
         2. Perbarui record database `sites.version = new_version` dan `sites.updated_at`.
         3. Catat audit event sukses `site.php_version_switched`.
         4. Kembalikan respons HTTP 200 `SwitchPhpVersionResponse`.

3. **Integrasi Antarmuka Web (`web/`)**:
   - Pembaruan tipe `Site` dengan field `runtime`, `version`, dan `preset`.
   - Penambahan kartu *Runtime PHP* di tab *Overview* halaman detail situs (`SiteDetailPage.tsx`) yang menampilkan versi aktif, preset aplikasi, dan tombol "Ganti Versi".
   - Penambahan modal *Ganti Versi PHP* (`SwitchPhpVersionModal`):
     - Memuat daftar versi PHP terpasang dari backend (`fetchPhpVersions`).
     - Menyajikan informasi transparan mengenai mekanisme zero-downtime dan automatic rollback.
     - Menjalankan mutasi via `switchSitePhpVersion(siteId, version)` dengan umpan balik toast sukses/error.

## Konsekuensi

### Positif
- **Zero-Downtime**: Request pengunjung situs web tidak terputus karena master target telah berjalan sebelum Nginx dialihkan.
- **Fail-Safe & Tahan Rusak**: Jika master versi baru mengalami error, rollback instan mengembalikan situs ke versi lama tanpa intervensi manual administrator.
- **Efisiensi Sumber Daya**: Pool lama dibersihkan dan master lama dihentikan secara otomatis jika tidak ada situs lain milik tenant yang menggunakannya (RAM 0 worker idle).
- **Audit Terperinci**: Seluruh percobaan pergantian versi, baik yang berhasil maupun yang memicu rollback, tercatat lengkap pada audit log.

### Pertimbangan
- Proses pergantian versi membutuhkan beberapa tahapan IPC serial (ensure master -> apply pool -> nginx reload -> health check -> cleanup pool). Menggunakan debounced reload dan socket check lokal memastikan keseluruhan alur selesai dalam rentang < 1-2 detik.
