# Panduan Lingkungan Pengembangan (Dev Environment)

Dokumen ini menjelaskan tata cara pengembangan Nusa Panel sesuai dengan prinsip isolasi dan keamanan yang ditentukan dalam PRD.

## 1. Aturan Keras Keamanan Dev

> [!CAUTION]
> **JANGAN PERNAH** menjalankan `panel-agent` langsung di laptop/mesin host developer!
> `panel-agent` memodifikasi konfigurasi sistem tingkat rendah (user Linux, grup, Nginx vhost, cgroup slice systemd, dan filesystem ACL).
> Selalu gunakan VM dev Ubuntu 24.04 (Multipass atau Lima).

> [!IMPORTANT]
> `panel-api` **TIDAK PERNAH** dijalankan sebagai root, baik dalam produksi maupun mode pengembangan.

---

## 2. Menjalankan VM Dev

Skrip otomatis tersedia di `scripts/dev-vm.sh`.

### Prasyarat
Instal salah satu virtualisasi:
- [Multipass](https://multipass.run/) (direkomendasikan untuk Windows/macOS/Linux)
- [Lima](https://lima-vm.io/) (alternatif macOS/Linux)

### Perintah Utama
```bash
# 1. Buat dan siapkan VM Ubuntu 24.04 dengan repo ter-mount
./scripts/dev-vm.sh up

# 2. Masuk ke terminal shell VM
./scripts/dev-vm.sh shell

# 3. Jalankan suite pengujian di dalam VM
./scripts/dev-vm.sh test
# atau via just:
just test-vm

# 4. Hentikan VM jika selesai bekerja
./scripts/dev-vm.sh stop
```

---

## 3. Workflow Pengembangan Harian

### Struktur Proyek
- `crates/panel-api`: API HTTP (Axum), otentikasi, runner pekerjaan latar, host aset UI.
- `crates/panel-agent`: Daemon root pelaksana operasi sistem bertipe.
- `crates/panel-proto`: Skema protokol RPC IPC bertipe antara API dan Agent.
- `crates/panel-core`: Tipe domain, newtype validator (Username, Domain, Port).
- `web/`: Frontend React 19 + TypeScript + Tailwind CSS v4.

### Menjalankan Linter & Format
Sebelum commit, pastikan semua pengecekan bersih:
```bash
just lint
# Ekivalen dengan:
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cd web && npm run lint && npm run check
```

### Menjalankan Test
```bash
just test
# Ekivalen dengan:
cargo test --workspace
cd web && npm test -- --run
```
