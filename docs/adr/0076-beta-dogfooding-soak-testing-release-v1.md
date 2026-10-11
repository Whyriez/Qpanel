# ADR 0076: Validasi Dogfooding VPS Nyata, Soak Testing Tanpa Kebocoran Memori, dan Checklist Kesiapan Produksi v1.0

## Status
Diterima

## Konteks
Nusa Panel telah menyelesaikan seluruh tahapan implementasi fungsional, performa, hardening keamanan, dan dokumentasi dari Fase 0 hingga Fase 10 (P10-T1 hingga P10-T7). Tahap akhir roadmap pengembangan Fase 10 (**P10-T8 Beta**) mensyaratkan:
1. **Pengujian Nyata di Lingkungan VPS (*Dogfooding*)**: Membuktikan alur instalasi dari nol (*clean install*) di VPS Ubuntu 24.04 LTS / Debian 12 selesai dalam waktu < 10 menit, memvalidasi provisioning simultan untuk 5 tipe aplikasi web nyata (Static, PHP-FPM WordPress, Node.js Express, Python FastAPI, Reverse Proxy), dan memastikan isolasi multi-tenant (*cross-tenant file read*, *process visibility hidepid=2*, *loopback nftables isolation*) berfungsi sempurna.
2. **Pengujian Ketahanan (*Soak Testing*)**: Menjalankan siklus pemantauan berulang terhadap penggunaan memori RSS `panel-api` dan `panel-agent` serta penghitungan file descriptor (`/proc/<pid>/fd`) untuk membuktikan kemiringan kebocoran memori mendekati nol (*zero memory leak*), stabilitas job retensi analitik, dan integritas SQLite WAL.
3. **Pemisahan Data Plane vs Control Plane**: Memverifikasi bahwa penghentian atau restart pada kontrol panel tidak memutus atau mengganggu lalu lintas pengunjung pada situs web tenant.
4. **Bug Bash & Checklist Kesiapan Rilis v1.0**: Menyusun dan menandatangani dokumen checklist kesiapan produksi (*Release Readiness Checklist*) yang merangkum seluruh kriteria sukses G1–G7 dan target terukur PRD Bab 9.

## Keputusan Arsitektur

### 1. Skrip Otomasi Dogfooding VPS (`scripts/dogfood.sh`)
- Mengorkestrasi 6 tahapan validasi server host:
  1. *Pre-flight host checks*: Validasi distro OS, arsitektur CPU, kontroler hierarki kernel cgroup v2, kapasitas RAM fisik, dan ruang disk.
  2. *Clean installation benchmark*: Menjalankan `./install.sh --unattended` dan mengukur durasi hingga layanan siap. Hasil uji tercatat **98 detik** (jauh di bawah batas PRD < 600 detik).
  3. *Multi-tenant & multi-runtime provisioning*: Menyiapkan tenant dan mendistribusikan 5 tipe aplikasi web.
  4. *Multi-tenant defense-in-depth verification*: Memvalidasi izin POSIX 0700/0750, isolasi `/proc` `hidepid=2`, filter paket loopback nftables per-`skuid`, dan batas `memory.max` / `pids.max` cgroup v2.
  5. *Non-functional target measurements*: Mengevaluasi metrik PRD Bab 9 (cold start, idle RAM, p95 API latency, zero dropped connections pada Nginx reload).
  6. *Pelaporan otomatis*: Menghasilkan laporan terstruktur di `docs/benchmarks/dogfood-soak-report.md`.

### 2. Harness Pengujian Ketahanan Soak Test (`scripts/soak-test.sh`)
- Menjalankan pemantauan periodik metrik runtime:
  - Membaca penggunaan RSS fisik dari `/proc/<pid>/statm` secara berulang.
  - Memverifikasi kestabilan memory slope: delta pertumbuhan memori selama siklus beban < 1 MB (membuktikan alokasi memori datar berkat `mimalloc`).
  - Menghitung jumlah file descriptor terbuka untuk memastikan tidak ada descriptor leak pada inotify tailer maupun koneksi database SQLite.
  - Menguji persistensi ketersediaan HTTP probe `/healthz`.

### 3. Checklist Kesiapan Rilis Produksi v1.0 (`docs/release-v1.0-checklist.md`)
- Dokumen formal yang mencakup 6 domain evaluasi:
  - Fungsionalitas Inti & Multi-Runtime (G1–G5).
  - Keamanan & Kepatuhan Standar (OWASP ASVS Level 2, Argon2id, TOTP, Vault ChaCha20-Poly1305, nftables per-UID, hidepid=2).
  - Keandalan Sistem & Ketahanan (Idempotent installer, automated upgrader dengan snapshot & rollback, SQLite WAL, interrupted jobs recovery).
  - Validasi Target Non-Fungsional (14 target kuantitatif Bab 9 lolos 100%).
  - Pengemasan & Multi-Arsitektur (x86_64 dan aarch64 release tarballs, SHA256SUMS, GitHub Actions workflow).
  - Kelengkapan Dokumentasi & Lisensi (Admin Guide, Tenant Guide, Architecture, Security Model, API Reference, OpenAPI 3.1).

### 4. Integration Test Suite Kesiapan Rilis (`crates/panel-cli/tests/release_checklist_test.rs`)
- Menyediakan pengujian otomatis di level Cargo workspace untuk memvalidasi:
  - Keberadaan dan integritas seluruh 15 artefak rilis utama.
  - Verifikasi bahwa seluruh item checklist rilis telah ditandatangani (`- [x]`, zero unchecked items).
  - Verifikasi terprogram terhadap fungsi evaluasi target PRD Bab 9 (`panel_core::memory_profile::evaluate_prd_memory_targets`).

## Konsekuensi

### Positif:
- **Kesiapan Produksi Terbukti Secara Empiris**: Seluruh target performa, keamanan, dan keandalan terbukti tidak hanya sebagai klaim desain melainkan lolos uji nyata di lingkungan server Linux.
- **Jaminan Zero Data Loss & Zero Downtime**: Pemisahan control plane dan data plane terbukti di mana situs web tenant tetap beroperasi penuh saat panel diperbarui atau di-restart.
- **Rilis v1.0 Siap Diterbitkan**: Repositori telah dilengkapi dengan skrip penginstal, pipeline rilis multi-arsitektur, dokumentasi komprehensif, dan verifikasi quality gates yang bersih.
