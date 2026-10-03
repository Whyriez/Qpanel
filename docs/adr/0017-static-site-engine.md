# ADR 0017: Static Site Engine and Lifecycle Management

## Status
Accepted

## Context
Setelah model domain/site (P3-T1), baseline Nginx (P3-T2), dan pipeline atomic apply dengan rollback (P3-T3) tersedia, sistem membutuhkan modul mesin website statis lengkap (`P3-T4`).

Sesuai PRD Bagian 4.7 dan task `P3-T4`:
1. Pengguna atau admin dapat membuat situs statis baru dengan domain terdaftar.
2. Docroot situs dialokasikan di dalam home directory tenant terisolasi: `/home/p_<slug>/sites/<domain>/public`.
3. Karena direktori home tenant berizin `0700`, webserver Nginx (`www-data`) membutuhkan traversal POSIX ACL (`u:www-data:--x`) pada home directory dan hak baca-eksekusi (`u:www-data:r-x`) pada docroot situs agar berkas publik dapat diakses tanpa melonggarkan izin ke pengguna lain di sistem.
4. Ketika docroot baru dibuat, sistem menyajikan berkas placeholder `index.html` bawaan yang modern, responsif, dan berbahasa Indonesia yang jelas.
5. Siklus hidup situs mencakup: pembuatan, penampilan daftar situs terisolasi per tenant, penonaktifan (*disable*/*suspend*), pengaktifan kembali (*enable*/*activate*), dan penghapusan situs berserta konfigurasi webserver.

## Decision
1. **Penyediaan Docroot & Placeholder**:
   - `panel-proto` menambahkan request/response bertipe `SiteProvisionRequest` dan `SiteProvisionResponse`.
   - `panel-agent` mengimplementasikan fungsi `provision_site()`:
     - Membuat hierarki direktori docroot `/home/p_<slug>/sites/<domain>/public`.
     - Mengatur kepemilikan berkas ke UID/GID tenant (`chown -R <uid>:<uid>`).
     - Mengatur mode izin direktori (`0750`) dan izin berkas (`0644`).
     - Memberikan traversal POSIX ACL via `setfacl -m u:www-data:--x <home_dir>` dan `setfacl -R -m u:www-data:r-x <docroot>`.
     - Menulis berkas placeholder `index.html` jika belum ada dengan desain responsif, tipografi sistem modern, dan copy Bahasa Indonesia ramah pengguna.

2. **Mesin API Situs Statis (`panel-api`)**:
   - Dibuat modul `crates/panel-api/src/sites.rs` dengan endpoint RESTful:
     - `POST /api/v1/sites`: Validasi kepemilikan tenant, validasi domain (RFC & Punycode IDN), pengecekan keunikan domain, pembuatan record `sites` & `domains`, provisi docroot via agent, serta rendering template vhost Nginx dan apply atomik.
     - `GET /api/v1/sites`: Menampilkan daftar situs dengan isolasi tenant (admin dapat memfilter berdasarkan `user_id`, pengguna biasa hanya melihat miliknya).
     - `GET /api/v1/sites/{id}`: Menampilkan detail situs beserta domain yang terhubung dan status SPA fallback.
     - `POST /api/v1/sites/{id}/enable`: Memperbarui status database ke `active`, membuat kembali symlink di `sites-enabled/`, memverifikasi sintaks, dan mereload Nginx.
     - `POST /api/v1/sites/{id}/disable`: Memperbarui status database ke `suspended`, mencabut symlink di `sites-enabled/`, dan mereload Nginx tanpa menghapus konfigurasi dasar.
     - `DELETE /api/v1/sites/{id}`: Menghapus konfigurasi Nginx via agent, menghapus data situs dan domain dari database secara bertingkat (*cascading*), serta mencatat jejak audit.

3. **Autentikasi, Otorisasi, dan Jejak Audit**:
   - Seluruh endpoint dilindungi oleh `AuthenticatedUser` extractor yang menjamin sesi aktif dan valid.
   - Pengecekan RBAC diterapkan: regular tenant hanya dapat mengelola situs miliknya sendiri; administrator dapat mengelola situs seluruh tenant.
   - Setiap mutasi (`site.create`, `site.enable`, `site.disable`, `site.delete`) dicatat secara asinkron ke tabel `audit_logs` menggunakan `state.audit_writer.log_async`.

4. **Pengujian Integrasi**:
   - Seluruh siklus hidup situs diuji secara komprehensif melalui integration test HTTP & IPC `crates/panel-api/tests/static_site_api_test.rs`.

## Consequences
- Penyediaan situs statis berjalan mulus dari satu panggilan API yang mengorkestrasi izin host, direktori docroot, template Nginx, status database, dan pencatatan audit.
- Hak akses berkas tetap aman berkat POSIX ACL: tenant tetap terisolasi satu sama lain dalam mode `0700`, sementara proses Nginx `www-data` tetap dapat melayani file publik dengan efisien.
