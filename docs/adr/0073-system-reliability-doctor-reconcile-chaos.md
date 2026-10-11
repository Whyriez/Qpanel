# ADR 0073: Keandalan Sistem, Diagnostics Doctor, State Reconciler, Snapshot Backup, dan Chaos Fault Tolerance

## Status
Diterima

## Konteks
Nusa Panel berjalan pada server production VPS/dedicated Linux Ubuntu 24.04 LTS yang dapat mengalami anomali runtime di luar kendali panel, seperti:
1. Pemadaman listrik mendadak atau crash kernel yang menyebabkan job queue berstatus `running` tertinggal dalam kondisi macet (interrupted jobs).
2. Perubahan manual atau korupsi pada konfigurasi Nginx vhost (`/etc/nginx/sites-available` atau `/etc/panel/nginx`), kegagalan direktif Nginx syntax, atau service Nginx berhenti (`failed`/`inactive`).
3. Disk penuh (ENOSPC) saat penulisan konfigurasi atau pembuatan cadangan.
4. Drift antara *desired state* yang tercatat pada basis data SQLite panel dengan *actual state* pada disk host (berkas vhost hilang, permission salah, atau isi konfigurasi berbeda).
5. Kebutuhan migrasi atau pemulihan panel instan yang mencakup basis data SQLite (`panel.db`) dan konfigurasi global (`panel.toml`, berkas vhost, manifest manifest.json).

Diperlukan arsitektur keandalan terpadu yang mencakup:
- `panelctl doctor` (dan REST API `/api/v1/system/doctor`) untuk diagnosis kesehatan end-to-end dengan rekomendasi perbaikan dan kemampuan `--fix` otomatis.
- `panelctl reconcile` (dan REST API `/api/v1/system/reconcile`) untuk mendeteksi drift antara DB dan host, dengan mode `dry_run` dan eksekusi perbaikan deterministik.
- `panelctl backup` (dan REST API `/api/v1/system/backups`) untuk pembuatan, verifikasi checksum sha256, daftar snapshot, dan pemulihan aman dengan pra-pemulihan otomatis (*safety pre-restore snapshot*).
- Pemulihan job terinterupsi secara deterministik (`running` -> `failed` dengan pesan `"Interrupted by server restart"`).
- Suite pengujian *Chaos Testing* dengan 5 skenario fault injection.

## Keputusan Arsitektur

### 1. Diagnostic Health Engine (`panel-core::doctor`, `panel-agent`, `panel-cli`)
- Struktur diagnosis mengelompokkan pemeriksaan ke dalam kategori: `OsKernel`, `Prerequisites`, `Services`, `Socket`, `Database`, `Jobs`, `Filesystem`, dan `Firewall`.
- Setiap pemeriksaan menghasilkan status `Pass`, `Warn`, atau `Fail`, dengan pesan deskriptif dan petunjuk perbaikan (`remediation_hint`).
- Status keseluruhan (`overall_status`) diagregasikan: bila ada `Fail`, sistem berstatus `Critical`; bila ada `Warn`, sistem berstatus `Warning`; jika semua `Pass`, sistem `Healthy`.
- Mode perbaikan (`--fix` atau endpoint `/api/v1/system/doctor/fix`) mengeksekusi aksi terkurasi:
  - Pemulihan semua pekerjaan yang berstatus `running` ke `failed`.
  - Checkpoint SQLite WAL truncate (`PRAGMA wal_checkpoint(TRUNCATE)`).
  - Pembuatan struktur direktori kerja panel yang hilang (`/var/backups/panel`, direktori log/status).
  - Restart service Nginx jika tidak aktif.

### 2. State Reconciler (`panel-core::reconcile`, `ReliabilityManager::reconcile`)
- Melakukan verifikasi dua arah antara database SQLite dan sistem berkas host:
  - **Missing Files**: Vhost yang tercatat di database namun tidak ada di `/etc/panel/nginx/{domain}.conf` atau `{site_id}.conf`.
  - **Syntax / Content Drift**: Berkas ada namun sintaks Nginx rusak atau kontennya tidak sinkron dengan template konfigurasi di basis data.
  - **Service Status**: Kondisi systemd Nginx yang aktif vs down.
