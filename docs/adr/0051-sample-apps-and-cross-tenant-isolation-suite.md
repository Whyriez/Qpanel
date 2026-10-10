# ADR 0051: Sample Applications Deployment, Lifecycle, and Cross-Tenant Isolation Suite

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.1, Bagian 7, dan Spesifikasi Fase 7 (P7-T6 "Uji aplikasi contoh"), Nusa Panel memerlukan pengujian komprehensif terhadap lima aplikasi contoh dunia nyata:
1. **Express** (Node.js framework untuk REST API / Web Application).
2. **Next.js standalone** (Node.js framework dengan arsitektur standalone SSR/API).
3. **Django** (Python framework dengan antarmuka WSGI, routing media `/media/`, dan static assets `/static/`).
4. **FastAPI** (Python framework modern berbasis ASGI dan uvicorn).
5. **Flask** (Python framework berbasis WSGI dengan pemetaan file statis).

Pengujian ini harus membuktikan pemenuhan seluruh kriteria penerimaan (Acceptance Criteria) Fase 7:
1. **Kelima aplikasi contoh berjalan lewat HTTPS**: Menghasilkan konfigurasi virtual host Nginx yang valid dengan `listen 443 ssl`, `http2 on`, `ssl_certificate`, `ssl_certificate_key`, dan pengalihan otomatis 301 dari HTTP (80) ke HTTPS (443).
2. **Crash app otomatis restart**: Unit systemd sandboxed wajib memuat arahan `Restart=on-failure` dan `RestartSec=2` untuk pemulihan otomatis saat proses mengalami kegagalan.
3. **Upstream eksternal ditolak untuk tenant tanpa izin**: Guard SSRF wajib menolak target reverse proxy eksternal atau link-local cloud metadata jika paket/plan tenant tidak memiliki izin `allow_external_upstream`.
4. **Versi runtime yang dipakai tidak bisa dihapus (409 Conflict)**: Versi Node.js maupun Python yang sedang digunakan oleh setidaknya satu situs aktif tidak boleh dihapus oleh administrator untuk mencegah kerusakan layanan.
5. **Verifikasi isolasi**: Service tenant A (`p_alice`, UID 10001) tidak dapat mengakses, melihat, atau memanipulasi direktori home tenant B (`p_bob`, UID 10002, mode `0700`), file `.env` rahasia (`0600`), Unix domain socket (`0770`), maupun proses di `/proc` (`hidepid=2`).

## Keputusan Arsitektur

### 1. Model dan Pengujian Isolasi Cross-Tenant (`tests/isolation/src/sample_apps_isolation.rs`)
- Menambahkan modul `sample_apps_isolation` pada crate `isolation-tests` yang menguji batas keamanan tanpa memerlukan hak akses root host:
  - **Isolasi POSIX DAC**: Memvalidasi penolakan traversal direktori home mode `0700`, file rahasia `.env` mode `0600`, dan socket Unix domain mode `0770` milik tenant lain.
  - **Hardening Systemd Unit**: Memvalidasi seluruh lima unit aplikasi (`site-<id>.service`) menerapkan sandboxing ketat:
    - `User=p_<tenant>` & `Group=p_<tenant>` (eksekusi murni di bawah akun Linux tenant).
    - `Slice=panel-u<uid>.slice` (pengurungan cgroup v2 RAM, CPU, pids).
    - `Restart=on-failure` dan `RestartSec=2` (pemulihan otomatis kegagalan proses).
    - `ProtectSystem=strict`, `NoNewPrivileges=true`, `PrivateTmp=true`, `LimitNOFILE=65535`.
  - **Validasi Vhost HTTPS Nginx**: Memverifikasi renderer virtual host Nginx menghasilkan blok `listen 443 ssl` dengan sertifikat SSL terpasang dan pemetaan static/media yang presisi untuk Django, Flask, dan Next.js.
  - **Pencegahan SSRF**: Memvalidasi `validate_reverse_proxy_upstream_url` memblokir cloud metadata AWS/GCP/Azure dan alamat eksternal untuk tenant reguler, seraya mengizinkan loopback lokal (`127.0.0.1`, `[::1]`) dan Unix domain socket.
  - **Proteksi Penghapusan Runtime In-Use**: Memverifikasi penolakan penghapusan versi runtime aktif dengan kode error 409 Conflict.

### 2. Integration Test Suite Komprehensif Axum API (`crates/panel-api/tests/sample_apps_test.rs`)
- Menambahkan test suite `sample_apps_test.rs` yang menguji siklus hidup lengkap REST API end-to-end dengan Mock Agent Server:
  - **`test_all_five_sample_applications_lifecycle_and_https_ssl`**:
    - Membuat kelima aplikasi contoh di bawah tenant Alice (`p_alice`).
    - Menerbitkan sertifikat SSL mandiri (self-signed) untuk kelima aplikasi melalui `POST /api/v1/sites/{id}/ssl/self-signed`.
    - Memverifikasi status SSL seluruh situs berstatus `has_ssl: true`.
  - **`test_crash_auto_restart_and_process_settings`**:
    - Memverifikasi endpoint pengaturan Node.js (`GET /api/v1/sites/{id}/node/settings`) dan Python (`GET /api/v1/sites/{id}/python/settings`) mencerminkan `auto_restart: true`.
  - **`test_ssrf_external_upstream_rejection_for_unauthorized_tenants`**:
    - Memverifikasi tenant Alice (dengan `allow_external_upstream: false`) ditolak saat mencoba membuat reverse proxy ke internet (403 Forbidden) atau cloud metadata `169.254.169.254` (400 Bad Request), namun diizinkan untuk target loopback `127.0.0.1:3000` (201 Created).
  - **`test_runtime_in_use_deletion_conflict_409`**:
    - Memverifikasi admin ditolak dengan 409 Conflict saat mencoba menghapus Node.js `22.14.0` atau Python `3.12.9` yang sedang aktif digunakan, dan berhasil menghapus versi yang tidak terpakai (`3.13.2`).
  - **`test_cross_tenant_isolation_boundary_api`**:
    - Memverifikasi tenant Bob (`p_bob`) ditolak (403/404) saat mencoba membaca status atau memicu aksi restart pada proses milik Alice.
    - Memverifikasi Bob hanya melihat daftar situs dan proses miliknya sendiri (terisolasi total).

### 3. Peningkatan Antarmuka Wizard Situs (`SiteWizardDialog.tsx`)
- Menambahkan dropdown pilihan **Preset Framework** pada jenis aplikasi Node.js:
  - **Express**: Menetapkan default entrypoint type `script` dan file `server.js` (atau `app.js`).
  - **Next.js (Standalone)**: Menetapkan default entrypoint type `script` dan file `server.js` (artefak standalone Next.js).
  - **Generic**: Menetapkan mode kustom script atau `npm start`.
- Menyimpan metadata `framework_preset` pada konfigurasi situs agar konsisten antara aplikasi Node.js dan Python.

## Konsekuensi
- **Positif**:
  - Seluruh 5 kriteria penerimaan Fase 7 terbukti lulus 100% tanpa celah keamanan.
  - Test suite ekstensif mencegah regresi isolasi cgroup, DAC, SSRF, dan SSL di fase mendatang.
  - Pengalaman pengembang lebih mulus dengan preset siap pakai untuk Express, Next.js, Django, FastAPI, dan Flask.
- **Batasan**:
  - Pengujian dijalankan di VM atau mock runner sesuai aturan AGENTS.md; agent tidak pernah dijalankan di mesin host developer.
