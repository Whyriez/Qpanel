# ADR 0055: Git Deploy, Atomic Symlink Releases, and Webhook Automation (P8-T4)

## Status
Accepted

## Konteks
Sesuai PRD Bagian 8 (Fase 8 P8-T4: Git Deploy & Webhook) dan Bagian 5 (Manajemen Web & Routing):
1. **Otomatisasi Deployment Berbasis Git**:
   - Pengembang membutuhkan mekanisme otomatis untuk men-deploy kode sumber dari remote repository (GitHub, GitLab, Bitbucket, self-hosted Git) langsung ke server tanpa memerlukan transfer FTP/SFTP manual.
   - Diperlukan dukungan isolasi keypair SSH (Ed25519) per-situs agar private key situs tidak dapat diakses situs lain maupun dieksfiltrasi.
2. **Zero-Downtime Atomic Symlink Swap**:
   - Pergantian versi rilis tidak boleh menimbulkan downtime atau file locking.
   - Pola struktur rilis terisolasi (`releases/<release_id>`) dengan swap atomik symlink (`current_tmp` -> `current`) menjadi standar industri untuk memastikan rollback instan dan retensi rilis terawat.
3. **Isolasi Eksekusi & Hook Pasca-Deploy**:
   - Perintah build (`build_cmd`) dan post-deploy hook (`post_deploy_cmd`) wajib dieksekusi di bawah identitas Linux tenant (`p_<username>`) dalam slice cgroup tenant terkait, bukan sebagai root atau user sistem panel.
4. **Otomatisasi Webhook Aman (GitHub & GitLab)**:
   - Endpoint webhook publik `/api/v1/webhooks/deploy/{site_id}` menerima notifikasi commit push dari penyedia Git.
   - Wajib diverifikasi secara kriptografis menggunakan HMAC-SHA256 (`X-Hub-Signature-256`) untuk GitHub dan secret token (`X-Gitlab-Token`) untuk GitLab dengan perbandingan constant-time guna mencegah timing attacks.
5. **Visibilitas Riwayat & Rollback 1-Klik**:
   - Setiap rilis dicatat lengkap dengan hash commit, author, commit message, waktu eksekusi, status, dan output log rilis.
   - Pengguna dapat melakukan rollback seketika ke versi rilis sukses sebelumnya jika terjadi kegagalan pada versi terbaru.

---

## Keputusan Arsitektur

### 1. Model Domain & Validasi Kriptografi (`crates/panel-core/src/git_deploy.rs`)
- **Validasi URL & Parameter Git**:
  - `validate_git_repo_url`: Memvalidasi URL SSH (`git@github.com:...`) dan HTTPS (`https://...`). Mencegah flag injection (awalan `-`) dan karakter berbahaya.
  - `validate_git_branch`: Memvalidasi nama branch Git sesuai spesifikasi Git ref format (bebas `..`, spasi, `~`, `^`, `:`, `?`, `*`, `[`).
  - `validate_docroot_subpath`: Memvalidasi relative subpath direktori publik, mencegah path traversal (`..` atau awalan `/`).
- **Verifikasi Webhook Kriptografis Zero-Dependency**:
  - Implementasi HMAC-SHA256 berbasis `hmac` dan `sha2` crate.
  - Perbandingan signature menggunakan `subtle::ConstantTimeEq` untuk mengeliminasi kerentanan side-channel timing attack.
  - Parser webhook modular: `parse_github_webhook` (memverifikasi signature hex sha256 dan mengekstrak branch ref, commit id, author, message) dan `parse_gitlab_webhook` (memverifikasi secret token dan mengekstrak commit metadata).

### 2. Skema Basis Data & Repositori (`crates/panel-db`)
- **Migrasi `20261003000009_git_deploy.sql`**:
  - Tabel `site_git_configs`: Menyimpan URL repo, branch aktif, deploy key status, docroot subpath, build command, post-deploy command, auto-deploy webhook toggle, webhook secret, dan retention count (default 5).
  - Tabel `site_deployments`: Mencatat riwayat deployment (release ID `YYYYMMDDHHMMSS-<nanos>`, trigger type `manual`, `webhook_github`, `webhook_gitlab`, `rollback`, commit hash, commit author, commit message, status `pending`, `running`, `success`, `failed`, `cancelled`, `rollback`, log ringkas, durasi ms, deployer username).
  - Tabel `deploy_markers`: Mencatat marker timestamp deployment untuk korelasi analitik performa dan error pada Fase 9.
- **Repositori `SqliteGitDeployRepository`**:
  - Operasi CRUD atomik untuk konfigurasi, deployment lifecycle (create, update status, list history by site, prune old records), dan analytics markers.

### 3. Protokol IPC & Eksekusi Agent Terisolasi (`panel-proto` & `panel-agent`)
- **Pesan IPC DTO**:
  - `SiteDeployKeyGenerateRequest` / `Response`: Menghasilkan pasangan kunci SSH Ed25519 (`ssh-keygen -t ed25519 -N "" -f ...`) di direktori `~/.ssh/id_deploy_<site_id>` milik tenant dengan mode file `0600`.
  - `SiteGitDeployRequest` / `Response`: Menjalankan git shallow clone (`--depth 1 --branch <branch>`), ekstraksi commit log, build commands di bawah tenant cgroup, update symlink atomik, dan pruning rilis lama.
  - `SiteGitRollbackRequest` / `Response`: Mengalihkan symlink `current` kembali ke rilis sebelumnya secara instan.
  - `SiteGitReleasesListRequest` / `Response`: Menginspeksi daftar folder rilis yang tersedia di sistem berkas host.
