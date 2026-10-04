# ADR-0028: Pemantauan Metrik Status FPM Real-Time dan Pembacaan Log PHP Terisolasi

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.1 dan Bagian 11 (Task `P4-T7`), administrator dan pemilik situs memerlukan visibilitas real-time terhadap kesehatan engine PHP-FPM dan pencatatan log aplikasi PHP:
1. **Query Metrik PHP-FPM Real-Time**:
   - Pemantauan metrik proses FPM: *active processes*, *idle processes*, *total processes*, *listen queue*, *max listen queue*, *listen queue len*, *accepted connections*, dan *slow requests*.
   - Data dikumpulkan dari endpoint internal status FastCGI (`/fpm-status?json`) melalui Unix Domain Socket (UDS) master tenant (`/run/panel/php/<user>-<ver>.sock`) tanpa membuka port ke publik.
2. **Pembacaan Error Log dan Slow Log Terisolasi**:
   - Master PHP-FPM per-tenant diarahkan untuk menulis log ke direktori terisolasi (`/var/log/panel/<tenant>/`):
     - Error log: `/var/log/panel/<tenant>/php-<version>-error.log`
     - Slow log: `/var/log/panel/<tenant>/<site>-slow.log`
   - Log viewer harus mendukung *tail* dengan batas baris tertentu (default 100, clamp 1..1000) dan filter pencarian *case-insensitive*.
3. **Integrasi REST API dan WebSocket Live Tail**:
   - Endpoint REST untuk pengambilan *snapshot* metrik dan log historis.
   - Integrasi WebSocket live tailing pada rute `/ws/logs/{id}?type=php_error|php_slow` dengan perlindungan CSWSH (*Cross-Site WebSocket Hijacking*) via validasi `Origin` dan autentikasi sesi.
4. **Isolasi RBAC Multi-Tenant**:
   - Tenant biasa hanya boleh mengakses status dan log situs miliknya sendiri. Akses ke situs milik tenant lain harus ditolak dengan HTTP 403 Forbidden.
   - Administrator memiliki izin untuk memantau status dan log seluruh situs di server.

## Keputusan Arsitektur

1. **Implementasi Protokol IPC Agent (`panel-proto` & `panel-agent`)**:
   - Menambahkan pesan IPC:
     - `PhpFpmPoolStatusRequest` / `PhpFpmPoolStatusResponse`: Mengirim query FastCGI status packet ke UDS socket pool tenant dan mem-parsing respons JSON.
     - `PhpLogsReadRequest` / `PhpLogsReadResponse`: Membaca baris log dengan pembatas ukuran buffer (maksimal 1 MiB), pemfilteran teks, dan *line limit*.
   - Mengimplementasikan `get_php_fpm_pool_status` di `crates/panel-agent/src/php.rs`:
     - Membuka koneksi stream ke socket UDS target.
     - Mengirim request FastCGI `FCGI_GET_VALUES` atau query HTTP FastCGI ke `/fpm-status?json`.
     - Mem-parsing struktur JSON metrik status proses FPM dengan fallback aman jika service sedang idle (0 worker).
   - Mengimplementasikan `read_php_logs` di `crates/panel-agent/src/php.rs`:
     - Membaca file log dari lokasi terisolasi tenant secara aman tanpa symlink traversal.
     - Memfilter baris berdasarkan query substring opsional.

2. **Endpoint REST API dan WebSocket di `panel-api` (`crates/panel-api/src/php.rs` & `logs.rs`)**:
   - `GET /api/v1/sites/{id}/php/status`:
     - Validasi keberadaan situs dan verifikasi `site.kind == "php"`.
     - Validasi hak akses tenant (RBAC).
     - Menghubungi agent IPC dan mengembalikan `SitePhpStatusResponse`.
   - `GET /api/v1/sites/{id}/php/logs?type=error|slow&lines=100&q=keyword`:
     - Mengembalikan potongan log PHP terfilter dalam struktur `SitePhpLogsResponse`.
   - `GET /api/v1/sites/{id}/logs?type=php_error|php_slow`:
     - Menghubungkan log PHP ke handler terpadu logs API.
   - `GET /ws/logs/{id}?type=php_error|php_slow`:
     - Mendukung live streaming log error dan slow log melalui WebSocket dengan verifikasi header `Origin` dan cookie sesi.

3. **Komponen Antarmuka Web (`web/`)**:
   - Membuat komponen `PhpStatusCard.tsx` yang menampilkan sparkline dan indikator proses FPM (Active, Idle, Total Workers, Listen Queue).
   - Menambahkan tombol pemilih tipe log di tab *Logs* `SiteDetailPage.tsx`:
     - `Nginx Access`
     - `Nginx Error`
     - `PHP Error`
     - `PHP Slow`
   - Terintegrasi dengan fitur Live Tail WebSocket dan filter pencarian instan.

## Konsekuensi

### Positif
- **Visibilitas Kinerja Real-Time**: Pengguna dapat langsung mengetahui apakah pool PHP-FPM kehabisan worker atau mengalami antrean (*listen queue saturation*).
- **Deteksi Bottleneck Cepat**: Slow log memungkinkan identifikasi script PHP yang berjalan lambat secara presisi per-situs.
- **Keamanan & Isolasi**: Log dibaca secara terisolasi per-tenant, mencegah kebocoran informasi (*information disclosure*) antar pengguna.
- **Koneksi Aman**: Socket status FastCGI tidak dipublikasikan ke jaringan internet, hanya dapat diakses secara lokal oleh panel daemon.

### Pertimbangan
- File log berukuran sangat besar dapat membebani memori jika dibaca sekaligus; oleh karena itu pembacaan dibatasi dengan *tail limit* maksimal 1000 baris dan batas baca 1 MiB per request.
