# ADR 0057: Site Backups & Snapshot Restore, Automated Schedules, Quota Accounting, and S3 Storage (P8-T6)

## Status
Accepted

## Konteks
Sesuai PRD Bagian 8 (Fase 8 P8-T6: Backup & snapshot restore) dan Bagian 5 (Site Management):
1. **Snapshot Cadangan Lokal Berbasis Kompresi Modern**:
   - Panel hosting membutuhkan fitur pencadangan snapshot situs mandiri yang menggabungkan berkas konten situs (`docroot`) dan dump database terkait (`mariadb-dump`, `mysqldump`, `pg_dump`).
   - Format kompresi default adalah Zstandard (`tar.zst`) untuk kecepatan kompresi superior dan rasio efisien, dengan fallback `tar.gz` (gzip) untuk kompatibilitas lingkungan konvensional.
2. **Pemulihan Cadangan yang Aman (In-Place & Subfolder Kustom)**:
   - Pemulihan cadangan harus mengembalikan situs yang identik ("restore backup mengembalikan situs yang identik").
   - Mendukung dua mode pemulihan:
     - In-place restore: Mengekstrak langsung ke folder docroot situs asli.
     - Custom target subfolder: Mengekstrak ke subfolder relatif (misalnya `staging`, `restore-test`) untuk pengujian atau inspeksi sebelum dipublikasikan.
   - Restorasi selektif: Pengguna dapat memilih untuk memulihkan berkas saja, basis data saja, atau keduanya.
3. **Pencadangan Terjadwal & Kebijakan Retensi Otomatis**:
   - Kemampuan menjadwalkan pencadangan berkala dengan frekuensi harian (`daily`), mingguan (`weekly`), atau bulanan (`monthly`).
   - Jam dan menit eksekusi yang dapat dikonfigurasi.
   - Batas retensi (`retention_count` 1-100, default 7) di mana arsip tertua akan dipruning secara otomatis ketika kuota jumlah cadangan tercapai.
   - Dukungan tujuan penyimpanan: Lokal host VM (`local`) dan remote S3-compatible cloud storage (`s3`) seperti AWS S3, Cloudflare R2, MinIO, atau Wasabi.
4. **Akuntansi Kuota Penyimpanan**:
   - Ukuran arsip cadangan fisik di host dilacak di basis data (`size_bytes`) dan dihitung ke dalam kuota disk tenant (`disk_limit_bytes`).
5. **Eksekusi Asinkron via Background JobRunner**:
   - Operasi pengompresan arsip (`backup.create`) dan pengekstrakkan/dump import (`backup.restore`) dapat memakan waktu lama, sehingga dijalankan secara asinkron di latar belakang menggunakan `JobRunner` dengan pelaporan progres real-time.
6. **Antarmuka Pengguna Komprehensif (Frontend UI)**:
   - Tab baru `'backups'` di `SiteDetailPage.tsx` (`SiteBackupsTab.tsx`).
   - Indikator penggunaan ruang cadangan, status jadwal otomatis, tabel arsip cadangan dengan badge konten & format, modal Buat Cadangan, modal Pemulihan, modal Jadwal & Retensi, dan dialog konfirmasi bahaya saat penghapusan.

---

## Keputusan Arsitektur

### 1. Model Domain & Validasi Keamanan (`crates/panel-core/src/backup.rs`)
- **Format Cadangan (`BackupFormat`)**:
  - `TarZst`: `tar.zst` (Zstandard kompresi tinggi).
  - `TarGz`: `tar.gz` (Gzip standar).
  - Parsing aman: `from_str_loose` memetakan string format dengan fallback aman `TarZst`.
- **Manifest Metadata Arsip (`BackupManifest`)**:
  - Berkas JSON `manifest.json` yang disematkan di dalam setiap arsip cadangan untuk melacak versi panel, ID situs, domain utama, timestamp pembuatan, format, dan daftar database yang disertakan.
