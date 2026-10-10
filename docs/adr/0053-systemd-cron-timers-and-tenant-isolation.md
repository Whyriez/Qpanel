# ADR 0053: Systemd Timer Cron Jobs, Bilingual Human Translator, and Tenant Isolation (P8-T2)

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.6 ("Cron job / scheduled task"), Bagian 6 (Keamanan), dan Bagian 8 (Fase 8 P8-T2):
1. **Keamanan & Isolasi Tenant**:
   - Cron daemon tradisional (`/etc/crontab` atau spool cron root) tidak digunakan karena tidak memiliki isolasi resource cgroup, tidak terintegrasi ke systemd slices, dan sulit dipantau per-tenant.
   - Setiap scheduled job dipetakan ke pasangan unit systemd per-job: `site-<site_id>-cron-<job_id>.service` dan `site-<site_id>-cron-<job_id>.timer`.
   - Timer dan service berjalan di bawah identitas Linux tenant (`User=p_<tenant>`, `Group=p_<tenant>`), dan berada di dalam cgroup slice tenant (`Slice=panel-u<uid>.slice`).
   - Direktif sandboxing ketat diterapkan: `Type=oneshot`, `LimitNOFILE=65535`, `ProtectSystem=strict`, `ProtectHome=read-only`, `ReadWritePaths=/home/<username>`, `PrivateTmp=true`, `PrivateDevices=true`, `NoNewPrivileges=true`, `ProtectKernelTunables=true`, `ProtectKernelModules=true`, `ProtectControlGroups=true`, `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`.
   - `Type=oneshot` secara inheren mencegah eksekusi bersamaan/tumpang tindih (*prevent overlapping runs*) karena systemd tidak akan memicu aktivasi baru jika service oneshot sedang berjalan.
2. **5-Part Cron Parser & Bilingual Human Translator**:
   - Format cron standar 5 bagian: `min hour dom month dow`.
   - Translasi otomatis ke spesifikasi `systemd.time` `OnCalendar` (contoh: `*/15 * * * *` -> `*-*-* *:00/15:00`).
   - Terjemahan bahasa manusia bilingual: Bahasa Indonesia dan English (contoh: "Setiap 15 menit" / "Every 15 minutes").
3. **Pembatasan Kuota & Eksekusi Instan (Run Now)**:
   - Pemeriksaan batas kuota hosting plan (`max_cron_jobs`) sebelum membuat job baru.
   - Fitur "Run Now" untuk mengeksekusi job secara instan di luar jadwal dan mencatat metrik ke database `cron_runs` (duration ms, exit code, stdout/stderr).
   - Pembacaan log systemd journal per-unit service (`journalctl -u site-<site_id>-cron-<job_id>.service`).
4. **Antarmuka Pengguna (UI)**:
   - Tab `Cron` pada `SiteDetailPage.tsx`.
   - Builder interaktif dengan preset cepat (setiap 5m, 15m, jam, harian 00:00, 02:00, mingguan, bulanan) serta input manual 5-field.
   - Pratinjau terjemahan bahasa manusia dan representasi `OnCalendar` systemd secara real-time.
   - Modal riwayat eksekusi dan log viewer systemd journal.

---

## Keputusan Arsitektur

### 1. Parser Cron & Translator Human (`panel-core::cron`)
- Menyediakan fungsi validasi `validate_cron_expression(expr: &str) -> Result<ParsedCron, CoreError>` yang memeriksa rentang angka untuk masing-masing bagian:
  - Menit: 0-59
  - Jam: 0-23
  - Tanggal/DOM: 1-31
  - Bulan: 1-12 (mendukung nama `jan` - `dec`)
  - Hari/DOW: 0-7 (0 & 7 = Sunday, mendukung nama `sun` - `sat`)
- Mengonversi ekspresi ke `systemd.time` `OnCalendar`:
  - `*/N * * * *` -> `*-*-* *:00/N:00`
  - `0 * * * *` -> `*-*-* *:00:00`
  - `M H * * *` -> `*-*-* HH:MM:00`
  - Pola kustom diformat ke format tanggal dan jam systemd `*-MM-DD HH:MM:00`.
- Menyediakan terjemahan dwibahasa `cron_to_human_id` dan `cron_to_human_en`.

### 2. Skema Database SQLite (`panel-db`)
- Tabel `cron_jobs`:
  - `id`, `site_id`, `name`, `command`, `schedule`, `systemd_calendar`, `human_description`, `human_description_en`, `work_dir`, `timeout_secs`, `enabled`, `last_run_at`, `last_duration_ms`, `last_exit_code`, `status`, `created_at`, `updated_at`.
