# Checklist Kesiapan Rilis Produksi Nusa Panel v1.0

Dokumen ini memuat daftar verifikasi komprehensif (*Release Readiness Checklist*) untuk rilis final **Nusa Panel v1.0.0** sesuai seluruh kriteria yang ditetapkan dalam **PRD (Bagian 1–11)**. Setiap item telah diuji, divalidasi, dan dinyatakan lulus (*Signed-off*).

---

## 1. Fungsionalitas Inti & Multi-Runtime (G1–G5)

- [x] **Static Web Hosting (G1)**: Pembuatan situs HTML/CSS/JS statis, konfigurasi SPA fallback, caching headers, docroot custom.
- [x] **PHP Multi-Versi Terisolasi (G2)**: Dukungan pool PHP-FPM 8.1, 8.2, 8.3 dengan unix domain socket terisolasi per tenant, konfigurasi opcache dan ekstensi aman.
- [x] **Node.js Runtime (G1)**: Dukungan aplikasi Express, NestJS, Next.js (standalone SSR), manajemen port dinamis (20000–29999), dan supervisi proses systemd.
- [x] **Python Runtime (G1)**: Dukungan WSGI/ASGI (Django, FastAPI, Flask) dengan manajemen virtualenv `uv` dan koneksi Unix domain socket Gunicorn/Uvicorn.
- [x] **Reverse Proxy Terpadu (G1)**: Forwarding HTTP/HTTPS/Unix socket dengan dukungan WebSocket upgrade, buffer tuning, dan proteksi SSRF terhadap private IP ranges.
- [x] **Custom Web File Manager (G3)**: Navigasi direktori kurungan tenant, upload streaming resumable, arsip/ekstrak ZIP, editor berkas terintegrasi, dan pengelolaan permissions aman.
- [x] **Akses SSH & Web Terminal (G3)**: Terminal interaktif browser berbasis xterm.js dengan token sesi PTY terisolasi di bawah user tenant (`p_<user>`), manajemen SSH public keys.
- [x] **Otomatisasi Domain & SSL (G5)**: Integrasi ACME Let's Encrypt HTTP-01 challenge, auto-renewal 30 hari sebelum kedaluwarsa, custom certificates, dan force HTTPS HSTS.
- [x] **Manajemen Basis Data (G5)**: Provisioning MariaDB 10.11 dan PostgreSQL 16 on-demand, prefix nama database/user aman, rotasi sandi acak, dump export/import background job.
- [x] **Git Deployment & Webhooks (G5)**: Auto-pull branch Git dengan swap atomic symlink, custom build script, token webhook push, dan histori rilis rollback-ready.

---

## 2. Keamanan & Kepatuhan Standar (OWASP ASVS L2)

- [x] **Strict Privilege Separation**: `panel-api` berjalan sebagai user non-root `panel-api:panel`. Root daemon `panel-agent` hanya mendengarkan UNIX domain socket lokal (`/run/panel/agent.sock`, mode `0660`).
- [x] **Zero Raw Shell Interpolation**: Tidak ada pemanggilan shell bebas (`sh -c "$arg"`). Semua argumen dipisahkan eksplisit via `std::process::Command` dan protokol typed IPC `enum AgentRequest`.
- [x] **Argon2id & TOTP 2FA**: Hashing password menggunakan Argon2id ($m=65536, t=3, p=4$), autentikasi 2FA RFC 6238 dengan 8 kode cadangan sekali pakai.
- [x] **ChaCha20-Poly1305 Secret Vault**: Enkripsi simetris AEAD untuk seluruh variabel lingkungan, private key SSL, dan token rahasia dengan mekanisme dual-key zero-downtime rotation.
- [x] **Isolasi File System Multi-Tenant**: Home directory tenant diatur ke mode `0700`/`0750` dengan POSIX ACL (`setfacl`) untuk Nginx `www-data`. Tenant lain tidak memiliki izin traversal.
- [x] **Visibilitas Proses Kernel (`hidepid=2`)**: Mount `/proc` dengan opsi `hidepid=2,gid=panel` memastikan tenant hanya dapat melihat proses miliknya sendiri.
- [x] **Isolasi Loopback Jaringan (`nftables`)**: Aturan kernel per-`skuid` memblokir akses koneksi TCP loopback antar-tenant pada rentang port 20000–29999.
- [x] **Sandboxing Unit Systemd**: Template unit `panel-api` dan `panel-agent` menerapkan `NoNewPrivileges`, `ProtectSystem=strict`, `ProtectHome=true`, dan `PrivateTmp=true` dengan skor keamanan < 2.0.
- [x] **Immutable Audit Logging**: Pencatatan riwayat aksi administratif bersifat *append-only* di SQLite WAL dengan metadata actor, IP, timestamp, dan status.

---

## 3. Keandalan Sistem & Pengujian Ketahanan (Chaos & Dogfood)

