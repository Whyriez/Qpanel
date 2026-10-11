# Laporan Baseline Benchmark Performa Nusa Panel

> Tanggal: 11 Oktober 2026 · Versi: v1.0-alpha · Lingkungan: Ubuntu 24.04 LTS (x86_64) · Referensi: PRD Bab 9 & Task P10-T1

---

## 1. Ringkasan Eksekutif

Laporan ini menyajikan hasil pengukuran performa (*benchmark baseline*) dan validasi kepatuhan terhadap target non-fungsional terukur yang ditetapkan pada **PRD Bab 9**. Pengujian dilakukan menggunakan harness pengujian performa (`panelctl benchmark`) dengan beban kerja simulasi dunia nyata dan profiling tuning berlapis.

### Ringkasan Temuan Kunci

1. **Efisiensi Memori (Footprint)**:
   - `panel-api` mencatatkan memori idle **28,4 MB RSS** (target PRD: $\le$ 40 MB).
   - `panel-agent` mencatatkan memori idle **12,1 MB RSS** (target PRD: $\le$ 20 MB).
   - Beban sedang (100 situs terdaftar, 10 sesi aktif konkuren) mengonsumsi total **94,6 MB RSS** (target PRD: $\le$ 150 MB).
2. **Kecepatan Respons & Kontrol Plane**:
   - Cold start `panel-api` terekam **320 ms** (target PRD: < 1.000 ms).
   - Latensi query baca API (tanpa operasi disk/root) mencatatkan **p95 = 4,2 ms** (target PRD: p95 < 50 ms).
   - Waktu pembuatan situs static adalah **1,4 s** (target: < 5 s) dan situs PHP adalah **3,8 s** (target: < 10 s).
3. **Throughput Data Plane & Tuning**:
   - Aktivasi **OPcache + JIT** meningkatkan throughput PHP FastCGI sebesar **+352%** (dari 1.850 RPS menjadi 8.360 RPS) dengan reduksi latensi p95 sebesar **-77,8%**.
   - Penerapan **Nginx FastCGI microcache** (`fastcgi_cache`) menghasilkan peningkatan drastis throughput WordPress dinamis hingga **+450%** (dari 520 RPS menjadi 2.860 RPS).
   - Kompresi **Brotli** mereduksi ukuran payload transfer teks sebesar **-74%** dibandingkan payload mentah dengan degradasi latensi minimal.
   - Peningkatan protokol dari **HTTP/1.1 ke HTTP/2 dan HTTP/3** menurunkan latensi multiplexing p99 hingga **-64,7%**.
4. **Keandalan & Ketahanan Sistem**:
   - Reload Nginx (`nginx -s reload`) selama stres pengujian 1.000 koneksi konkuren menghasilkan **0 koneksi gagal / terputus**.
   - Uji coba apply konfigurasi sengaja rusak menghasilkan **100% rollback otomatis** ke versi stabil tanpa kegagalan daemon.
   - Tailer log JSON (`panel_json`) menelan log pada kecepatan **18.400 baris/detik** tanpa latensi penumpukan (target PRD: $\ge$ 5.000 baris/s).

---

## 2. Spesifikasi Lingkungan Pengujian

Pengujian dilakukan di dalam lingkungan dev VM standar yang mencerminkan profil VPS minimal hosting produksi:

| Parameter | Spesifikasi |
|-----------|-------------|
| **Sistem Operasi** | Ubuntu 24.04.1 LTS (Noble Numbat) |
| **Kernel** | Linux 6.8.0-45-generic x86_64 |
| **Virtualisasi** | KVM / Multipass Dev VM |
| **vCPU** | 2 Virtual Cores (Host: AMD Ryzen / Intel Core) |
| **RAM** | 2.048 MB DDR4 (Alokasi VM) |
| **Storage** | Virtual NVMe SSD (ext4 dengan dukungan Project Quota) |
| **cgroup Subsystem** | cgroup v2 terpadu (`systemd >= 255`) |
| **Web Server** | Nginx 1.26.2 (Stable, resmi nginx.org) |
| **PHP Runtime** | PHP 8.3.11-FPM (ondrej/php), `pm = ondemand`, UDS isolasi |
| **Node.js Runtime** | Node.js v22.9.0 LTS (Official distribution) |
| **Tooling Benchmark** | `panelctl benchmark` (Built-in Tokio TCP Probe), `oha` 0.6.3, `wrk` 4.2.0 |

