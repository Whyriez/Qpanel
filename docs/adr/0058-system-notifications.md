# ADR 0058: System Notifications, In-App Notification Center, and Optional SMTP Delivery (P8-T7)

## Status
Accepted

## Konteks
Sesuai PRD Bagian 8 (Fase 8 P8-T7: Notifikasi sistem, line 831):
1. **Pusat Notifikasi (In-App Notification Center & Unread Badge)**:
   - Pengguna dan administrator memerlukan pusat notifikasi in-app terpadu di header antarmuka (Shell) dengan badge jumlah pesan yang belum dibaca (`unread_count`).
   - Notifikasi mendukung filter (Semua, Belum dibaca), status dibaca (`is_read`), tindakan menandai dibaca per-item, menandai semua dibaca (`mark_all_read`), menghapus notifikasi, dan membersihkan notifikasi yang sudah dibaca (`clear_read`).
   - Deep-linking: Notifikasi menyediakan `action_url` opsional (misalnya navigasi langsung ke halaman detail situs, SSL, atau tenant saat diklik).
2. **Kategori Peringatan Kritis Otomatis**:
   - Peringatan sertifikat SSL hampir kedaluwarsa (`ssl_expiring`), dengan ambang batas hari yang dapat dikonfigurasi (`ssl_warning_days`, default 14 hari).
   - Peringatan kuota disk tenant melebihi batas (`quota_warning`), dengan ambang batas persentase yang dapat dikonfigurasi (`quota_warning_percent`, default 90%).
   - Peringatan kegagalan pekerjaan latar belakang (`job_failed` / `backup_failed`).
   - Peringatan kegagalan deployment Git (`deploy_failed`).
3. **Pengiriman Email Eksternal Opsional via SMTP**:
   - Kemampuan mengirimkan notifikasi penting langsung ke email administrator melalui protokol SMTP standar (RFC 5321).
   - Mendukung enkripsi `STARTTLS` (port 587 default), implicit `TLS`/SSL (port 465), dan plaintext (`none`, port 25).
   - Fitur uji coba pengiriman email (`test_smtp_connection`) untuk memverifikasi kredensial dan koneksi server SMTP dari UI modal pengaturan.
4. **Isolasi Arsitektur & Keamanan (Rule 3/4)**:
   - `panel-api` tidak berjalan sebagai root. Agent berkomunikasi via IPC bertipe (`AgentClient`), bukan command shell mentah.
   - Kredensial kata sandi SMTP tidak pernah dikembalikan secara plaintext di API (menggunakan placeholder `********` masked).
   - Evaluasi peringatan otomatis (`evaluate_system_alerts`) dapat dipicu secara berkala atau manual oleh admin tanpa mengunci thread HTTP.

---

## Keputusan Arsitektur

### 1. Model Domain & Validasi Keamanan (`crates/panel-core/src/notification.rs`)
- **Tingkat Keparahan (`NotificationSeverity`)**:
  - `Info`: Informasi operasional umum.
  - `Warning`: Peringatan yang memerlukan perhatian (misalnya kuota > 90%, SSL < 14 hari).
  - `Critical`: Kejadian kegagalan sistem penting (misalnya SSL expired hari ini, disk 100%, deploy gagal).
- **Kategori Notifikasi (`NotificationCategory`)**:
  - `SslExpiring`: `ssl_expiring`.
  - `QuotaWarning`: `quota_warning`.
  - `JobFailed`: `job_failed`.
  - `DeployFailed`: `deploy_failed`.
  - `BackupFailed`: `backup_failed`.
  - `SecurityAlert`: `security_alert`.
  - `System`: `system`.
- **Enkripsi SMTP (`SmtpEncryption`)**:
  - `Starttls`: Port 587 default.
  - `Tls`: Port 465 implicit SSL/TLS.
  - `None`: Port 25 plaintext.
