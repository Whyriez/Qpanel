# ADR 0063: Pengumpulan Metrik Aplikasi FPM/Node/Python dan Parser Slowlog (Application Metrics & Slowlog Collection)

## Status
Diterima (Accepted) - Fase 9 (Task P9-T4)

## Konteks & Masalah
Sesuai PRD Bagian 5.6 ("Metrik aplikasi FPM/Node/Python") dan backlog fase 9 (`P9-T4`), Nusa Panel membutuhkan kemampuan observabilitas mendalam untuk runtime aplikasi yang berjalan di atas server:
1. **PHP-FPM Pool Metrics**:
   - Memantau kesehatan proses worker pool (`/fpm-status?json`) via Unix Domain Socket FastCGI (`/run/panel/php/{pool}.sock`).
   - Melacak: active processes, idle processes, total processes, max active processes, max children reached, listen queue, max listen queue, slow requests count, dan accepted connections.
2. **PHP-FPM Slowlog Parser**:
   - Menganalisis berkas trace slowlog per-tenant (`/var/log/panel/<user>/php-fpm-<ver>-slow.log`) yang dihasilkan ketika eksekusi melebihi ambang batas (`request_slowlog_timeout = 5s`).
   - Mengekstrak informasi terstruktur: timestamp kejadian, nama pool, PID proses, path script filename eksekusi, serta deretan frame call stack trace (alamat memori, signature fungsi/method, berkas sumber PHP, nomor baris kode).
   - Mengidentifikasi fungsi paling lambat (*top offending function*) pada frame teratas.
3. **Metrik Proses Aplikasi Node.js & Python (cgroup v2)**:
   - Memantau service unit transien systemd (`site-{id}.service` atau `site-{id}-python.service`) menggunakan antarmuka hierarki cgroup v2 Linux.
   - Mengambil metrik: persentase utilisasi CPU delta %, penggunaan memori saat ini (`memory.current`), puncak memori (`memory.peak`), penggunaan swap (`memory.swap.current`), jumlah proses aktif (`pids.current`), serta laju disk I/O baca/tulis (`io.stat`).
4. **Penyimpanan SQLite & Rollup Agregasi**:
   - Menyimpan metrik mentah (interval 10 detik) dan melakukan rollup otomatis ke bucket 1 menit (`metrics_php_fpm_1m` dan `metrics_app_process_1m`).
   - Menyimpan event slowlog ke tabel `php_slowlog_events` dengan indeks pencarian cepat berdasarkan pool, tenant user, dan site ID.
   - Memastikan retensi dan pembersihan data kedaluwarsa secara otomatis.

## Keputusan Arsitektur

1. **FastCGI Status Protocol & Client (`fpm_status.rs`)**:
   - Implementasi klien FastCGI biner murni (`build_fcgi_status_request` dan `parse_fcgi_status_response`) tanpa dependensi HTTP client berat luar.
   - Membangun frame FastCGI `FCGI_BEGIN_REQUEST`, `FCGI_PARAMS` (`SCRIPT_NAME=/fpm-status`, `QUERY_STRING=json`, `REQUEST_METHOD=GET`), dan `FCGI_STDIN`.
   - Mengurai rekaman `FCGI_STDOUT`, memisahkan header respons, dan mendeserialisasi JSON status payload menjadi model metrik proto `PhpFpmStatusMetrics` dan `PhpFpmPoolMetricPoint`.
   - Mendukung fallback hermetis file `.status.json` untuk pengujian unit deterministik dan lingkungan simulasi lintas platform.

2. **Parser Slowlog PHP Terstruktur (`slowlog.rs`)**:
   - `PhpSlowlogParser` mengurai blok slowlog multi-baris PHP-FPM dengan deteksi header berbasis token `[<date>]  [pool <name>] pid <num>`.
   - Parsing timestamp fleksibel mendukung format bawaan PHP-FPM (`%d-%b-%Y %H:%M:%S`), ISO 8601, dan RFC 3339.
   - Mengurai setiap baris call stack ke dalam `PhpSlowlogFrame` yang memuat `frame_index`, `function`, `file`, `line`, dan `address`.
   - Menyediakan `parse_from_offset` untuk pembacaan inkremental berkas yang terus bertambah (append/stream) dengan penanganan rotasi berkas.

3. **Kolektor Metrik Proses Cgroup v2 (`app_cgroup.rs`)**:
   - `AppProcessMetricsCollector` memanfaatkan pustaka cgroup v2 yang telah divalidasi pada task `P9-T1`.
   - Pencarian direktori cgroup cerdas pada `system.slice/{service}.service` atau root cgroup.
   - Pelacak internal thread-safe (`Mutex<AppTrackerState>`) menghitung delta mikrodetik CPU serta laju transfer byte I/O antar interval sampling secara akurat.
   - Mengembalikan metrik nol yang aman (*graceful fallback*) jika service aplikasi sedang berhenti atau belum dibuat.

4. **Skema Basis Data SQLite & Rollup 1 Menit (`storage.rs`)**:
   - Tabel mentah 10 detik: `metrics_php_fpm_raw` dan `metrics_app_process_raw`.
   - Tabel rollup 1 menit: `metrics_php_fpm_1m` dan `metrics_app_process_1m` dengan agregasi rata-rata, nilai maksimum, dan delta kumulatif (untuk accepted connections dan slow requests).
   - Tabel event: `php_slowlog_events` dengan representasi JSON untuk frame call stack lengkap.
   - Penebas retensi `prune_expired` diperluas untuk membersihkan metrik mentah PHP/App (retensi 2 jam) dan metrik 1 menit serta slowlog (retensi 14 hari).

## Konsekuensi & Kepatuhan
- **Zero Raw Shell Commands**: Seluruh interaksi dengan FastCGI dan cgroup v2 dilakukan langsung via socket stream dan berkas filesystem tanpa string interpolation shell.
- **Kepatuhan Aturan Proyek**: Tidak ada penggunaan `unwrap()` / `expect()` pada kode produksi `panel-analytics`.
- **Kualitas Kode**: 100% lolos verifikasi `cargo fmt --check` dan `cargo clippy -- -D warnings`.
- **Pengujian Lengkap**: Seluruh fungsionalitas diuji melalui 7 integrasi pengujian baru di `crates/panel-analytics/tests/app_metrics_test.rs` (total 65 tests di `panel-analytics` passing).