- Tabel `cron_runs`:
  - `id`, `cron_job_id`, `site_id`, `started_at`, `finished_at`, `duration_ms`, `exit_code`, `status`, `output_tail`.
- Repository `SqliteCronRepository` dengan metode atomik CRUD, penghitungan jumlah job aktif per situs, dan pencatatan run logs.

### 3. IPC Protobuf & Systemd Management di Agent (`panel-proto` & `panel-agent`)
- Pesan IPC:
  - `CronJobApplyRequest` / `CronJobApplyResponse`: Menulis file unit `.service` dan `.timer`, me-reload systemd daemon, dan mengaktifkan timer.
  - `CronJobRemoveRequest` / `CronJobRemoveResponse`: Menghentikan dan menonaktifkan timer/service, serta menghapus file unit.
  - `CronJobRunNowRequest` / `CronJobRunNowResponse`: Menjalankan service dengan timeout dan isolasi hak akses Linux tenant, mengembalikan durasi, exit code, dan stdout/stderr.
  - `CronJobLogsReadRequest` / `CronJobLogsReadResponse`: Membaca log journalctl dari unit service.
- Struktur `CronServiceUnitConfig` membatasi argumen dan menerapkan sandboxing ketat ke direktif unit systemd.

### 4. API Handlers & Tenant Isolation (`panel-api::cron`)
- Endpoints:
  - `GET /api/v1/sites/{id}/cron`: Daftar cron job beserta kuota hosting plan tenant.
  - `POST /api/v1/sites/{id}/cron`: Validasi ekspresi, cek kuota `max_cron_jobs`, insert ke DB, panggil agent `apply_cron_job`, catat audit log.
  - `POST /api/v1/sites/{id}/cron/validate`: Validasi ekspresi dan menghasilkan terjemahan tanpa menyimpan ke DB.
  - `GET /api/v1/sites/{id}/cron/{job_id}`: Detail cron job.
  - `PUT /api/v1/sites/{id}/cron/{job_id}`: Update konfigurasi job dan sinkronisasi ke agent.
  - `DELETE /api/v1/sites/{id}/cron/{job_id}`: Hapus job dan hapus unit systemd di agent.
  - `POST /api/v1/sites/{id}/cron/{job_id}/run`: Eksekusi instan di agent, catat metrik ke `cron_runs` dan update `last_run_*` pada `cron_jobs`.
  - `GET /api/v1/sites/{id}/cron/{job_id}/runs`: Riwayat eksekusi job.
  - `GET /api/v1/sites/{id}/cron/{job_id}/logs`: Log journalctl systemd.
- Isolasi tenant diverifikasi secara ketat: Tenant non-admin tidak dapat mengakses atau memodifikasi cron job milik tenant lain (mengembalikan 403 Forbidden).

### 5. Frontend Web UI (`web/src/components/sites/SiteCronTab.tsx`)
- Komponen visual premium sesuai standar PRD:
  - Kartu header dengan badge kuota dan tombol "+ Tambah Cron Job".
  - Pencarian dan filter instan.
  - Tabel job interaktif dengan status pill (Idle, Berjalan, Berhasil, Gagal), badge durasi ms dan exit code, serta aksi Run Now, Riwayat & Log, Edit, dan Hapus.
  - Modal Create/Edit dengan tab preset (Setiap 5 Menit, Jam, 02:00, dll) dan pembuat cron 5-bagian kustom dengan live translation badge.
  - Modal Riwayat Eksekusi & Systemd Journal Viewer.

---

## Konsekuensi
- **Positif**:
  - Semua eksekusi scheduled job tenant terisolasi penuh di cgroup slice tenant (`panel-u<uid>.slice`).
  - Tidak ada lagi risiko tumpang tindih eksekusi berkat `Type=oneshot`.
  - Riwayat eksekusi dan log tersimpan dengan rapi dan dapat diaudit.
  - Pengguna mendapatkan pengalaman intuitif dalam membangun ekspresi cron tanpa harus menghafal sintaksis UNIX.
- **Batasan**:
  - Unit systemd memerlukan hak root agent untuk ditulis ke `/etc/systemd/system/` dan di-reload melalui `systemctl daemon-reload`.
  - Testing harus menggunakan mock system runner di unit test dan dijalankan di VM Linux Ubuntu 24.04 untuk verifikasi penuh.