- **Validasi Keamanan Ketat**:
  - `validate_notification_title`: Judul 1 - 200 karakter.
  - `validate_notification_message`: Pesan 1 - 4000 karakter.
  - `validate_smtp_host`: Mencegah injeksi karakter ilegal (`\s`, `/`, `\`, `:`).
  - `validate_smtp_port`: Rentang port TCP 1 - 65535.
  - `validate_email_address`: Format alamat email standar (memiliki `@` dan domain valid).
  - `validate_ssl_warning_days`: Rentang ambang batas 1 - 90 hari.
  - `validate_quota_warning_percent`: Rentang ambang batas persentase 50 - 99%.
- **Email Formatter**:
  - `format_alert_email_subject` dan `format_alert_email_body` untuk menghasilkan format email teks bersih dengan identitas Nusa Panel.

### 2. Skema Basis Data & Repositori (`crates/panel-db`)
- **Migrasi `20261003000012_notifications.sql`**:
  - Tabel `notifications`: `id` (UUID text primary key), `user_id` (nullable, NULL = siaran sistem/admin), `category`, `severity`, `title`, `message`, `action_url`, `is_read`, `read_at`, `created_at`.
  - Indeks performa: `idx_notifications_user_is_read`, `idx_notifications_created_at`, `idx_notifications_category`.
  - Tabel `notification_settings`: Konfigurasi singleton (`id = 1`) menyimpan `smtp_enabled`, `smtp_host`, `smtp_port`, `smtp_username`, `smtp_password_enc`, `smtp_encryption`, `from_address`, `from_name`, `admin_email`, `ssl_warning_days`, `quota_warning_percent`, `notify_ssl_expiring`, `notify_quota_warning`, `notify_job_failed`, `notify_deploy_failed`, `updated_at`.
- **Repositori `SqliteNotificationRepository`**:
  - `create`: Menambahkan rekaman notifikasi baru.
  - `find_by_id`: Mengambil notifikasi berdasarkan UUID.
  - `list`: Paginasi dan pemfilteran berdasarkan `unread_only`, `category`, `severity`, serta pemisahan hak akses admin vs tenant.
  - `unread_count`: Menghitung total unread untuk pengguna/admin.
  - `mark_as_read` & `mark_all_read`: Mengubah flag `is_read = 1` dan mencatat `read_at`.
  - `delete` & `clear_all`: Menghapus notifikasi individual atau pembersihan massal yang sudah dibaca.
  - `get_settings` & `update_settings`: Membaca dan memperbarui pengaturan SMTP serta ambang batas.

### 3. Klien SMTP Asinkron Ringan (`crates/panel-api/src/mail.rs`)
- Mengimplementasikan alur protokol RFC 5321 asinkron di atas `tokio::net::TcpStream` dan `tokio_rustls`:
  - `EHLO` handshake.
  - Upgrade STARTTLS / TLS langsung dengan `rustls` (menggunakan sertifikat webpki/native roots).
  - Autentikasi `AUTH LOGIN` dengan encoding Base64.
  - Pengiriman amplop `MAIL FROM:<...>` dan `RCPT TO:<...>`.
  - Penulisan konten `DATA` dengan dot-stuffing RFC 5321 (mengganti baris yang diawali titik `.` dengan `..`) dan terminasi CRLF `.`.
  - Penutupan sesi `QUIT`.
- Diuji unit test mandiri menggunakan mock SMTP TCP listener server.

### 4. Layanan Evaluasi & Pengiriman Notifikasi (`NotificationService`)
- Terletak di `crates/panel-api/src/notifications/service.rs`:
  - `dispatch`: Menyimpan rekaman notifikasi ke SQLite dan memicu pengiriman email latar belakang jika SMTP diaktifkan di pengaturan.
  - `evaluate_system_alerts`:
    - Mengambil sertifikat yang mendekati kedaluwarsa via `SqliteCertificateRepository::find_expiring_certificates`.
    - Mengambil kuota disk semua tenant melalui IPC `AgentClient::get_quota` dan memeriksa persentase penggunaan terhadap `plan.disk_limit_bytes`.
    - Menghasilkan notifikasi `ssl_expiring` dan `quota_warning` dengan tingkat keparahan yang tepat (`critical` vs `warning`).
- Diintegrasikan ke `Worker` (`crates/panel-api/src/jobs/worker.rs`) untuk otomatis mendispatch notifikasi saat pekerjaan latar belakang (`deploy`, `backup`, `restore`) mengalami kegagalan permanen.

### 5. REST API Endpoints (`crates/panel-api/src/notifications/routes.rs`)
- `GET /api/v1/notifications`: Daftar notifikasi dengan filter dan paginasi.
- `GET /api/v1/notifications/unread-count`: Jumlah badge unread.
- `PATCH /api/v1/notifications/{id}/read`: Tandai dibaca per-item.
- `POST /api/v1/notifications/mark-all-read`: Tandai semua dibaca.
- `DELETE /api/v1/notifications/{id}`: Hapus notifikasi.
- `POST /api/v1/notifications/clear`: Bersihkan notifikasi yang sudah dibaca.
- `GET /api/v1/notifications/settings`: Ambil konfigurasi SMTP & ambang batas.
- `PUT /api/v1/notifications/settings`: Perbarui konfigurasi (admin only, CSRF protected).
- `POST /api/v1/notifications/test-smtp`: Uji koneksi pengiriman email SMTP (admin only, CSRF protected).
- `POST /api/v1/notifications/evaluate-alerts`: Pemicu manual evaluasi peringatan sistem (admin only, CSRF protected).

### 6. Antarmuka Pengguna (`web/src/components/notifications/`)
- `NotificationBell.tsx`:
  - Menggantikan ikon Bell statis di `Shell.tsx`.
  - Badge unread merah interaktif yang diperbarui otomatis setiap 30 detik.
  - Popover dropdown dengan tab filter (Semua, Belum dibaca), status item, deep-link navigation, tombol aksi instan, dan tombol evaluasi langsung.
- `NotificationSettingsModal.tsx`:
  - Modal pengaturan admin dengan dua tab: "Koneksi SMTP" dan "Ambang Batas & Peringatan".
  - Uji coba koneksi SMTP langsung dari antarmuka dengan umpan balik pesan sukses/gagal.
  - Sliders untuk ambang batas hari kedaluwarsa SSL dan persentase kuota disk.

---

## Konsekuensi
- **Positif**:
  - Admin dan pengguna langsung mengetahui masalah krusial (SSL kedaluwarsa, kuota disk penuh, deploy gagal) sebelum terjadi downtime layanan.
  - Fitur email fleksibel: dapat dinonaktifkan jika server tidak memiliki gateway SMTP, atau diaktifkan untuk pengiriman instan.
  - Tidak ada dependency crate eksternal berat baru untuk SMTP; implementasi RFC 5321 bersih di atas Tokio dan Tokio-Rustls yang sudah ada.
  - Sesuai standar desain PRD bagian 8 dengan token warna HSL terkurasi, salinan bahasa Indonesia yang jelas, dan aksesibilitas keyboard (Escape / Click Outside).
- **Mitigasi**:
  - Untuk mencegah spam notifikasi redundan saat evaluasi dipanggil berulang, perbandingan dilakukan pada entitas yang belum memiliki notifikasi aktif yang belum dibaca.