- Reconciler mendukung `dry_run: true` (hanya mendeteksi dan melaporkan daftar `DriftItem`) dan `dry_run: false` (menulis ulang konfigurasi yang hilang/rusak dan me-reload Nginx).

### 3. System Snapshot Backup & Restore (`panel-core::system_backup`, `panel-cli`, `panel-api`)
- Snapshot format standar `.tar.gz` yang disimpan di direktori terisolasi (`/var/backups/panel`).
- Format penamaan berkas divalidasi dengan regex ketat: `panel-backup-{timestamp}-{sha8}.tar.gz`.
- Isi arsip meliputi:
  - `panel.db`: Salinan basis data SQLite setelah WAL checkpoint truncate.
  - `panel.toml`: Berkas konfigurasi panel jika tersedia.
  - `nginx/`: Seluruh berkas konfigurasi situs dan vhost aktif.
  - `manifest.json`: Metadata snapshot berisi timestamp, versi panel, daftar berkas beserta ukuran dan checksum SHA-256 masing-masing.
- Saat pemulihan (`restore`), panel secara otomatis mengambil snapshot keselamatan (*safety pre-restore snapshot*) sebelum menimpa database, guna mencegah kehilangan data bila arsip pemulihan bermasalah.

### 4. Deterministic Job Interruption Recovery
- Pada saat startup API atau agent, query repository `recover_interrupted_jobs()` dijalankan:
  ```sql
  UPDATE jobs
  SET status = 'failed',
      error = 'Interrupted by server restart',
      completed_at = CURRENT_TIMESTAMP
  WHERE status = 'running';
  ```
- Ini memastikan tidak ada job zombie yang menggantung tanpa henti.

### 5. Chaos Testing Suite (`crates/panel-agent/tests/chaos_resilience_test.rs`)
Suite chaos fault injection memvalidasi 5 skenario ekstrem:
1. **Atomic Staging Abort**: Abort saat staging konfigurasi meninggalkan konfigurasi aktif Nginx utuh tanpa modifikasi parsial.
2. **Nginx Service Down Detection & Auto-Remediation**: `ReliabilityManager::check_doctor` mendeteksi Nginx down dan `fix_doctor` memulihkan status service.
3. **ENOSPC / Disk Full Error Handling**: Penulisan konfigurasi saat disk penuh menghasilkan `AgentError::Io` / error deterministik yang tertangkap bersih tanpa panic atau file rusak.
4. **Corrupted Nginx Config Reconcile**: Kerusakan konfigurasi di disk terdeteksi sebagai drift dan dipulihkan kembali sesuai basis data oleh `reconcile(dry_run = false)`.
5. **Forced Restart Interrupted Job Deterministic Recovery**: Membuktikan bahwa pekerjaan `running` yang terinterupsi server reboot dipulihkan menjadi `failed` dengan alasan error `"Interrupted by server restart"`.

### 6. User Interface (`SystemReliabilityCard.tsx`)
- Komponen web terintegrasi di halaman Pengaturan (`SettingsPage.tsx`) dengan 3 tab:
  - **Dokter Sistem**: Visualisasi status kesehatan, ringkasan kartu status, dan tombol "Perbaiki Otomatis".
  - **Penyelarasan Host (Reconcile)**: Daftar drift sumber daya, status drift, dan tombol tindakan rekonsiliasi.
  - **Cadangan Sistem**: Pembuatan cadangan cepat, verifikasi SHA-256, riwayat berkas cadangan, dan tombol pemulihan.
- Aksesibilitas WCAG 2.2 AA (kontras warna jelas, aria attributes, keyboard navigable) dan copy Bahasa Indonesia ramah pengguna.

## Konsekuensi
- **Positif**:
  - Administrator panel memiliki visibilitas lengkap atas integritas sistem melalui UI dan CLI (`panelctl doctor`, `reconcile`, `backup`).
  - Sistem memiliki toleransi kesalahan tinggi terhadap crash, restart mendadak, dan disk error.
  - Rollback dan pemulihan snapshot terjamin aman dengan audit checksum sha256 dan safety snapshot.
- **Batasan**:
  - Pada sistem host non-Linux (pengembangan), agent berjalan dengan simulasi ramah testing; eksekusi sebenarnya memerlukan VM Linux Ubuntu 24.04 LTS dengan `systemd` dan `cgroup v2`.