- **Symlink Operasional pada `SystemRunner`**:
  - Menambahkan abstraksi `create_symlink` dan `read_link` pada trait `SystemRunner` (diimplementasikan oleh `RealSystemRunner` di Linux dan didukung oleh `MockSystemRunner` untuk pengujian unit terisolasi).
  - Swap atomik diimplementasikan dengan membuat symlink sementara `current_tmp` lalu memanggil `std::fs::rename(current_tmp, current)` yang merupakan operasi atomik pada sistem berkas POSIX.
- **Whitelisting Biner Agent**:
  - Menambahkan biner `git`, `ssh-keygen`, `ssh`, `sh`, dan `bash` ke daftar whitelist biner yang diizinkan untuk dieksekusi oleh agent.

### 4. API Endpoints & Otorisasi RBAC (`crates/panel-api`)
- **REST Endpoints**:
  - `GET /api/v1/sites/{id}/git`: Mengambil konfigurasi Git situs dan status public key (tenant terisolasi atau admin).
  - `PUT /api/v1/sites/{id}/git`: Memperbarui konfigurasi Git, webhook secret, command build, dan retention count.
  - `POST /api/v1/sites/{id}/git/key/generate`: Menghasilkan pasangan kunci SSH Ed25519 baru dan mengembalikan public key untuk didaftarkan pada GitHub/GitLab Deploy Keys.
  - `POST /api/v1/sites/{id}/git/deploy`: Men-trigger deployment manual segera.
  - `POST /api/v1/sites/{id}/git/rollback`: Melakukan rollback ke release ID tertentu.
  - `GET /api/v1/sites/{id}/git/deployments`: Mendapatkan daftar riwayat deployment dengan pagination/limit.
  - `GET /api/v1/sites/{id}/git/deployments/{deploy_id}`: Mengambil detail rilis beserta full log stdout/stderr.
- **Webhook Receiver Endpoint Publik**:
  - `POST /api/v1/webhooks/deploy/{site_id}`:
    - Tidak memerlukan Bearer token autentikasi pengguna panel (publik bagi GitHub/GitLab).
    - Memverifikasi signature payload atau token rahasia secara constant-time terhadap `webhook_secret` situs.
    - Memastikan auto-deploy aktif dan branch payload cocok dengan branch konfigurasi.
    - Mencatat audit trail dan men-trigger background deployment secara asinkron dengan respons HTTP 202 Accepted.

### 5. Antarmuka Pengguna Frontend Modern (`web/src/components/sites/SiteDeployTab.tsx`)
- Tab `Deploy` terintegrasi langsung di `SiteDetailPage.tsx`.
- Komponen terstruktur:
  - **Deploy Key Card**: Visualisasi Ed25519 public key, instruksi pendaftaran ke GitHub/GitLab Deploy Keys, tombol salin instan, dan aksi regenerate key dengan konfirmasi aman.
  - **Repository & Build Configuration Form**: Input URL repositori, branch, docroot subpath, build command, post-deploy command, dan retention release count.
  - **Webhook Automation Card**: Webhook payload URL, generator random webhook secret, dan panduan konfigurasi event push GitHub/GitLab.
  - **Deployment Actions & Status**: Tombol "Deploy Sekarang" dengan status progress real-time.
  - **Riwayat Deployment Table**: Menampilkan waktu, ID rilis, trigger badge (`Manual`, `GitHub Webhook`, `GitLab Webhook`, `Rollback`), hash commit, author, commit message, durasi eksekusi, tombol Rollback 1-klik, dan tombol "Lihat Log".
  - **Interactive Log Modal**: Viewer log berbasis font monospace dengan warna terminal dan tombol salin log.
- Memenuhi standar aksesibilitas WCAG 2.2 AA dan lokalisasi penuh dwibahasa (ID/EN).

---

## Konsekuensi

### Positif
- **Zero-Downtime**: Pergantian versi aplikasi berlangsung instan dan atomik tanpa jeda layanan HTTP.
- **Keamanan Berlapis**: SSH deploy key diisolasi per-situs, perintah build/hook dibatasi di slice cgroup tenant, dan webhook terlindungi dari spoofing via HMAC-SHA256 constant-time.
- **Kemudahan Pemulihan**: Kegagalan rilis baru dapat segera dipulihkan dalam hitungan detik melalui 1-klik rollback ke direktori rilis sebelumnya.
- **Pembersihan Otomatis**: Pruning otomatis mencegah penumpukan konsumsi disk dari rilis lama sesuai konfigurasi retensi.

### Netral
- Menggunakan shallow clone Git (`--depth 1`) demi kecepatan transfer dan efisiensi ruang disk host; history commit lokal di server sengaja dibatasi pada commit rilis aktif.
