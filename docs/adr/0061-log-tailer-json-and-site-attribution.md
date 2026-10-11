# ADR 0061: Log Tailer JSON, Inotify, dan Atribusi Situs (Log Tailer JSON & Site Attribution)

## Status
Diterima (Accepted) - Fase 9 (Task P9-T2)

## Konteks & Masalah
Sesuai PRD Bagian 5.6 ("Analytics mendalam") dan backlog fase 9 (`P9-T2`), modul analitik Vpanel memerlukan tailer log akses Nginx JSON yang tangguh, efisien, dan andal:
1. **Format Log Sumber**:
   Nginx vhost Nusa Panel mencatat log akses per-situs dalam format JSON satu baris (`panel_json`), mencakup bidang: timestamp (`t`/`time_local`), host (`h`), HTTP method (`m`), URI (`u`), status (`s`), body bytes (`b`), request time (`rt`), upstream response time (`urt`), cache status (`cs`), remote IP (`ip`), referer (`ref`), user agent (`ua`), protocol (`proto`), dan TLS version (`tls`).
2. **Kebutuhan Log Tailer**:
   - **Tahan Rotasi Log**: Sistem logrotate secara berkala melakukan `copytruncate` (truncate file kembali ke ukuran 0) atau `rename` (`access.log` di-rename menjadi `access.log.1`, dan file baru dibuat). Tailer harus mendeteksi kedua varian rotasi tanpa kehilangan data atau membaca ulang data lama.
   - **Pemulihan Crash & Persistensi Checkpoint**: Tailer harus menyimpan checkpoint atomik (`inode`, `file_offset`, `last_processed_ts`) di database SQLite `analytics.db`. Jika daemon panel mengalami restart atau crash, tailer melanjutkan pembacaan tepat dari offset terakhir tanpa membaca ulang jutaan baris.
   - **Deteksi Garis Parsial & Korup**: Baris log yang sedang ditulis Nginx secara parsial ditunda pembacaannya hingga newline (`\n`) lengkap diterima; baris non-JSON yang rusak/korup dilewati tanpa membuat proses tailer panik.
   - **Atribusi Situs Otomatis**: Mendeteksi `site_id` yang sesuai dari path file log (misal `/var/log/panel/{user}/{domain}/access.log` atau `/var/log/panel/sites/site-{id}/access.log`) dan fallback ke pemetaan Host header HTTP (termasuk wildcard subdomain `*.domain.com`).
   - **Backpressure & Throughput Tinggi**: Antrean buffer terikat (`tokio::sync::mpsc`) untuk mencegah pembengkakan memori saat downstream aggregator lambat, dengan throughput ingestion memenuhi target PRD Bagian 9 (≥ 5.000 baris/detik dengan lag < 5 detik).

## Keputusan Arsitektur

1. **Modul `NginxJsonLog` (`crates/panel-analytics/src/log_entry.rs`)**:
   - Struktur data deserialisasi zero-copy/fleksibel yang mendukung alias kunci singkat (`t`, `h`, `m`, `u`, `s`, `b`, `rt`, `urt`, `cs`, `ip`, `ref`, `ua`, `proto`, `tls`) maupun alias panjang Nginx standar (`time_local`, `remote_addr`, `request_method`, `request_uri`, dll).
   - Penanganan fleksibel untuk format Nginx upstream time majemuk (`upstream_response_time: "0.020, 0.045"`).
   - Konversi aman ke entri analitik terstruktur `LogEntryRecord` tanpa memicu panic atau alokasi berlebih.

