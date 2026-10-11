# ADR 0062: Agregasi Metrik Trafik Web dan Rollup Multi-Resolusi (Web Traffic Metrics Aggregator & Rollups)

## Status
Diterima (Accepted) - Fase 9 (Task P9-T3)

## Konteks & Masalah
Sesuai PRD Bagian 5.6 ("Analytics mendalam") dan backlog fase 9 (`P9-T3`), Nusa Panel membutuhkan mesin agregasi metrik trafik web per-situs yang mengolah ribuan log request per detik secara efisien:
1. **Kebutuhan Metrik per Menit**:
   - Volume request total dan body bytes sent.
   - Distribusi kelas status HTTP (2xx, 3xx, 4xx, 5xx).
   - Efektivitas cache (HIT, MISS, BYPASS, OTHER).
   - Histogram latensi log-bucket dengan estimasi persentil akurat (p50, p95, p99) serta waktu respon upstream.
   - Estimasi pengunjung unik harian yang menjaga privasi pengunjung (tanpa menyimpan IP mentah).
   - Klasifikasi User-Agent: deteksi peramban (Browser), sistem operasi (OS), jenis perangkat (Device), serta bot web vs manusia (Bot vs Human).
   - Resolusi negara asal pengunjung via GeoIP (mendukung format `.mmdb` MaxMind DB-IP Lite / GeoLite2).
   - Top-K path endpoint dengan normalisasi template (mengelompokkan ID/UUID/hash dinamis).
   - Top-K referrer host.
   - Top-K slow URL (> 250 ms) dan top-K error URL (4xx / 5xx).
2. **Kebutuhan Rollup & Retensi**:
   - Agregasi berjenjang: 1 menit (`site_metrics_1m`) -> 1 jam (`site_metrics_1h`) -> 1 hari (`site_metrics_1d`).
   - Retensi data otomatis: 1m disimpan 14 hari, 1h disimpan 400 hari, 1d disimpan permanen.
   - Operasi hemat memori (bounded memory) yang aman terhadap lonjakan trafik tiba-tiba.

## Keputusan Arsitektur

1. **Normalisasi Path URL (`PathNormalizer`)**:
   - Menghapus query string (`?foo=bar`) dan fragment (`#section`).
   - Mengelompokkan ID numerik ke `:id` (misal `/posts/123` -> `/posts/:id`).
   - Mengelompokkan UUID standar ke `:uuid` (misal `/orders/550e8400-e29b-41d4-a716-446655440000` -> `/orders/:uuid`).
   - Mengelompokkan hex hash (MD5/SHA1/SHA256) ke `:hash`.
   - Mengurangi kardinalitas path secara drastis sehingga aman disimpan dalam agregasi statistik.

2. **Algoritma Space-Saving Stream (`SpaceSavingCounter`)**:
   - Menggunakan algoritma Space-Saving (Metwally et al.) untuk melacak heavy-hitters dalam memori terbatas (*bounded capacity*).
   - Menjamin akurasi elemen top-K (paths, referrers, countries, browsers, OS, devices) tanpa alokasi memori tak terbatas.

3. **Histogram Latensi Logaritmik (`LatencyHistogram`)**:
   - 12 bucket logaritmik: `[0, 5ms)`, `[5, 10ms)`, `[10, 25ms)`, `[25, 50ms)`, `[50, 100ms)`, `[100, 250ms)`, `[250, 500ms)`, `[500, 1000ms)`, `[1000, 2500ms)`, `[2500, 5000ms)`, `[5000, 10000ms)`, `[10000, +inf)`.
   - Menghitung p50, p95, p99 dengan interpolasi linier dalam rentang bucket.
   - Memungkinkan penggabungan (*merge*) antar bucket histogram secara instan pada rollup 1h dan 1d tanpa perlu menyimpan array sampel individual.

4. **Klasifikasi Peramban & Bot Bebas Regex (`UaParser`)**:
   - Deteksi berbasis pencocokan token cepat tanpa overhead mesin regex berat.
   - Mengenali puluhan bot umum (Googlebot, Bingbot, YandexBot, Baiduspider, Ahrefs, Semrush, curl, Wget, Python, Go, uptime monitors, scraper) dan menandai status `is_bot` serta nama bot.
   - Klasifikasi perangkat: Desktop, Mobile, Tablet, Bot.

5. **Resolusi Negara GeoIP (`GeoIpResolver`)**:
   - Mendukung pembacaan database biner `.mmdb` MaxMind (`maxminddb` crate, lisensi ISC/MIT yang kompatibel dengan `deny.toml`).
   - Deteksi otomatis IP lokal/loopback (`LOCAL`) dan subnet privat RFC1918 (`LAN`).
   - Menyediakan mekanisme override untuk pengujian unit dan lingkungan tanpa berkas `.mmdb`.

6. **Privasi Pengunjung Unik (`compute_visitor_hash`)**:
   - Menghitung hash 64-bit menggunakan SHA-256 dari gabungan `IP + User-Agent + Salt Harian`.
   - IP mentah pengunjung tidak disimpan di tabel analitik trafik publik, memenuhi prinsip privasi GDPR dan PRD Bagian 5.6.

7. **Penyimpanan SQLite & Pemroses Rollup (`AnalyticsStorage` & `SiteMetricsRollupProcessor`)**:
   - Tabel `site_metrics_1m`, `site_metrics_1h`, `site_metrics_1d` dengan primary key `(site_id, bucket_ts)`.
   - Fungsi `execute_site_rollups` mengagregasi 1m -> 1h dan 1h -> 1d secara otomatis dengan menghitung ulang p50/p95/p99 dari histogram gabungan serta menggabungkan top-K dictionary.
   - Fungsi `prune_expired_site_metrics` membersihkan rekaman kedaluwarsa secara berkala sesuai kebijakan `RetentionConfig`.

## Konsekuensi

### Positif
- **Kinerja dan Skalabilitas**: Agregasi 1 menit in-memory mereduksi ribuan log HTTP menjadi 1 baris basis data per situs per menit, mencegah kelebihan beban I/O database.
- **Matematika Rollup Akurat**: Histogram logaritmik memungkinkan estimasi p50, p95, dan p99 tetap presisi bahkan setelah diagregasi ke tingkat 1 jam dan 1 hari.
- **Kepatuhan Privasi**: Alamat IP publik pengunjung dianonimkan seketika melalui salted hashing harian.
- **Standar Kode Rust**: 100% bebas dari `unwrap()` / `expect()` pada kode produksi, lolos `cargo clippy -D warnings`, dan terformat rapi sesuai `cargo fmt`.

### Negatif / Mitigasi
- Penambahan dependensi `maxminddb` versi 0.24; dimitigasi karena pustaka ini sangat ringan, tidak memiliki dependensi C/unsafe berat, dan lisensi ISC diizinkan oleh `deny.toml`.
