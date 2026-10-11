# ADR 0068: Siklus Hidup Data dan Manajemen Retensi Analitik

## Konteks
Panel hosting mengumpulkan volume data metrik lalu lintas web, metrik sistem host, jejak slowlog aplikasi, dan event keamanan secara terus menerus. Tanpa mekanisme manajemen siklus hidup dan kebijakan retensi data yang tegas, basis data SQLite analitik (`analytics.db`) akan membengkak tak terkendali, menghabiskan kuota ruang disk server, meningkatkan overhead jurnal WAL, serta memperlambat waktu respons query analitik.

PRD Bab 4.6, 5.6, 6, 8.4–8.5, dan Task P9-T9 mensyaratkan:
1. Multi-resolution retention hierarchy:
   - Data mentah (resolusi 10 detik): retensi bawaan 2 jam.
   - Rollup 1 menit: retensi bawaan 14 hari.
   - Rollup 1 jam: retensi bawaan 400 hari (~13 bulan).
   - Rollup 1 hari: retensi permanen / opsional 3 tahun.
   - Event keamanan & probing: retensi bawaan 7 hari.
   - Log tailer checkpoints: pembersihan checkpoint usang > 30 hari.
2. Background Retention Worker terjadwal:
   - Berjalan berkala di latar belakang (setiap 30 menit) dengan graceful shutdown channel.
   - Menjalankan `PRAGMA wal_checkpoint(TRUNCATE)` untuk mencegah pembengkakan berkas jurnal WAL.
   - Menjalankan `VACUUM` otomatis saat rasio fragmentasi halaman freelist SQLite melebihi ambang batas 20%.
   - Database size guardrail: Deteksi ukuran berkas (`main_db_size_bytes` + `wal_size_bytes`) melebihi `max_db_size_bytes`, dengan pemangkasan darurat (*emergency pruning*) bertingkat jika disk terancam penuh.
3. Deteksi korupsi basis data & pemulihan otomatis:
   - Pemeriksaan integritas struktur SQLite melalui `PRAGMA quick_check`.
   - Pemulihan otomatis basis data korup (`recover_corrupted_database`): isolasi/rename berkas korup ke berkas backup bertimestamp dan inisialisasi ulang skema analitik segar.
4. REST API Endpoints khusus admin:
   - `GET /api/v1/system/analytics/retention`: status konfigurasi retensi, statistik penyimpanan database (`DatabaseStorageStatsDto`), rincian 23 tabel, dan laporan pemeliharaan terakhir.
   - `PUT /api/v1/system/analytics/retention`: pembaharuan konfigurasi retensi.
   - `POST /api/v1/system/analytics/maintenance`: pemicu pemeliharaan & pemangkasan manual segera (dengan opsi `force_vacuum` dan `force_wal_checkpoint`).
   - `POST /api/v1/system/analytics/integrity-check`: uji integritas basis data SQLite.
5. Antarmuka Web UI yang ramah pengguna, informatif, dan aksesibel (WCAG 2.2 AA).

## Keputusan Arsitektur

### 1. Ekstensi Model dan Mesin Pemeliharaan di `panel-analytics`
- Model `RetentionConfig` diperluas dengan bidang:
  - `security_events_retention_secs` (bawaan 7 hari),
  - `max_db_size_bytes` (opsional batas ukuran basis data),
  - `auto_vacuum` (boolean toggle untuk auto-vacuum saat fragmentasi > 20%).
- Didefinisikan struktur metrik penyimpanan `DatabaseStorageStats`:
  - `main_db_size_bytes`, `wal_size_bytes`, `total_size_bytes`,
  - `page_size`, `page_count`, `freelist_count`, `free_bytes`, `fragmentation_ratio`,
  - Rincian jumlah baris untuk seluruh 23 tabel SQLite (`table_stats`).
- `AnalyticsStorage` dilengkapi metode pemeliharaan dan integritas:
  - `wal_checkpoint_truncate()`: mengeksekusi `PRAGMA wal_checkpoint(TRUNCATE)`.
  - `vacuum()`: mengeksekusi perintah SQL `VACUUM`.
  - `check_integrity()`: mengeksekusi `PRAGMA quick_check` dan mengembalikan daftar galat jika ada.
  - `run_maintenance(now, config, force_vacuum)`: memangkas metrik mentah, rollup 1m/1h/1d, event keamanan, dan checkpoint usang; memeriksa batas `max_db_size_bytes`; memicu checkpoint WAL; dan menjalankan VACUUM bila fragmentasi > 20% atau `force_vacuum` bernilai true.
  - `recover_corrupted_database(db_path)`: memindahkan berkas korup ke `.corrupt.<timestamp>` dan menginisialisasi skema baru yang bersih.

### 2. REST API & Background Worker di `panel-api`
- Dibuat modul `crates/panel-api/src/retention.rs` dengan DTOs OpenAPI (`ToSchema`, `Serialize`, `Deserialize`):
  - `AnalyticsRetentionResponse`, `RetentionConfigDto`, `DatabaseStorageStatsDto`, `RetentionMaintenanceReportDto`, `IntegrityCheckReportDto`, `UpdateRetentionRequest`, `TriggerMaintenanceRequest`.
- Background worker `run_retention_worker` dijalankan saat server startup dan memicu pemeliharaan setiap 30 menit atau saat menerima sinyal shutdown.
- Logging audit otomatis: Setiap perubahan konfigurasi retensi dan pemeliharaan manual dicatat ke audit log sistem secara asinkron melalui `AuditWriter::log_async`.

### 3. Antarmuka Pengguna Frontend
- Types di `web/src/types/api.ts` dan fungsi HTTP di `web/src/api/client.ts`.
- Komponen `AnalyticsRetentionCard.tsx`:
  - 4 Kartu KPI: Ukuran Berkas DB & WAL, Ruang Bebas & Halaman SQLite, Rasio Fragmentasi (dengan rekomendasi VACUUM bila > 20%), Status Integritas SQLite (badge Sehat / Korup).
  - Banner Ringkasan Pembersihan Terakhir: Bytes yang dibebaskan, jumlah baris data mentah, rollup, dan keamanan yang dipangkas, serta penanda status pemangkasan darurat.
  - Form Konfigurasi Retensi interaktif untuk admin dengan input data mentah (jam), rollup 1m (hari), rollup 1h (hari), rollup 1d (hari / permanen), event keamanan (hari), batas ukuran DB (MB), dan toggle auto-vacuum.
  - Aksi Operasional: Tombol "Jalankan Pembersihan & Optimasi" (dengan opsi paksa VACUUM dan truncate WAL) serta tombol "Uji Integritas Basis Data".
  - Section Collapsible Rincian 23 Tabel Basis Data lengkap dengan kategorisasi resolusi tabel.
- Diintegrasikan ke tab Server Host pada `web/src/pages/AnalyticsPage.tsx`.

## Konsekuensi
- **Positif**:
  - Ukuran basis data SQLite terkontrol stabil dengan pemangkasan multi-resolusi otomatis.
  - Berkas jurnal WAL tidak membengkak tanpa batas berkat checkpoint berkala TRUNCATE.
  - Integritas data terpantau terus-menerus dan terhindar dari downtime server akibat disk kepenuhan.
  - Admin memiliki kendali penuh melalui dashboard visual dan REST API.
- **Batasan**:
  - Operasi `VACUUM` memerlukan ruang disk sementara setara dengan ukuran berkas basis data utama saat proses pemadatan berlangsung. Pemicuan dibatasi hanya saat fragmentasi > 20% atau instruksi manual admin.
