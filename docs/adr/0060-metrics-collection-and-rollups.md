# ADR 0060: Koleksi Metrik Sistem dan Agregasi Rollup (System Metrics Collection & Rollups)

## Status
Diterima (Accepted) - Fase 9 (Task P9-T1)

## Konteks & Masalah
Sesuai PRD Bagian 7.1 dan backlog fase 9 (`P9-T1`), sistem Vpanel membutuhkan pemantauan metrik sumber daya host server dan tenant secara real-time dan historis:
1. **Cakupan Metrik**:
   - **Host/Server**: CPU % utilization, core count, RAM used/total, swap used/total, load average (1m/5m/15m), disk space used/total, inode used/total, network rx/tx bytes/sec, disk read/write bytes/sec.
   - **Tenant (cgroup v2)**: Penggunaan resource per tenant slice (`panel-u{uid}.slice`): CPU usec & %, memory current, peak, swap, process count (PIDs), disk IO read/write bytes/sec.
   - **Service (cgroup v2)**: Pemantauan service inti seperti Nginx, MariaDB, PHP-FPM (`system.slice/{service}.service`).
2. **Kebutuhan Penyimpanan & Rollup**:
   - Interval pengambilan data raw setiap 10 detik.
   - Data historis diagregasi ke bucket 1 menit, 1 jam, dan 1 hari.
   - Retensi data: raw disimpan 2 jam, rollup 1 menit disimpan 14 hari, rollup 1 jam disimpan 400 hari, rollup 1 hari disimpan tak terbatas / sesuai konfigurasi.
   - Tidak boleh mengganggu database operasional `panel.db`. Penyimpanan metrik harus di database SQLite mandiri (`analytics.db`) dengan mode WAL.
   - Tidak menggunakan external monitoring agent (seperti Prometheus daemon atau Telegraf) untuk instalasi baseline agar panel tetap mandiri dan ringan.

## Keputusan Arsitektur

1. **Modul Rust `panel-analytics` Mandiri**:
   - Membangun crate `crates/panel-analytics` yang bertugas khusus untuk pembacaan metrik Linux host, cgroup v2 tenant & service, agregasi SQL, serta retensi.

2. **Pembaca Linux `/proc` (`ProcReader`)**:
   - Membaca `/proc/stat` untuk CPU total user/nice/system/idle/iowait ticks dan menghitung delta CPU % antar interval.
   - Membaca `/proc/meminfo` untuk memori terpakai (`MemTotal - MemAvailable`) dan swap.
   - Membaca `/proc/loadavg` untuk load 1m, 5m, 15m.
   - Membaca `/proc/net/dev` untuk menghitung laju transfer network interface utama (rx/tx bytes/sec).
   - Membaca `/proc/diskstats` untuk laju I/O disk block devices (read/write sectors -> bytes/sec).

3. **Kolektor cgroup v2 (`CgroupMetricsCollector`)**:
   - Mendeteksi direktori slice tenant `panel-u{uid}.slice` dan service `system.slice/{service}.service`.
   - Mengambil metrik: `cpu.stat` (`usage_usec`), `memory.current`, `memory.peak`, `memory.swap.current`, `pids.current`, dan `io.stat` (`rbytes`, `wbytes`).
   - Menyimpan timestamp dan counter sebelumnya di memory tracker untuk menghitung delta CPU % dan disk I/O rate secara akurat.

4. **Pembaca Mount File System (`DiskUsageReader`)**:
   - Menggunakan `statvfs` pada sistem Unix untuk membaca ukuran total blok, ruang bebas, serta total dan penggunaan inode mount root `/`.
   - Menyediakan interface mock untuk lingkungan pengujian unit dan pengembangan lintas sistem operasi.

5. **Penyimpanan Database SQLite `analytics.db` (`AnalyticsStorage`)**:
   - Menggunakan database SQLite terpisah dengan `PRAGMA journal_mode = WAL`, `synchronous = NORMAL`, dan `busy_timeout = 5000ms`.
   - Skema tabel:
     - Raw: `metrics_server_raw`, `metrics_tenant_raw`, `metrics_service_raw`.
     - 1-minute rollup: `metrics_server_1m`, `metrics_tenant_1m`, `metrics_service_1m`.
     - 1-hour rollup: `metrics_server_1h`, `metrics_tenant_1h`, `metrics_service_1h`.
     - 1-day rollup: `metrics_server_1d`, `metrics_tenant_1d`, `metrics_service_1d`.
   - Agregasi rollup dieksekusi langsung melalui query agregasi SQL (`AVG`, `MAX`, `MIN`, `COUNT(*)`, `GROUP BY bucket_ts`) yang sangat cepat dan hemat memori.
   - Fungsi pembersihan (`prune_expired`) menghapus data kedaluwarsa secara berkala sesuai kebijakan `RetentionConfig`.

6. **Orkestrator Koleksi Metrik (`SystemMetricsCollector`)**:
   - Mengintegrasikan `ProcReader`, `CgroupMetricsCollector`, `DiskUsageReader`, dan `AnalyticsStorage`.
   - Menyediakan metode `collect_and_store` untuk menjalankan pengumpulan snapshot 10 detik.
   - Menyediakan metode `execute_rollups_and_retention` untuk rollup bertahap dan pruning data lama.

## Konsekuensi

### Positif
- **Kemandirian Penuh**: Tidak memerlukan dependency atau daemon monitoring pihak ketiga di server host.
- **Isolasi Database**: `analytics.db` terpisah sepenuhnya dari `panel.db`, memastikan penulisan metrik berfrekuensi tinggi (10 detik) tidak memblokir query API atau transaksi akun dan situs.
- **Kueri Cepat & Efisien**: Agregasi rollup 1m, 1h, dan 1d memungkinkan grafik historis dalam rentang waktu berbulan-bulan di-load dalam milidetik tanpa memindai jutaan baris raw.
- **Keamanan & Standar Kode**: Mengikuti prinsip zero `unwrap()`/`expect()` di kode produksi, compile bersih tanpa warning clippy.

### Negatif / Mitigasi
- Database SQLite dapat tumbuh seiring jumlah tenant; dimitigasi dengan retensi raw yang singkat (2 jam) dan pembersihan otomatis (`prune_expired`).
