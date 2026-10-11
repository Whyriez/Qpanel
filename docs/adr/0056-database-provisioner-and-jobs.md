# ADR 0056: Database Provisioner (MariaDB + PostgreSQL), Multi-Tenant Quotas, Remote Access, and Async Job Dumps (P8-T5)

## Status
Accepted

## Konteks
Sesuai PRD Bagian 8 (Fase 8 P8-T5: Database provisioner) dan Bagian 5.5 (Database Management):
1. **Dukungan Multi-Engine On-Demand (MariaDB & PostgreSQL)**:
   - Panel hosting membutuhkan penyediaan basis data relasional fleksibel dengan dua engine utama: MariaDB (port default 3306) dan PostgreSQL (port default 5432).
   - Server engine tidak harus selalu aktif atau terpasang secara bawaan pada VM berukuran kecil; admin sistem harus dapat melakukan instalasi on-demand (`database.server.install`) serta kontrol lifecycle service (`start`, `stop`, `restart`).
2. **Isolasi Multi-Tenant & Penamaan Prefixed Ketat**:
   - Mencegah tabrakan nama skema dan user antar tenant dengan memberlakukan prefix wajib berbasis identitas tenant (`p_<username>_` atau `<username>_`).
   - Setiap database tenant memiliki user khusus yang hanya memiliki hak akses (GRANT) terbatas pada database miliknya sendiri tanpa hak administratif server (`SUPERUSER`, `ALL PRIVILEGES` global).
3. **Penegakan Batasan Kuota Paket (Plan Limits)**:
   - Jumlah database yang dapat dibuat oleh tenant dibatasi secara ketat oleh `plan.max_databases`.
   - Percobaan pembuatan database melampaui batas kuota harus ditolak pada lapisan API dengan kode error HTTP 403 Forbidden / ValidationError sebelum delegasi ke sistem agent.
4. **Rotasi Kredensial Aman**:
   - Penyediaan fitur rotasi kata sandi instan dengan opsi generator sandi acak kuat (18 karakter alfanumerik + simbol aman) atau sandi masukan tervalidasi yang memenuhi kriteria kompleksitas.
5. **Kebijakan Remote Access Default-Deny**:
   - Standar keamanan ketat: Akses jarak jauh dimatikan secara default, membatasi bind dan grant host hanya ke `localhost` / `127.0.0.1`.
   - Tenant dapat mengaktifkan remote access dengan menentukan daftar IP/CIDR spesifik atau wildcard (`%` untuk MySQL/MariaDB) secara selektif.
6. **Ekspor & Impor Dump Berbasis Asinkron (Background Jobs)**:
   - Ekspor dan impor file dump (`.sql`, `.sql.gz`, `.dump`) dapat memakan waktu lama pada basis data besar, sehingga tidak boleh memblokir thread HTTP API request/response.
   - Menggunakan `JobRunner` (`database.export` dan `database.import`) dengan penelusuran status job dan pembatasan direktori dump path di bawah root direktori tenant yang terisolasi.
7. **Antarmuka Pengguna Responsif & Aksesibel**:
   - Integrasi tab `database` di `SiteDetailPage` (menampilkan kuota, daftar database, modal pembuatan, rotasi sandi, toggle remote access, modal ekspor, dan impor).
   - Integrasi tab `databases` di `RuntimePage` (manajemen engine server untuk administrator sistem).

---

## Keputusan Arsitektur

### 1. Model Domain & Validasi Keamanan (`crates/panel-core/src/database.rs`)
- **Engine Enum**:
  - `DatabaseEngine`: `MariaDb` (MariaDB / MySQL kompatibel) dan `Postgres` (PostgreSQL).
- **Validasi Penamaan & Enklave Tenant**:
  - `validate_database_name(name, tenant_prefix)`: Memverifikasi nama skema tidak kosong, maksimal 64 karakter, karakter alfanumerik dan underscore, serta wajib diawali oleh prefix tenant yang sah (`p_<username>_` atau `<username>_`).
  - `validate_database_username(user, tenant_prefix)`: Memverifikasi username database tidak melebihi panjang maksimum biner (16-32 karakter) dan diawali oleh prefix tenant.
  - `validate_database_password(password)`: Memastikan panjang minimal 12 karakter dan memiliki variasi karakter aman tanpa karakter kontrol shell.
  - `generate_strong_database_password()`: Menghasilkan 18 karakter acak berkekuatan tinggi menggunakan `rand`.
- **Validasi Remote Access & File Paths**:
  - `validate_allowed_hosts(hosts)`: Memeriksa token host (IP valid, format CIDR, atau wildcard `%` untuk MariaDB) serta menolak karakter injeksi SQL/shell.
  - `validate_dump_filepath(path, tenant_home)`: Memastikan jalur berkas ekspor/impor berada di dalam batas kurungan direktori tenant (`tenant_home`), mencegah directory traversal (`..`).

### 2. Skema Basis Data & Repositori (`crates/panel-db`)
- **Migrasi `20261003000010_database_provisioner.sql`**:
  - Tabel `database_servers`: Menyimpan status engine (`mariadb`, `postgres`), port, versi terdeteksi, status running, dan timestamp instalasi.
  - Tabel `databases`: Menyimpan relasi multi-tenant database (id, user_id, site_id nullable, engine, name, username, allowed_hosts, is_remote_enabled, size_bytes, created_at, updated_at).
- **Repositori `SqliteDatabaseRepository`**:
  - `count_by_user(user_id)`: Menghitung jumlah database aktif untuk validasi kuota `max_databases`.
  - Operasi CRUD atomik untuk registrasi database, rotasi kredensial, pembaruan remote hosts, pembaruan estimasi ukuran berkas, dan status server engine.

