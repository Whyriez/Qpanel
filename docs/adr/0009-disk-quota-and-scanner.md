# ADR-0009: Manajemen Kuota Disk, Deteksi Filesystem, dan Scanner Direktori Fallback

- **Status**: Diterima (Accepted)
- **Tanggal**: 2026-10-03
- **Konteks**: PRD Bagian 4.4, 5.2, Task P2-T3

## Konteks dan Masalah

Setiap tenant pada panel hosting memiliki kuota penyimpanan (disk byte quota) dan kuota jumlah berkas (inode quota) yang ditentukan oleh hosting plan mereka. Kegagalan membatasi kapasitas penyimpanan dapat menyebabkan kehabisan disk server atau serangan denial-of-service melalui kehabisan inode.

Kebutuhan arsitektur:
1. **Deteksi Dukungan Filesystem**:
   - Filesystem modern Linux pada server hosting umumnya menggunakan **ext4** (dengan kuota berbasis kernel `usrquota`) atau **XFS** (dengan kuota bawaan proyek/user).
   - Namun, pada lingkungan container, VM tanpa kuota kernel aktif, atau filesystem tanpa opsi mount quota, sistem harus memiliki mekanisme fallback yang andal agar pemantauan kapasitas tetap berjalan tanpa error fatal.
2. **Operasi Set dan Get Quota Bertipe**:
   - `panel-api` tidak memiliki hak akses root, sehingga pengaturan kuota kernel (`setquota` / `xfs_quota`) didelegasikan ke `panel-agent` melalui Unix Domain Socket dengan payload bertipe (`QuotaSetRequest`, `QuotaGetRequest`, `QuotaScanRequest`).
3. **Pekerjaan Latar Belakang (Background Jobs)**:
   - Pelaporan pemakaian kuota berkala (`quota.report`) untuk memperbarui status pemakaian di database.
   - Pemindaian direktori terjadwal (`quota.scan`) sebagai fallback saat kuota kernel tidak tersedia atau untuk rekonsiliasi berkala.
4. **Keamanan & Ketahanan (Safety & Resilience)**:
   - Dilarang keras melakukan traversal symlink saat memindai direktori (mencegah symlink escape ke `/` atau direktori tenant lain).
   - Validasi ketat terhadap path root tenant dan penolakan traversal `..`.
   - Menggunakan allowlisted binaries (`setquota`, `repquota`, `xfs_quota`, `findmnt`) tanpa manipulasi string shell sembarangan.

## Keputusan Arsitektur

### 1. Deteksi Backend Filesystem (`FsQuotaBackend`)
Modul `crates/panel-agent/src/quota.rs` menyediakan enum `FsQuotaBackend`:
- `Ext4`: Menggunakan `setquota -u <username>` dan `repquota -u`.
- `Xfs`: Menggunakan `xfs_quota -x -c "limit bsoft=... bhard=..."`.
- `FallbackScan`: Menghitung penggunaan byte dan inode secara rekursif langsung dari sistem berkas home tenant.

Metode deteksi `detect_backend` memeriksa utilitas biner dan mount options (melalui `findmnt` atau simulasi runner).

### 2. Protokol Bertipe (`panel-proto`)
Payload RPC baru ditambahkan ke `panel-proto`:
- `RequestPayload::QuotaSet(QuotaSetRequest)` -> `ResponsePayload::QuotaSet(QuotaSetResponse)`
- `RequestPayload::QuotaGet(QuotaGetRequest)` -> `ResponsePayload::QuotaGet(QuotaGetResponse)`
- `RequestPayload::QuotaScan(QuotaScanRequest)` -> `ResponsePayload::QuotaScan(QuotaScanResponse)`

Struktur `QuotaUsage` mencakup:
- `used_bytes`, `byte_soft_limit`, `byte_hard_limit`
- `used_inodes`, `inode_soft_limit`, `inode_hard_limit`
- `backend`: Backend yang digunakan untuk kalkulasi.

### 3. Scanner Direktori Fallback Tanpa Traversal Symlink
Fungsi `scan_directory` pada `panel-agent` melakukan DFS menggunakan antrian:
- `fs::symlink_metadata` digunakan untuk membaca ukuran berkas dan jenis entri tanpa mengikuti target symlink (`file_type.is_symlink()`).
- Hanya direktori riil (`file_type.is_dir()`) yang dimasukkan ke antrian telusur anak.
- Menghitung satu inode untuk setiap direktori riil dan setiap entri berkas/symlink di dalamnya.

### 4. Handler Job Pemantauan di `panel-api`
Pada `crates/panel-api/src/jobs/registry.rs`:
- `QuotaReportHandler` (`quota.report`): Mengambil status kuota tenant dari agent dan mencatat metrik ke log audit / DB.
- `QuotaScanHandler` (`quota.scan`): Menjalankan pemindaian fallback direktori melalui agent RPC dan menyimpan hasil rekonsiliasi.

### 5. Client Integration dan IPC Test
- `AgentClient` diperkaya dengan metode `set_quota`, `get_quota`, `scan_quota`, dan helper `plan_to_quota_set_request`.
- Pengujian integration test `tests/quota_ipc_test.rs` memverifikasi alur penuh: deteksi backend, set quota, get quota, fallback scanner, validasi keamanan path traversal (`../etc`), dan eksekusi background job worker.

## Konsekuensi

- Sistem kuota siap berjalan di lingkungan produksi (ext4/xfs) maupun lingkungan testing/container (fallback scan).
- Tidak ada privilege leak ke `panel-api`.
- Pemindaian rekursif aman dari loop symlink atau escape traversal.