- **Validasi Keamanan Ketat**:
  - `validate_backup_label(label)`: Memastikan panjang label tidak melebihi 64 karakter dan hanya menggunakan karakter alfanumerik, spasi, tanda hubung, atau garis bawah.
  - `validate_backup_filename(filename)`: Mencegah directory traversal (`..`, `/`, `\`) dan hanya mengizinkan karakter yang valid dengan ekstensi `.tar.zst` atau `.tar.gz`.
  - `validate_restore_destination(dest, tenant_home)`: Memastikan folder tujuan pemulihan berada di dalam direktori kurungan tenant (`tenant_home`), mencegah penulisan di luar batas isolasi tenant.
  - `validate_retention_count(count)`: Membatasi retensi antara 1 hingga 100 cadangan.
  - `validate_backup_schedule(frequency, hour, minute)`: Memvalidasi frekuensi (`daily`, `weekly`, `monthly`), jam (0-23), dan menit (0-59).
  - `validate_s3_config(...)`: Memvalidasi endpoint URL (HTTPS/HTTP), nama bucket (standar RFC S3), region, dan access key.

### 2. Skema Basis Data & Repositori (`crates/panel-db`)
- **Migrasi `20261003000011_backups.sql`**:
  - Tabel `site_backups`: Menyimpan data id, site_id, user_id, filename, filepath, size_bytes, label, includes_files, includes_databases, status (`pending`, `running`, `completed`, `failed`, `restoring`), error_message, storage_type (`local`, `s3`), s3_key, checksum_sha256, created_at, updated_at.
  - Tabel `site_backup_schedules`: Menyimpan konfigurasi jadwal per situs (site_id primary key, is_enabled, frequency, time_hour, time_minute, retention_count, include_files, include_databases, last_run_at, next_run_at, storage_destination, s3_endpoint, s3_bucket, s3_region, s3_access_key, s3_secret_key_enc, s3_prefix, updated_at).
- **Repositori `SqliteBackupRepository`**:
  - Pelacakan kuota: `get_total_backup_bytes_by_site(site_id)` dan `get_total_backup_bytes_by_user(user_id)`.
  - Pengelolaan lifecycle cadangan: `create_backup`, `update_backup_status`, `find_backup_by_id`, `list_backups_by_site`, `delete_backup`.
  - Otomatisasi retensi: `prune_old_backups(site_id, retention_count)` mengembalikan daftar arsip yang kedaluwarsa untuk dihapus fisiknya oleh agent.
  - Pengelolaan jadwal: `get_schedule` dan `upsert_schedule`.

### 3. IPC DTO & Eksekusi Agent Terisolasi (`panel-proto` & `panel-agent`)
- **Pesan IPC DTO (`panel-proto`)**:
  - `BackupCreateRequest` / `BackupCreateResponse`: Meminta agent membuat arsip staging, menjalankan dump database, mengompres berkas via `tar` + `zstd`/`gzip`, menghitung SHA-256 checksum, dan mengembalikan ukuran bita.
  - `BackupRestoreRequest` / `BackupRestoreResponse`: Meminta agent mengekstrak arsip cadangan ke folder target, mengatur kepemilikan UID/GID tenant, dan mengimpor dump SQL ke engine database.
  - `BackupDeleteRequest` / `BackupDeleteResponse`: Menghapus berkas arsip fisik dari host.
- **Whitelisting Biner pada Agent (`crates/panel-agent/src/system.rs`)**:
  - Menambahkan `tar`, `zstd`, `gzip` ke dalam whitelist biner resmi yang diizinkan untuk dieksekusi oleh agent.
- **Modul `crates/panel-agent/src/backups.rs`**:
  - Eksekusi aman tanpa shell piping string interpolation: Menggunakan argumen biner langsung melalui `SystemRunner`.
  - Staging terisolasi: Menggunakan direktori sementara tersembunyi `~/backups/.staging_<site_id>_<timestamp>` milik tenant.
  - Integritas data: Menghitung SHA-256 checksum dari arsip yang dihasilkan menggunakan library `sha2`.

### 4. REST API & Background Job Handlers (`panel-api`)
- **Job Registry (`crates/panel-api/src/jobs/registry.rs`)**:
  - `BackupCreateHandler` (`backup.create`): Menjalankan pembuatan arsip via agent, memperbarui database model dengan status `completed` beserta ukuran bita dan checksum SHA-256, serta memicu pembersihan retensi (`prune_old_backups`).
  - `BackupRestoreHandler` (`backup.restore`): Mengubah status cadangan menjadi `restoring`, mengekstrak berkas & data via agent, dan mengembalikan status `completed`.
- **REST Endpoints (`crates/panel-api/src/backups.rs`)**:
  - `GET /api/v1/sites/{id}/backups`: Mengambil daftar cadangan dan total penggunaan ruang.
  - `POST /api/v1/sites/{id}/backups`: Menjadwalkan job pembuatan cadangan baru via `JobRunner`.
  - `GET /api/v1/sites/{id}/backups/{backup_id}`: Mengambil rincian arsip cadangan.
  - `DELETE /api/v1/sites/{id}/backups/{backup_id}`: Menghapus arsip fisik di host dan rekaman database.
  - `POST /api/v1/sites/{id}/backups/{backup_id}/restore`: Menjadwalkan job pemulihan cadangan via `JobRunner`.
  - `GET /api/v1/sites/{id}/backups/{backup_id}/download`: Mengunduh berkas arsip biner langsung dengan header `attachment; filename=...`.
  - `GET /api/v1/sites/{id}/backups/schedule`: Mengambil konfigurasi jadwal pencadangan situs.
  - `PUT /api/v1/sites/{id}/backups/schedule`: Memperbarui konfigurasi jadwal pencadangan dan remote S3.

### 5. Frontend UI & Aksesibilitas (`web/src`)
- **Komponen `SiteBackupsTab.tsx`**:
  - Kartu Ringkasan: Penggunaan ruang cadangan, status jadwal otomatis, dan jenis penyimpanan (Lokal/S3).
  - Bilah Aksi & Pencarian: Filter nama/label berkas secara real-time.
  - Tabel Cadangan: Kolom nama berkas, label, tanggal pembuatan, ukuran bita terformat, badge komponen (Berkas & Basis Data), badge status, dan tombol aksi (Unduh, Pulihkan, Hapus).
  - Modal Interaktif:
    - Modal Buat Cadangan: Pilihan format (`tar.zst` vs `tar.gz`), label opsional, checkbox berkas dan database.
    - Modal Pemulihan: Peringatan risiko penimpaan data, pilihan target (In-place docroot vs subfolder kustom), dan seleksi komponen.
    - Modal Jadwal & Retensi: Toggle aktif, frekuensi, jam & menit, batas retensi, tujuan lokal/S3, serta input kredensial S3.
    - Dialog Konfirmasi Hapus: Konfirmasi bahaya sebelum menghapus berkas permanen.
- **Navigasi Tab di `SiteDetailPage.tsx`**:
  - Menambahkan tab `'backups'` dengan ikon `Archive` dan lokalisasi multi-bahasa (`id.json` dan `en.json`).

---

## Konsekuensi

### Positif
1. **Perlindungan Data Menyeluruh**: Situs statis, PHP, Node.js, Python, dan reverse proxy beserta basis data MariaDB/PostgreSQL dapat dicadangkan dan dipulihkan secara identik.
2. **Efisiensi Penyimpanan**: Format Zstandard memberikan rasio kompresi tinggi dan waktu pemrosesan sangat cepat, meminimalkan penggunaan disk VM.
3. **Akuntansi Kuota yang Adil**: Penggunaan ruang cadangan dilacak dan diperhitungkan terhadap kuota disk paket tenant.
4. **Keamanan Eksekusi**: Operasi pembuatan dan pemulihan berjalan di latar belakang tanpa string interpolation shell dan dengan pembersihan staging otomatis.

### Netral
1. Kredensial S3 disimpan di database; pada lingkungan produksi dengan secret management penuh, kunci rahasia dapat dienkripsi dengan symmetric master key.

---

## Pengujian & Verifikasi
- **Unit Tests Core Domain**: 8 unit test di `panel-core::backup::tests` memvalidasi format, manifest, dan validasi label, filename, restore destination, schedule, dan konfigurasi S3.
- **Repository Integration Tests**: `crates/panel-db/tests/backup_repository_test.rs` memvalidasi pembuatan entri cadangan, query total ukuran kuota, pembaruan status, pruning retensi otomatis, upsert schedule, dan penghapusan.
- **Agent Dispatch Mock Tests**: `panel_agent::tests::test_delete_backup_mock` memvalidasi penghapusan file arsip oleh agent.
- **REST API Integration Tests**: `crates/panel-api/tests/backup_api_test.rs` memvalidasi endpoint list, create job, download archive, delete, update schedule, dan restore job.
- **Frontend Component Tests**: `web/src/components/sites/__tests__/SiteBackupsTab.test.tsx` memvalidasi render kartu ringkasan, tombol aksi, formulir modal, dan status pemuatan.
- **Quality Gates**: Semua lulus `cargo fmt --check`, `cargo clippy -- -D warnings`, `npx tsc --noEmit`, `npm run lint`, dan `npm test -- --run`.