2. **Tailer Berkas `LogFileTailer` (`crates/panel-analytics/src/tailer.rs`)**:
   - Mengelola state per-file: `file_path`, `site_id`, `current_inode`, `current_offset`, `batch_size`.
   - Menggunakan asynchronous `tokio::io::BufReader` dan `AsyncSeekExt::seek` untuk membaca secara efisien berbasis buffer blok.
   - **Logrotate Rename Handling**: Saat deteksi pergantian inode atau `file_len <= current_offset`, memeriksa berkas kandidat rotasi (`access.log.1`, `access.log.old`, dll) yang memiliki inode lama dan masih memuat sisa byte belum terbaca. Tailer menguras (*drain*) sisa byte tersebut hingga EOF file rotasi, lalu beralih mulus ke file log baru pada offset 0.
   - **Copytruncate Handling**: Jika ukuran berkas berkurang drastis di bawah `current_offset` dengan inode tetap sama, tailer mengenali bahwa file dipotong (`copytruncate`) dan mengatur ulang offset ke 0 secara aman.
   - **Line Delimiter Guarantee**: Menunda pemrosesan jika buffer berakhir sebelum karakter `\n`, memastikan baris parsial tidak pernah diproses setengah-setengah.

3. **Mesin Penemu & Pengawas `LogTailerEngine` (`crates/panel-analytics/src/engine.rs`)**:
   - Terintegrasi dengan filesystem watcher `notify::RecommendedWatcher` untuk mendeteksi event pembuatan berkas log dan penulisan berkas baru di direktori pengawasan (`/var/log/panel`, `/var/log/nginx`).
   - Mendukung penambahan direktori pantau secara dinamis (`add_watch_directory`) dan pemindaian rekursif log sites (`discover_logs`).
   - Sinkronisasi checkpoint berkala ke tabel SQLite `log_checkpoints` di `analytics.db`.
   - Menyediakan bounded channel `mpsc::Sender<LogBatch>` untuk transfer batch log ke pipeline agregator downstream dengan backpressure alami.
   - Menyediakan snapshot statistik telemetri mesin (`active_tailers`, `total_lines_processed`, `total_bytes_processed`, `total_batches_processed`).

4. **Resolver Atribusi Situs `SiteAttributor` (`crates/panel-analytics/src/attribution.rs`)**:
   - Resolusi berlapis dan thread-safe via `RwLock`:
     1. Pencocokan langsung dari pola direktori numerik (`site-{id}`, `sites/{id}`).
     2. Pencocokan nama folder induk terhadap daftar domain terdaftar (`/var/log/panel/{user}/{domain}/access.log`).
     3. Registry pencocokan exact Host header dan wildcard subdomain (`*.domain.com`).
     4. Registry manual path log terpetakan.

5. **Pengujian Komprehensif (`tests/log_tailer_test.rs`)**:
   - 11 integration test skenario pengujian: parsing Nginx JSON, multi-upstream parsing, resolusi atribusi path & host wildcard, toleransi copytruncate, penanganan logrotate rename draining, pemulihan checkpoint restart/crash, penolakan baris korup, backpressure bounded channel, verifikasi metrik stats engine, dan uji performa benchmark 5.000 entri.

## Konsekuensi

### Positif
- **Zero Data Loss**: Kombinasi checkpointing persisten dan mekanisme pengurasan sisa log rotasi (`try_drain_rotated_file`) menjamin integritas data metrik trafik Nginx bahkan saat logrotate berjalan di tengah-tengah aktivitas.
- **Kinerja Tinggi**: Uji benchmark membuktikan penyerapan 5.000 log line selesai dalam waktu ratusan milidetik (< 0,5 detik), melampaui standar PRD Bagian 9 (target < 5 detik).
- **Aman dari Crash**: Panel yang mati atau direstart segera melanjutkan pembacaan log dari titik offset tersimpan tanpa duplikasi request counter.
- **Kepatuhan Aturan**: Bebas `unwrap()` / `expect()` pada kode produksi pustaka, bebas warning clippy, dan format kode tervalidasi `cargo fmt`.

### Negatif / Mitigasi
- Inotify descriptor pada sistem Linux memiliki batas (`fs.inotify.max_user_watches`); dimitigasi dengan pengawasan di level folder direktori induk sites (`RecursiveMode::Recursive`) alih-alih memasang watch individual per berkas.
