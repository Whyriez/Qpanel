# ADR 0069: Performance Benchmark Harness dan Validasi Target Non-Fungsional

## Konteks

Dalam fase akhir menuju produksi (Phase 10 — Performance, hardening, installer, rilis), panel hosting dituntut membuktikan stabilitas, throughput, efisiensi konsumsi memori, dan keandalan operasional secara kuantitatif.

PRD Bab 1.2 (Sasaran G6), Bab 9 (Target Non-Fungsional Terukur), Bab 10 (Strategi Testing), dan Roadmap Phase 10 (Task P10-T1) menetapkan:
1. Pembangunan **benchmark harness** terpadu yang mendukung alat load testing standar industri (`oha`, `wrk`, `k6`) serta engine mandiri untuk 5 skenario beban kerja:
   - Berkas statis & HTML (`static`)
   - PHP FastCGI Hello World (`php_hello`)
   - WordPress / Dynamic CMS Emulation (`wordpress_dynamic`)
   - Node.js Native HTTP / API (`node_hello`)
   - Reverse Proxy Upstream (`proxy_upstream`)
2. Pengukuran dan perbandingan kuantitatif terhadap profil tuning:
   - Baseline vs OPcache + JIT
   - Baseline vs Nginx FastCGI microcaching (`fastcgi_cache` HIT vs BYPASS)
   - Baseline vs Kompresi HTTP (Gzip / Brotli)
   - HTTP/1.1 vs HTTP/2 vs HTTP/3 (QUIC)
3. Validasi otomatis terhadap **10 Target Non-Fungsional Bab 9**:
   - RAM idle `panel-api` $\le$ 40 MB RSS
   - RAM idle `panel-agent` $\le$ 20 MB RSS
   - RAM total panel saat beban sedang (100 situs terdaftar) $\le$ 150 MB RSS
   - Cold start `panel-api` < 1 detik
   - Latensi query baca API p95 < 50 ms
   - Waktu pembuatan situs static < 5 detik, situs PHP < 10 detik
   - Reload Nginx 0 koneksi aktif terputus
   - Rollback konfigurasi gagal 100% pulih
   - Ingest log akses Nginx $\ge$ 5.000 baris/detik
4. Penyusunan laporan baseline di `docs/benchmarks/` dan automasi skrip VM.

---

## Keputusan Arsitektur

### 1. Biner CLI Mandiri & Built-in Tokio TCP Probe di `panel-cli`
- Implementasi harness ditempatkan di dalam crate CLI administrasi (`crates/panel-cli/src/commands/benchmark.rs`).
- Subcommand `panelctl benchmark` (`run`, `compare`, `validate-targets`, `tools`) dipisahkan secara modular pada `main.rs` sehingga dapat dijalankan standalone **tanpa menginisialisasi atau mewajibkan keberadaan basis data `panel.db`**.
- Untuk memastikan harness 100% hermetis dan dapat berjalan di lingkungan VM atau CI tanpa keharusan menginstal biner pihak ketiga (`oha`/`wrk`), dikembangkan engine beban kerja bawaan `run_builtin_benchmark` berbasis raw Tokio async TCP stream:
  - Mengelola worker concurrency pool (`tokio::spawn`),
  - Mengirim HTTP/1.1 keep-alive probe berurutan,
  - Menghitung durasi latensi per request dengan `Instant::now()`,
  - Menghitung persentil presisi (p50, p90, p95, p99) melalui kalkulasi linier array terurut.

### 2. Integrasi Multi-Tool Fleksibel
- Struktur `BenchmarkTool` mendeteksi keberadaan biner eksternal (`oha`, `wrk`, `k6`) via pengecekan `std::process::Command` dengan flag `--version`.
- Jika tool eksternal dipilih atau tersedia, harness membungkus eksekusinya dan mem-parsing output JSON/teks menjadi model standar `BenchmarkRunResult`.
- Bila tool eksternal tidak ditemukan di PATH sistem, harness secara otomatis beralih ke engine built-in tanpa menggagalkan pengujian.

### 3. Model Perbandingan Tuning Kuantitatif
- Didefinisikan model `BenchmarkComparison` yang membandingkan dua hasil pengujian (`baseline` vs `tuned`).
- Menghitung delta persentase throughput (`rps_improvement_pct`), reduksi latensi p50/p95/p99 (`latency_p95_reduction_pct`), serta efisiensi penggunaan sumber daya CPU & RAM.
- Menyediakan serializer hasil ke format Markdown terstruktur untuk publikasi laporan dan JSON untuk analisis CI pipeline.

### 4. Engine Validasi Target Non-Fungsional PRD Bab 9
- Dibuat fungsi `validate_non_functional_targets` dan struktur `NonFunctionalTargetsReport` yang mengevaluasi 10 metrik kunci Bab 9.
- Mengklasifikasikan setiap indikator ke dalam status terukur `PASS` atau `FAIL` berdasarkan operator pembanding (`LessThan`, `LessThanOrEqual`, `GreaterThanOrEqual`, `Exact`).
- Mengembalikan ringkasan status kelulusan (kepatuhan 100%).

### 5. Skrip Otomasi dan Integrasi Tooling
- Dibuat skrip shell pembungkus `scripts/benchmark-harness.sh` yang memfasilitasi eksekusi pengujian di dalam VM (mendukung flag `--scenario`, `--tuning`, `--tool`, `--concurrency`, `--duration`, `--validate-targets`, `--all`).
- Ditambahkan target `just benchmark` dan `just benchmark-compare` pada `justfile`.
- Dokumentasi arsitektur dan laporan baseline disusun di `docs/benchmarks/README.md` dan `docs/benchmarks/baseline-report.md`.

---

## Konsekuensi

### Positif
1. **Verifikasi Terukur & Objektif**: Target PRD Bab 9 dapat diaudit secara otomatis di lingkungan staging maupun CI kapan saja.
2. **Zero Dependency Lock-in**: Pengujian tetap dapat berjalan penuh berkat engine Tokio HTTP bawaan meskipun alat eksternal belum diinstal di server/VM.
3. **Pondasi Optimizer (P10-T2)**: Hasil perbandingan tuning menyediakan basis data kuantitatif yang jelas bagi rule engine optimizer otomatis di task P10-T2.

### Mitigasi
- Pengujian beban tinggi (concurrency $\ge$ 200 koneksi) dijalankan di dalam lingkungan Dev VM terisolasi untuk menghindari kejenuhan soket jaringan pada mesin host pengembang.
