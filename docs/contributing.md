# Panduan Kontribusi Pengembang Nusa Panel

Terima kasih atas ketertarikan Anda untuk berkontribusi pada Nusa Panel! Dokumen ini memandu Anda dalam menyiapkan lingkungan pengembangan, memahami arsitektur, dan mematuhi konvensi kualitas kode proyek.

---

## 1. Nilai & Prinsip Utama

Setiap kontribusi harus selaras dengan prinsip inti Nusa Panel:
1. **Keamanan Tanpa Kompromi**:
   - `panel-api` TIDAK PERNAH dijalankan sebagai root.
   - `panel-agent` hanya menerima perintah bertipe (`AgentRequest`).
   - Dilarang keras menggunakan string interpolation untuk membangun perintah shell (`sh -c "$arg"`). Semua eksekusi argumen wajib dipisahkan via `std::process::Command`.
   - Dilarang menggunakan `unwrap()` atau `expect()` pada kode non-test (gunakan `?` atau `match` dengan error terstruktur).
2. **Kinerja & Efisiensi Memori**:
   - Biner Rust native dioptimalkan agar konsumsi RAM idle total di bawah 65 MB.
   - Hindari dependensi berukuran besar (*heavy dependencies*) yang memperlambat waktu kompilasi atau menambah footprint tanpa persetujuan melalui ADR (*Architectural Decision Record*).
3. **Ketahanan Sistem (*Crash-Only Design*)**:
   - Operasi perubahan sistem (Nginx, unit systemd, nftables) harus bersifat idempoten dengan mekanisme rollback otomatis bila gagal validasi.

---

## 2. Aturan Kerja & Definition of Done (DoD)

Sesuai panduan di `AGENTS.md`, seluruh pekerjaan wajib memenuhi kriteria berikut sebelum digabungkan (*merge*):
- **Satu Task Per Sesi**: Kerjakan satu task terdefinisi (ID `P<fase>-T<n>`) per sesi atau PR. Jangan melompati fase atau menggabungkan task berbeda.
- **Kompilasi & Lint Bersih**:
  ```bash
  cargo fmt --check
  cargo clippy --workspace --all-targets -- -D warnings
  ```
- **Bukti Pengujian**: Setiap fitur atau perbaikan bug wajib disertai unit test atau integration test yang membuktikan penerimaan kriteria (*acceptance criteria*).
- **Integritas Frontend UI**:
  Perubahan antarmuka web wajib lolos:
  ```bash
  npm run typecheck   # atau npx tsc --noEmit
  npm test
  ```
- **Pencatatan Kemajuan & ADR**:
  - Perbarui status task di `docs/PROGRESS.md`.
  - Catat setiap keputusan desain atau penambahan pustaka pihak ketiga di `docs/adr/XXXX-<judul>.md`.

---

## 3. Menyiapkan Lingkungan Pengembangan

> [!CAUTION]
> **JANGAN JALANKAN `panel-agent` DI MESIN HOST PRIBADI!**
> `panel-agent` memodifikasi konfigurasi sistem host (menambah user Linux, mengubah Nginx, aturan firewall nftables, cgroup v2, dan unit systemd). Selalu kembangkan dan uji coba di dalam Virtual Machine (VM).

### 3.1 Menggunakan Skrip Virtual Machine (`scripts/dev-vm.sh`)
Nusa Panel menyediakan utilitas otomatisasi VM berbasis Multipass, Lima, atau Vagrant (Ubuntu 24.04 LTS):

```bash
# 1. Jalankan VM pengembangan
./scripts/dev-vm.sh start

# 2. Masuk ke shell VM
./scripts/dev-vm.sh ssh

# 3. Hentikan VM setelah selesai bekerja
./scripts/dev-vm.sh stop
```

### 3.2 Kebutuhan Alat Lokal (Host / VM)
- **Rust Toolchain**: Versi stable terbaru (1.80+), komponen `rustfmt` dan `clippy`.
- **Node.js**: Versi LTS 20.x ke atas.
- **Paket Sistem (di dalam VM)**: `nginx`, `sqlite3`, `nftables`, `pkg-config`, `libssl-dev`.

---

## 4. Struktur Repositori

```
vpanel/
├── crates/
│   ├── panel-core/     # Abstraksi domain bersama, vault crypto, diagnosa, backup
│   ├── panel-agent/    # Daemon sistem root privileged (IPC via socket domain UNIX)
│   ├── panel-api/      # Server REST API HTTP (Axum) non-root, utoipa OpenAPI
│   └── panel-cli/      # Alat baris perintah admin host (panelctl)
├── web/                # Antarmuka web pengguna & admin (React + TypeScript)
├── scripts/            # Skrip instalasi, pemeliharaan dev VM, dan build rilis
├── docs/               # Dokumentasi sistem, panduan admin/tenant, & ADR
└── Cargo.toml          # Workspace root manifest
```

---

## 5. Menjalankan Pengujian (Testing)

### 5.1 Menjalankan Test Rust Workspace
```bash
# Menjalankan seluruh test di workspace
cargo test --workspace

# Menjalankan test per-crate spesifik
cargo test -p panel-core
cargo test -p panel-agent
cargo test -p panel-api
cargo test -p panel-cli
```

### 5.2 Menjalankan Frontend Web Checks
```bash
cd web
npm run lint
npx tsc --noEmit
npm test
```

---

## 6. Konvensi Commit (Conventional Commits)

Gunakan format commit pesan ringkas, jelas, dan menggunakan standar Conventional Commits:
```
<tipe>(<lingkup>): <deskripsi singkat perubahan>
```

### Contoh Tipe:
- `feat`: Menambahkan fitur baru (contoh: `feat(api): tambah endpoint ekspor openapi 3.1`).
- `fix`: Memperbaiki kesalahan/bug (contoh: `fix(agent): perbaiki izin cgroup slice pids`).
- `test`: Menambah atau memodifikasi pengujian (contoh: `test(cli): tambah test upgrade rollback`).
- `docs`: Dokumentasi atau ADR (contoh: `docs(adr): catat keputusan openapi auto-export`).
- `refactor`: Restrukturisasi kode tanpa mengubah perilaku eksternal.

---

## 7. Pembuatan Architectural Decision Records (ADR)

Setiap keputusan arsitektural yang berdampak besar (misalnya: penambahan dependensi baru, perubahan skema database besar, revisi protokol IPC) wajib dicatat menggunakan berkas di folder `docs/adr/`.

Format berkas: `docs/adr/XXXX-<nama-keputusan-huruf-kecil>.md` dengan struktur:
1. **Status**: PROPOSED, ACCEPTED, SUPERSEDED, atau REJECTED.
2. **Konteks**: Latar belakang masalah yang dihadapi.
3. **Keputusan**: Solusi teknis yang dipilih dan alasannya.
4. **Konsekuensi**: Dampak positif, trade-off, dan konsekuensi pemeliharaan.
