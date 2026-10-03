# ADR 0016: Atomic Config Apply, Validation Pipeline, and Automatic Rollback

## Status
Accepted

## Context
Perubahan konfigurasi virtual host Nginx (pembuatan site baru, perubahan domain, pengaktifan SSL, modifikasi header/SPA) berisiko menyebabkan *service outage* total apabila terdapat kesalahan sintaks atau direktif yang tidak valid. Apabila Nginx di-reload dengan file konfigurasi yang rusak, proses Nginx master akan menolak konfigurasi tersebut atau mengalami kegagalan, dan situs lain di server dapat terganggu.

Selain itu, rentan terjadi serangan *config injection* apabila domain, nama path, atau header HTTP disisipkan ke dalam file konfigurasi tanpa sanitasi ketat (misalnya domain berisi `; return 403;` atau karakter escape `\n` dan `$`).

Sesuai PRD Bagian 4.7, Bagian 6, dan task `P3-T3`:
1. Templating vhost menggunakan `minijinja` dengan filter anti-injeksi ketat.
2. Pipeline apply harus atomik: render ke direktori `staging/` → uji sintaks via `nginx -t` → swap ke `sites-available/` dan link ke `sites-enabled/` → reload dengan debounce (500 ms).
3. Apabila pengujian sintaks `nginx -t` gagal, pipeline harus melakukan **rollback otomatis** mengembalikan konfigurasi lama yang berfungsi (atau menghapus file baru jika situs belum pernah ada), sehingga Nginx tidak pernah mati atau berada dalam status korup.
4. Diuji dengan tes golden, tes injeksi domain jahat, dan tes rollback dengan konfigurasi sengaja dirusak.

## Decision
1. **Renderer `minijinja` & Anti-Injeksi**:
   - `NginxTemplateRenderer` di `crates/panel-webserver/src/template.rs` merender template virtual host Nginx.
   - Fungsi `validate_vhost_inputs()` memverifikasi seluruh parameter sebelum rendering:
     - Nama domain diverifikasi dengan parser RFC 1034/1035 dan Punycode IDN (`DomainName::parse`).
     - Dilarang keras karakter kontrol dan pemisah direktif Nginx: `;`, `{`, `}`, `$`, `\n`, `\r`, `\0`, `"`, dan spasi.
     - Parameter path (`docroot`, `log_dir`, `snippets_dir`, `ssl_cert`, `ssl_key`) dan header HTTP divalidasi ketat.

2. **Pipeline Apply Atomik (`NginxApplyPipeline`)**:
   - `staging_dir` (`/etc/panel/nginx/staging/`): file hasil render ditulis ke `.tmp` lalu di-rename ke `<site_id>.conf`.
   - `backup_dir` (`/etc/panel/nginx/.backup/`): jika file konfigurasi lama sudah ada di `sites-available/`, dibuat cadangan atomik ke `.backup/<site_id>.conf.bak`.
   - File baru di-swap ke `sites-available/<site_id>.conf`.
   - Bila `enable == true`, symlink dibuat di `sites-enabled/<site_id>.conf`.
   - Validator sintaks `nginx -t -c <nginx_conf_path>` dijalankan.

3. **Mekanisme Automatic Rollback**:
   - Jika validator sintaks mengembalikan status gagal (`exit code != 0`):
     - Pipeline segera mengembalikan konfigurasi lama dari `.backup/<site_id>.conf.bak` ke `sites-available/<site_id>.conf` dan memperbaiki symlink `sites-enabled`.
     - Jika merupakan situs baru, seluruh file yang baru dibuat di `sites-available` dan `sites-enabled` dihapus bersih.
     - Seluruh file sementara di `staging/` dibersihkan.
     - Error dikembalikan sebagai `WebserverError::ValidationFailed { stderr, rolled_back: true }`.

4. **Reload Debounce (`ReloadDebouncer`)**:
   - Diterapkan window debounce 500 ms.
   - Mutasi konfigurasi yang terjadi beruntun dalam rentang 500 ms digabungkan menjadi satu operasi reload service `systemctl reload nginx` tanpa memicu badai reload (*reload storm*).

5. **Integrasi IPC**:
   - Ditambahkan `SiteApplyRequest` dan `SiteAppliedResponse`, serta `SiteRemoveRequest` dan `SiteRemovedResponse` pada `panel-proto`.
   - `panel-agent` menangani permintaan melalui `NginxManager::apply_site` dan `NginxManager::remove_site`.

## Consequences
- Keamanan host terjamin: tidak ada konfigurasi rusak yang dapat mencederai Nginx yang sedang berjalan.
- Upaya injeksi konfigurasi melalui domain berbahaya ditolak sejak tahap validasi sebelum menyentuh filesystem atau template engine.
- Integrasi sepenuhnya teruji otomatis dengan unit tests (golden test, anti-injection test, rollback test) dan IPC integration test.
