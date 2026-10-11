# ADR 0064: Analytics Query API, Auto-Interval Resolution, Cost-Limit Guardrails, dan CSV Export

## Status
Diterima (Accepted) - Fase 9 (Task P9-T5)

## Konteks & Masalah
Sesuai PRD Bagian 5.6 ("Query API") dan backlog Fase 9 (`P9-T5`), Nusa Panel memerlukan REST API analytics yang komprehensif untuk menyajikan data lalu lintas web, metrik runtime aplikasi, slowlog, dan metrik sistem host:
1. **Rentang Waktu & Resolusi Interval Otomatis**:
   - Parameter `from`, `to`, `interval` (`auto`, `1m`, `1h`, `1d`).
   - Penentuan resolusi otomatis: $\le 6$ jam $\to$ 1m (`OneMinute`), $6$ jam s/d $30$ hari $\to$ 1h (`OneHour`), dan $> 30$ hari $\to$ 1d (`OneDay`).
2. **Batas Biaya Query (Query Cost Limits & Guardrails)**:
   - Mencegah query resource-heavy tak terkendali yang dapat membebani CPU dan I/O server hosting:
     - Interval 1m: maksimal rentang waktu 7 hari (`MAX_1M_RANGE_SECS`).
     - Interval 1h: maksimal rentang waktu 90 hari (`MAX_1H_RANGE_SECS`).
     - Interval 1d: maksimal rentang waktu 3 tahun (`MAX_1D_RANGE_SECS`).
     - Breakdown item limit: default 10, maksimal 50 (`MAX_BREAKDOWN_LIMIT`).
   - Validasi ketat: tolak rentang masa depan berlebih atau `from > to` dengan HTTP 400 Bad Request RFC 9457 Problem Details (`https://api.vpanel.dev/errors/invalid-time-range` dan `query-cost-exceeded`).
3. **Perbandingan Periode (Period Comparison)**:
   - Parameter `compare=true` atau endpoint `/compare` yang menghitung durasi yang sama di periode sebelumnya dan mengembalikan persentase delta perubahan (`requests_change_percent`, `bytes_change_percent`, `visitors_change_percent`, `avg_latency_change_percent`, `error_rate_change_percent`).
4. **Agregasi Top-N Breakdowns**:
   - Agregasi heavy-hitters untuk dimensi: paths, referrers, countries, browsers, operating systems, devices, slow URLs, dan error URLs dengan persentase kontribusi terhadap total trafik.
5. **Ekspor CSV Terstruktur**:
   - Endpoint `GET /api/v1/sites/{id}/analytics/export` menghasilkan output format CSV bertipe MIME `text/csv; charset=utf-8` dengan header `Content-Disposition: attachment; filename="site-{id}-analytics-{from}-{to}.csv"`.
6. **Metrik Aplikasi & Slowlog Terstruktur**:
   - Endpoint `GET /api/v1/sites/{id}/analytics/app` untuk metrik pool PHP-FPM dan proses cgroup v2 (Node.js/Python).
   - Endpoint `GET /api/v1/sites/{id}/analytics/slowlog` untuk event slow trace PHP-FPM terstruktur beserta call stack frames.
7. **Metrik Timeseries Sistem Host (Admin Guard)**:
   - Endpoint `GET /api/v1/system/analytics/timeseries` khusus untuk Administrator (CPU, RAM, Swap, Load, Disk IO, Network RX/TX).
8. **Isolasi Multi-Tenant**:
   - Memastikan tenant biasa hanya dapat mengakses analytics dari situs milik akunnya sendiri via `fetch_and_authorize_site`.

## Keputusan Arsitektur

1. **Modul `panel_api::analytics`**:
   - Struktur endpoint REST Axum yang modular dengan anotasi OpenAPI `utoipa` dan schema DTO lengkap.
   - Pemanfaatan `resolve_query_time_range` sebagai validator sentral query parameters dan penentu resolusi interval rollup.
   - Integrasi `AppState` dengan `analytics_storage: Arc<RwLock<Option<Arc<AnalyticsStorage>>>>` mendukung injeksi dependency secara eksplisit (`with_analytics`) maupun lazy initialization (`get_analytics`).

2. **Rekonstruksi Latensi Gabungan (Logarithmic Latency Interpolation)**:
   - Penggabungan data rollup periode tidak merata-ratakan p50/p95/p99 secara naif (yang secara matematis keliru).
   - Menggabungkan bucket histogram 12 rentang (`LatencyHistogram`) dari seluruh rollup point yang di-query, lalu menghitung ulang nilai p50, p95, dan p99 yang akurat secara matematis melalui `percentile_ms()`.

3. **Query Cost Limits & RFC 9457 Problem Details**:
   - Error response diformat mengikuti standar RFC 9457 `application/problem+json` dengan URI type, title berbahasa Indonesia yang jelas, status HTTP numerik, dan detail permasalahan.

4. **Isolasi Multi-Tenant & RBAC**:
   - Menggunakan helper `fetch_and_authorize_site` di setiap endpoint analytics per-site:
     - Admin diizinkan melihat semua situs.
     - Tenant non-admin dibatasi hanya pada situs miliknya (`site.user_id == auth_user.user.id`).
     - Akses tak sah ditolak dengan HTTP 403 Forbidden atau HTTP 404 Not Found.
   - Endpoint metrik sistem host `/api/v1/system/analytics/timeseries` dilindungi dengan `auth_user.is_admin()`.

5. **Ekspor CSV Streaming**:
   - Header HTTP `Content-Type: text/csv; charset=utf-8` dan `Content-Disposition: attachment; filename="..."` dibangun secara aman tanpa `unwrap()` / `expect()`.
   - Data baris CSV mencakup kolom: Timestamp, DateTime (ISO 8601 UTC), Requests, BytesSent, Status2xx, Status3xx, Status4xx, Status5xx, CacheHit, CacheMiss, LatencyP50Ms, LatencyP95Ms, LatencyP99Ms, LatencyAvgMs, Visitors.

## Konsekuensi & Kepatuhan
- **Zero Raw Shell & Path Traversal**: Seluruh operasi query dieksekusi melalui SQL parameter binding di SQLite storage.
- **Kepatuhan Aturan Proyek**: Tidak ada penggunaan `unwrap()` / `expect()` pada kode produksi `panel-api`.
- **Kualitas Kode**: 100% lolos verifikasi `cargo fmt --check` dan `cargo clippy -p panel-api -p panel-analytics -- -D warnings`.
- **Pengujian Lengkap**: 10 integration tests baru di `crates/panel-api/tests/analytics_api_test.rs` lolos 100%, membuktikan semua kriteria acceptance PRD 5.6 dan task P9-T5.