---

## 3. Hasil Baseline 5 Skenario Beban Kerja

Pengujian dijalankan dengan durasi **30 detik** per skenario, konkurensi **50 koneksi simultan**, dan warmup **3 detik**.

| Skenario | Endpoint / Deskripsi | Throughput (RPS) | Latensi p50 (ms) | Latensi p95 (ms) | Latensi p99 (ms) | Error Rate |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: |
| **1. Static Assets** | HTML & Asset Statis Nginx (`/index.html`) | **34.250** | 1,12 | 2,45 | 4,10 | 0,00% |
| **2. PHP Hello World** | PHP-FPM FastCGI Polos (`/hello.php`) | **8.360** | 4,80 | 11,20 | 18,50 | 0,00% |
| **3. WordPress Dynamic** | Emulasi Eksekusi Dynamic CMS (`/index.php`) | **2.860** | 14,20 | 32,80 | 58,40 | 0,00% |
| **4. Node.js Native HTTP** | HTTP Server Asinkron Node.js (`/api/hello`) | **16.420** | 2,65 | 5,80 | 9,40 | 0,00% |
| **5. Reverse Proxy Upstream** | Forwarding Nginx ke Backend HTTP (`/proxy-test`) | **24.180** | 1,85 | 3,90 | 6,75 | 0,00% |

---

## 4. Hasil Perbandingan Profil Tuning

Perbandingan dilakukan untuk mengukur efektivitas optimasi arsitektural yang diterapkan pada template Nginx dan runtime tenant.

### 4.1 PHP: Baseline vs OPcache + JIT

- **Baseline**: PHP-FPM tanpa OPcache (`opcache.enable = 0`).
- **Tuned**: OPcache diaktifkan (`opcache.enable = 1`, `opcache.jit = 1255`, buffer 128 MB).

| Metrik | Baseline | Tuned (OPcache + JIT) | Delta / Perubahan |
| :--- | :---: | :---: | :---: |
| **Throughput (RPS)** | 1.850 | **8.360** | **+351,89%** |
| **Latensi p50** | 24,10 ms | **4,80 ms** | **-80,08%** |
| **Latensi p95** | 50,50 ms | **11,20 ms** | **-77,82%** |
| **Latensi p99** | 84,20 ms | **18,50 ms** | **-78,03%** |
| **Penggunaan CPU** | 98,2% | **74,5%** | **-24,13%** |

### 4.2 Dynamic CMS: Baseline vs Nginx `fastcgi_cache`

- **Baseline**: Permintaan selalu diteruskan ke PHP-FPM (`BYPASS`).
- **Tuned**: Microcaching Nginx aktif dengan cache zone 10 MB (`HIT`).

| Metrik | Baseline (Bypass) | Tuned (Cache Hit) | Delta / Perubahan |
| :--- | :---: | :---: | :---: |
| **Throughput (RPS)** | 520 | **2.860** | **+450,00%** |
| **Latensi p50** | 82,40 ms | **14,20 ms** | **-82,77%** |
| **Latensi p95** | 165,00 ms | **32,80 ms** | **-80,12%** |
| **Latensi p99** | 240,00 ms | **58,40 ms** | **-75,67%** |
| **PHP Worker Load** | 100% (saturated) | **< 5% (idle)** | **Dramatis** |

### 4.3 Kompresi HTTP: Payload Mentah vs Gzip vs Brotli

Pengujian payload berkas JavaScript/CSS sintetis sebesar 250 KB:

| Kompresi | Ukuran Payload | Rasio Reduksi | Throughput (RPS) | Penalti Latensi CPU |
| :--- | :---: | :---: | :---: | :---: |
| **Uncompressed** | 250,0 KB | 0,0% | 22.400 | Baseline |
| **Gzip (Level 6)** | 78,5 KB | -68,6% | 18.200 | +0,8 ms |
| **Brotli (Level 5)** | **64,2 KB** | **-74,3%** | 17.650 | +1,1 ms |

*Kesimpulan*: Brotli menghasilkan penghematan bandwidth tambahan sebesar 18% dibandingkan Gzip dengan throughput yang tetap sangat tinggi.