### 3. Whitelist Biner & Eksekusi Agent Terisolasi (`panel-proto` & `panel-agent`)
- **Whitelisting Biner pada Agent (`crates/panel-agent/src/system.rs`)**:
  - Menambahkan biner CLI resmi ke whitelist: `mariadb`, `mysql`, `mysqldump`, `psql`, `pg_dump`, `mariadb-admin`, `mysqladmin`, `createdb`, `dropdb`, `createuser`, `dropuser`, `apt-get`.
- **Pesan IPC DTO (`panel-proto`)**:
  - `DatabaseServerStatusRequest` / `Response`: Deteksi keberadaan biner dan status systemd service engine.
  - `DatabaseServerInstallRequest` / `Response`: Instalasi otomatis paket MariaDB / PostgreSQL via apt-get.
  - `DatabaseServerActionRequest` / `Response`: Perintah `start`, `stop`, `restart` service database via systemd.
  - `DatabaseCreateRequest` / `Response`: Eksekusi pembuatan basis data, pembuatan user, dan penetapan izin (GRANT).
  - `DatabaseDeleteRequest` / `Response`: Penghapusan basis data dan pencabutan user secara bersih.
  - `DatabasePasswordRotateRequest` / `Response`: Penggantian kata sandi user database.
  - `DatabaseRemoteAccessUpdateRequest` / `Response`: Pembaruan daftar host remote yang diizinkan pada hak akses user.
  - `DatabaseExportRequest` / `Response`: Ekspor dump terkompresi menggunakan `mysqldump` / `pg_dump`.
  - `DatabaseImportRequest` / `Response`: Impor dump ke database target secara streaming aman tanpa shell piping.
  - `DatabaseSizeCheckRequest` / `Response`: Pengecekan ukuran disk database langsung dari engine metadata.
- **Eksekusi Aman Anti-Injeksi (`crates/panel-agent/src/databases.rs`)**:
  - Tidak ada piping shell mentah (`|`) atau string interpolation di shell.
  - Pemanggilan biner langsung dengan parameter terisolasi. Impor file dijalankan via streaming langsung berkas ke stdin biner client database.

### 4. API Endpoints & Background Jobs (`crates/panel-api`)
- **Background Jobs Terdaftar (`crates/panel-api/src/jobs/registry.rs`)**:
  - `database.server.install`: Job instalasi engine database oleh admin.
  - `database.export`: Job asinkron ekspor dump database ke path terkurung.
  - `database.import`: Job asinkron pemulihan/impor file dump ke database.
- **REST Endpoints**:
  - `GET /api/v1/databases/servers`: Mendapatkan daftar server database dan statusnya.
  - `POST /api/v1/databases/servers/{engine}/install`: Memicu instalasi on-demand engine database (Admin only).
  - `POST /api/v1/databases/servers/{engine}/action`: Mengendalikan service database (Admin only).
  - `GET /api/v1/databases`: Mendapatkan daftar database milik tenant atau seluruh database (Admin).
  - `POST /api/v1/databases`: Membuat database baru dengan pengecekan kuota `plan.max_databases` dan enforcement prefix.
  - `GET /api/v1/databases/{id}`: Detail informasi konfigurasi database.
  - `DELETE /api/v1/databases/{id}`: Menghapus database dan user terkait.
  - `POST /api/v1/databases/{id}/rotate-password`: Memperbarui kata sandi user database.
  - `PUT /api/v1/databases/{id}/remote-access`: Mengatur izin akses remote (IP / CIDR / `%`).
  - `POST /api/v1/databases/{id}/export`: Memulai background job ekspor dump database.
  - `POST /api/v1/databases/{id}/import`: Memulai background job impor dump database.

### 5. Antarmuka Pengguna Web (`web/`)
- **Tipe & Client API (`web/src/types/api.ts` & `web/src/api/client.ts`)**:
  - Tipe DTO lengkap untuk `DatabaseServerInfo`, `DatabaseItem`, `DatabaseCreateRequest`, dll.
  - 11 fungsi client API untuk operasi manajemen server dan tenant database.
- **Komponen UI**:
  - `SiteDatabaseTab.tsx`: Tab database pada detail situs dengan kartu kuota paket, tabel database, modal pembuatan dengan generator sandi acak, modal rotasi sandi, modal konfigurasi remote access, modal ekspor, dan modal impor dump.
  - `DatabaseServersTab.tsx`: Tab manajemen server pada halaman runtime dengan kartu MariaDB dan PostgreSQL, badge status instalasi dan running, tombol kontrol service, dan aksi instalasi on-demand.
- **Dukungan Lokalisasi**:
  - Kamus bahasa Indonesia (`id.json`) dan bahasa Inggris (`en.json`) mencakup semua label, status, tooltip, dan pesan kesalahan terkait database.

---

## Konsekuensi
- **Positif**:
  - Dukungan multi-engine (MariaDB + PostgreSQL) terisolasi penuh dengan penamaan skema yang aman antar tenant.
  - Pengurangan beban resource pada server kecil karena engine database dapat diinstal secara on-demand.
  - Ekspor dan impor file berukuran besar tidak lagi memblokir API HTTP berkat integrasi background `JobRunner`.
  - Keamanan default-deny remote access menjaga server dari ancaman brute force database port dari internet publik.
- **Trade-off**:
  - Ekspor/impor database ukuran gigabyte membutuhkan ruang disk sementara yang cukup pada direktori tenant.
  - Penegakan prefix tenant (`p_<username>_`) membatasi kustomisasi nama database bebas, namun esensial untuk isolasi multi-tenant yang tangguh.
