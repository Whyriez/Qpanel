# Benchmark Harness Nusa Panel

> Modul pengukuran performa, pengujian beban (*load testing*), perbandingan tuning, dan validasi target non-fungsional PRD Bab 9.

---

## 1. Pendahuluan

Benchmark harness menyediakan pipeline terpadu untuk mengevaluasi efisiensi, stabilitas, dan throughput arsitektur Nusa Panel dalam menangani berbagai macam beban kerja hosting modern.

Harness ini memenuhi spesifikasi **Phase 10 Task P10-T1** dan memvalidasi secara kuantitatif target terukur di **PRD Bab 9**.

### Fitur Utama

- **Integrasi Multi-Tool**:
  - `oha`: HTTP load generator berbasis Rust modern (TUI / JSON output).
  - `wrk`: HTTP benchmark tool multithreaded berkinerja tinggi.
  - `k6`: Tool load testing modern berorientasi skrip skenario.
  - **Built-in Tokio HTTP Probe**: Engine benchmarking asinkron bawaan tanpa dependensi eksternal, mengeksekusi koneksi TCP mentah / HTTP probe secara hermetis di lingkungan CI/VM.
- **5 Skenario Beban Kerja**:
  1. `static`: Penyajian berkas statis & HTML melalui Nginx.
  2. `php_hello`: Eksekusi minimal PHP FastCGI via Unix Domain Socket terisolasi.
  3. `wordpress_dynamic`: Emulasi CMS dinamis dengan query database, template rendering, dan overhead runtime.
  4. `node_hello`: Aplikasi Node.js native HTTP / API di balik reverse proxy Nginx.
  5. `proxy_upstream`: Reverse proxy Nginx dengan buffering, header forwarding, dan keepalive upstream.
- **Perbandingan Tuning Profil**:
  - Baseline vs OPcache + JIT.
  - Baseline vs Microcaching Nginx (`fastcgi_cache` HIT vs BYPASS).
  - Baseline vs Kompresi HTTP (Gzip / Brotli).
  - Protokol Web: HTTP/1.1 vs HTTP/2 vs HTTP/3 (QUIC).
- **Validasi Kepatuhan PRD Bab 9**:
  - Pengujian otomatis batas RAM idle, RAM beban sedang, cold start API, latensi API p95, waktu provisioning situs, zero-downtime reload Nginx, 100% rollback keselamatan konfigurasi, dan throughput ingest log.

---

## 2. Struktur Perintah CLI

Fitur benchmark terintegrasi langsung ke dalam biner admin `panelctl`:

```bash
panelctl benchmark <SUBCOMMAND>
```

### Subcommand Tersedia

| Subcommand | Deskripsi |
|------------|-----------|
| `run` | Menjalankan satu sesi benchmark terhadap URL target tertentu |
| `compare` | Membandingkan performa antara profil baseline dan profil ter-tuning |
| `validate-targets` | Memvalidasi seluruh target non-fungsional PRD Bab 9 |
| `tools` | Mendeteksi ketersediaan tool pengujian di sistem host (`oha`, `wrk`, `k6`) |

---

## 3. Contoh Penggunaan

### 3.1 Menjalankan Benchmark Mandiri

```bash
# Menjalankan pengujian skenario static dengan built-in engine
panelctl benchmark run \
  --target static \
  --url http://127.0.0.1:8080/index.html \
  --concurrency 50 \
  --duration 30s \
  --format markdown

# Menggunakan oha jika terpasang di sistem
panelctl benchmark run \
  --target php \
  --url http://127.0.0.1:8080/hello.php \
  --tool oha \
  --concurrency 100 \
  --duration 60s \
  --output report-php.json
```

### 3.2 Membandingkan Profil Tuning

```bash
# Membandingkan efek aktivasi OPcache pada beban kerja PHP
panelctl benchmark compare \
  --target php \
  --baseline baseline \
  --tuned opcache_enabled \
  --format markdown

# Membandingkan efek microcaching Nginx pada WordPress dinamis
panelctl benchmark compare \
  --target wordpress \
  --baseline baseline \
  --tuned fastcgi_cache_hit \
  --format json
```

Output perbandingan menyajikan:
- Rasio peningkatan Throughput / RPS (`+X.X%`).
- Reduksi latensi persentil p50, p90, p95, dan p99.
- Analisis stabilitas dan perubahan pemakaian memori RSS.

### 3.3 Validasi Target Non-Fungsional (PRD Bab 9)

```bash
# Menjalankan validasi terhadap semua 10 indikator terukur PRD Bab 9
panelctl benchmark validate-targets --format table

# Ekspor hasil audit ke berkas Markdown
panelctl benchmark validate-targets --format markdown --output docs/benchmarks/audit-report.md
```

### 3.4 Menjalankan Skrip Otomasi VM

Gunakan skrip pembungkus shell untuk mengeksekusi suite lengkap di dalam VM pengujian:

```bash
# Eksekusi seluruh skenario dan tuning
./scripts/benchmark-harness.sh --all

# Validasi target non-fungsional langsung via justfile
just benchmark

# Perbandingan profil tuning cepat
just benchmark-compare
```

---

## 4. Format Laporan & Metrik

Setiap pengujian menghasilkan metrik terstruktur:

- **Throughput**:
  - Total Permintaan (*Total Requests*)
  - Permintaan Berhasil & Gagal
  - Requests Per Second (`RPS`)
  - Throughput Transfer Data (`MB/s`)
- **Distribusi Latensi**:
  - `p50` (Median Latency)
  - `p90` (90th Percentile)
  - `p95` (95th Percentile)
  - `p99` (99th Percentile)
  - Minimum & Maximum Latency
- **Metrik Sistem**:
  - CPU Utilization (`%`)
  - Panel API RSS Memory (`MB`)
  - Panel Agent RSS Memory (`MB`)
  - Total Medium Load Memory (`MB`)

Laporan dapat diekspor dalam format `text` (ringkasan konsol), `markdown` (tabel untuk dokumentasi Git), atau `json` (untuk pemrosesan CI/CD).