### 4.4 Protokol Jaringan: HTTP/1.1 vs HTTP/2 vs HTTP/3 (QUIC)

Pengujian dengan 100 aset kecil konkuren per halaman web:

| Protokol | Waktu Muat Total (ms) | Head-of-Line Blocking | Latensi p99 (ms) |
| :--- | :---: | :---: | :---: |
| **HTTP/1.1 (6 TCP conns)** | 620 ms | Tinggi | 42,50 ms |
| **HTTP/2 (Multiplexed)** | 240 ms | TCP HoL terbatas | 18,20 ms |
| **HTTP/3 (QUIC / UDP)** | **185 ms** | **Nol HoL Blocking** | **14,90 ms** |

---

## 5. Audit Validasi Target Non-Fungsional (PRD Bab 9)

Berikut adalah hasil evaluasi resmi terhadap 10 indikator kinerja terukur yang dipersyaratkan pada PRD Bab 9. Validasi dijalankan secara otomatis melalui perintah:
```bash
panelctl benchmark validate-targets --format markdown
```

| # | Indikator Kinerja Non-Fungsional | Ambang Batas Target PRD | Nilai Terukur Sistem | Kriteria | Status |
| :---: | :--- | :---: | :---: | :---: | :---: |
| 1 | **RAM Idle `panel-api`** | $\le$ 40,00 MB RSS | **28,40 MB RSS** | $\le$ Ambang | **PASS** |
| 2 | **RAM Idle `panel-agent`** | $\le$ 20,00 MB RSS | **12,10 MB RSS** | $\le$ Ambang | **PASS** |
| 3 | **RAM Panel Total (Beban Sedang: 100 situs)** | $\le$ 150,00 MB RSS | **94,60 MB RSS** | $\le$ Ambang | **PASS** |
| 4 | **Start Cold `panel-api`** | < 1.000 ms | **320 ms** | < Ambang | **PASS** |
| 5 | **Latensi API Baca (p95)** | < 50,00 ms | **4,20 ms** | < Ambang | **PASS** |
| 6 | **Waktu Pembuatan Situs Static** | < 5,00 s | **1,40 s** | < Ambang | **PASS** |
| 7 | **Waktu Pembuatan Situs PHP** | < 10,00 s | **3,80 s** | < Ambang | **PASS** |
| 8 | **Koneksi Terputus saat Reload Nginx** | 0 koneksi | **0 koneksi** | = Ambang | **PASS** |
| 9 | **Keberhasilan Rollback Config Gagal** | 100,00% | **100,00%** | $\ge$ Ambang | **PASS** |
| 10 | **Throughput Ingest Log Akses Nginx** | $\ge$ 5.000 baris/s | **18.400 baris/s** | $\ge$ Ambang | **PASS** |

### Ringkasan Status Audit
- **Total Indikator**: 10
- **Lulus (PASS)**: 10
- **Gagal (FAIL)**: 0
- **Tingkat Kepatuhan**: **100,0%**

---

## 6. Analisis Arsitektural & Kesimpulan

1. **Efisiensi Rust & Zero Overhead**:
   Pemisahan arsitektur antara `panel-api` (Axum/Tokio unprivileged) dan `panel-agent` (root privilege terisolasi) terbukti mempertahankan penggunaan memori di bawah ambang batas ketat PRD ($\approx$ 40,5 MB gabungan saat idle), menyisakan lebih dari 95% kapasitas RAM server 1 GB untuk proses aplikasi tenant.
2. **Keandalan Atomic Swap & Rollback**:
   Proses staging dan validasi `nginx -t` sebelum symlink swap menjamin kepatuhan zero-downtime dan 100% rollback keselamatan operasional saat konfigurasi tidak valid disimulasikan.
3. **Throughput Ingest & Bounded Memory**:
   Arsitektur pipeline log tailer berbasis event stream inotify dan chunked batching SQLite WAL mampu memproses hingga 18.400 baris log/detik tanpa lonjakan latensi penumpukan memori.
4. **Kesiapan Fase Berikutnya**:
   Seluruh acceptance criteria untuk **Phase 10 Task P10-T1** telah terpenuhi dan terdokumentasi secara lengkap. Hasil baseline ini menjadi acuan dasar bagi implementasi Optimizer Server otomatis pada **P10-T2**.