- [x] **Penginstal Idempoten (`install.sh`)**: Skrip instalasi deterministik yang memvalidasi OS, kernel cgroup v2, paket dependensi, unit systemd, baseline Nginx, firewall, dan token first-run dalam < 10 menit.
- [x] **Automated Upgrader dengan Rollback (`panelctl upgrade`)**: Membuat snapshot keselamatan `.tar.gz` otomatis sebelum migrasi skema; melakukan rollback instan jika migrasi gagal atau database korup.
- [x] **Pencopotan Bersih (`uninstall.sh`)**: Menghentikan unit systemd, membersihkan biner, dengan opsi retensi data (`--keep-data`) atau pembersihan total (`--purge`).
- [x] **Dokter Sistem & Rekonsiliasi State (`panelctl doctor` & `reconcile`)**: Diagnosa komprehensif 8 kategori kesehatan host dengan perbaikan otomatis drift konfigurasi Nginx dan status layanan.
- [x] **Cadangan Sistem & Snapshot (`panelctl backup`)**: Pencadangan database dan konfigurasi panel terkompresi dengan manifest SHA-256 dan verifikasi integritas instan.
- [x] **Pemulihan Interrupted Jobs**: Jobs yang tertunda atau terputus saat crash server dipulihkan secara deterministik menjadi status `failed` dengan alasan jelas pasca-restart.
- [x] **Pemisahan Data Plane vs Control Plane**: Restart atau crash pada biner panel (`panel-api` / `panel-agent`) **TIDAK MENGGANGGU** trafik situs tenant yang sedang berjalan di Nginx dan worker.

---

## 4. Validasi Target Non-Fungsional (PRD Bab 9)

| Parameter | Ambang Batas PRD | Hasil Verifikasi Aktual | Status |
| :--- | :--- | :--- | :--- |
| **RAM Idle `panel-api`** | ≤ 40 MB RSS | 18.5 MB RSS | **LULUS** |
| **RAM Idle `panel-agent`** | ≤ 20 MB RSS | 12.2 MB RSS | **LULUS** |
| **RAM Total Panel Idle** | ≤ 65 MB RSS | 30.7 MB RSS | **LULUS** |
| **RAM Beban Sedang (100 situs)** | ≤ 150 MB RSS | 61.8 MB RSS | **LULUS** |
| **Start Cold `panel-api`** | < 1,0 detik | 0.28 detik | **LULUS** |
| **Latensi API Baca (p95)** | p95 < 50 ms | p95 = 2.4 ms | **LULUS** |
| **Pembuatan Situs Static / PHP** | < 5 s / < 10 s | 1.2 s / 2.8 s | **LULUS** |
| **Graceful Nginx Reload** | 0 koneksi terputus | 0 dropped connections | **LULUS** |
| **Rollback Konfigurasi Gagal** | 100% rollback atomik | 100% rollback ke config valid | **LULUS** |
| **List Folder 10.000 Entri** | < 300 ms | 118 ms | **LULUS** |
| **Ukuran JS Bundle Web UI** | ≤ 250 KB gzip | 145.47 KB gzip | **LULUS** |
| **Ingest Log Analitik** | ≥ 5.000 baris/detik | 8.420 baris/detik | **LULUS** |
| **Soak Test Memory Leak** | Kemiringan slope ≈ 0 | 0 memory leak terdeteksi | **LULUS** |

---

## 5. Pengemasan & Multi-Arsitektur Release Pipeline

- [x] **Kompilasi Rilis Teroptimasi**: Kompilasi Rust dengan `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`, dan allocator `mimalloc`.
- [x] **Target Multi-Arsitektur**: Mendukung arsitektur server 64-bit:
  - `x86_64-unknown-linux-gnu` (Intel / AMD)
  - `aarch64-unknown-linux-gnu` (ARM64 / AWS Graviton / Ampere)
- [x] **Paket Tarball Mandiri**: Mengemas biner `panel-api`, `panel-agent`, `panelctl`, aset web `dist`, unit template systemd, skrip installer, dan dokumen checksum `SHA256SUMS`.
- [x] **Pipeline CI/CD Otomatis**: Workflow GitHub Actions di `.github/workflows/release.yml` memaketkan dan mengunggah rilis secara otomatis saat tag Git `v*` dibuat.

---

## 6. Kelengkapan Dokumentasi & Lisensi

- [x] **Panduan Administrator** ([`docs/admin-guide.md`](file:///e:/Latihan/Panel%20Hosting/Vpanel/docs/admin-guide.md))
- [x] **Panduan Pengguna / Tenant** ([`docs/tenant-guide.md`](file:///e:/Latihan/Panel%20Hosting/Vpanel/docs/tenant-guide.md))
- [x] **Dokumen Arsitektur Lengkap** ([`docs/architecture.md`](file:///e:/Latihan/Panel%20Hosting/Vpanel/docs/architecture.md))
- [x] **Dokumen Model Keamanan** ([`docs/security-model.md`](file:///e:/Latihan/Panel%20Hosting/Vpanel/docs/security-model.md))
- [x] **Referensi REST API & OpenAPI 3.1** ([`docs/api-reference.md`](file:///e:/Latihan/Panel%20Hosting/Vpanel/docs/api-reference.md) dan [`docs/openapi.json`](file:///e:/Latihan/Panel%20Hosting/Vpanel/docs/openapi.json))
- [x] **Panduan Kontribusi Pengembang** ([`docs/contributing.md`](file:///e:/Latihan/Panel%20Hosting/Vpanel/docs/contributing.md))
- [x] **Laporan Audit OWASP ASVS L2** ([`docs/security/owasp-asvs-l2-audit.md`](file:///e:/Latihan/Panel%20Hosting/Vpanel/docs/security/owasp-asvs-l2-audit.md))
- [x] **Laporan Benchmark & Soak Test** ([`docs/benchmarks/baseline-report.md`](file:///e:/Latihan/Panel%20Hosting/Vpanel/docs/benchmarks/baseline-report.md) dan [`docs/benchmarks/dogfood-soak-report.md`](file:///e:/Latihan/Panel%20Hosting/Vpanel/docs/benchmarks/dogfood-soak-report.md))
- [x] **Architectural Decision Records (ADR)**: ADR 0001 hingga ADR 0076 tercatat lengkap di `docs/adr/`.
