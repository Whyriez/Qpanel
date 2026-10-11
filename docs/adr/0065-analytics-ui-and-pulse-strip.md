# 65. Analytics UI, Interactive SVG Visualizations, and Dynamic PulseStrip

Date: 2026-10-11

## Status
Accepted

## Context
Pada Phase 9 (Observabilitas, Metrik, & Analitik), backend analitik (Timeseries, Rollup, Aggregations, Breakdowns 8 dimensi, PHP Slowlog, dan System Host metrics) telah selesai diimplementasikan pada task P9-T1 sampai P9-T5.

Task P9-T6 menuntut antarmuka pengguna (UI) analitik mendalam dengan prinsip:
1. Menampilkan overview ringkasan KPI, timeseries lalu lintas, latensi (p50/p95/p99), distribusi error (4xx/5xx), breakdown kontribusi 8 dimensi, status runtime (PHP-FPM & cgroup v2), dan stack trace explorer untuk PHP Slowlog.
2. Tidak menambahkan dependensi pustaka visualisasi grafik pihak ketiga yang berat (seperti Recharts atau Chart.js yang dapat memperbesar ukuran bundle JavaScript melebihi target anggaran 250 KB gzip di PRD Bagian 9).
3. Menggantikan placeholder statis `PulseStrip` dengan sparkline SVG dinamis yang reaktif, mengambil data 1 menit riil dari backend atau array data, memiliki penanda titik berdenyut (`animate-pulse`), dan terintegrasi di tabel situs, header detail situs, dan dashboard ringkasan.
4. Mendukung penanda rilis git deploy (**Deploy Marker**) yang diproyeksikan langsung pada garis linier waktu analitik lalu lintas.
5. Mematuhi design tokens CSS variables Vpanel, bahasa Indonesia sentence case yang jelas (i18n), dan standar aksesibilitas WCAG 2.2 AA.

## Decisions

1. **Lightweight Custom SVG Visualizations**
   - Mengimplementasikan seluruh grafik menggunakan komponen SVG murni (`<svg>`, `<path>`, `<line>`, `<rect>`, `<circle>`, `<text>`) tanpa runtime library eksternal.
   - Grafik yang dibuat meliputi:
     - `AnalyticsAreaChart`: Area chart SVG bergradien dengan sumbu X/Y dinamis, interactive crosshair hover, tooltip gabungan, dan **Deploy Marker** (garis putus-putus vertikal penanda rilis Git).
     - `AnalyticsLatencyChart`: Multi-line chart yang memplot 3 kurva latensi sekaligus (p50 median biru, p95 amber, p99 ekor tail merah) dengan shared tooltip koordinat.
     - `AnalyticsBarChart`: Stacked bar chart interaktif untuk error 4xx dan 5xx.
     - `ServerSystemMetrics`: Grafik observabilitas host server untuk admin (CPU avg/max, Load average, Jaringan RX/TX).

2. **Smart Dynamic `PulseStrip` Component**
   - Menerima prop `siteId` (number) atau `data` (number[]).
   - Jika `siteId` diberikan tanpa `data`, komponen otomatis melakukan fetch asinkron ke `/api/v1/sites/:id/analytics/timeseries?interval=1m` untuk mengambil 30 titik interval 1 menit terakhir.
   - Otomatis mendeteksi status error: jika terdapat status 4xx atau 5xx, warna sparkline beralih ke `var(--danger)`, sebaliknya `var(--primary)`.
   - Menggunakan entrance animation CSS 600ms dari kiri ke kanan dan titik ujung berdenyut (`animate-ping`).
   - Diintegrasikan di kolom tabel situs `SitesPage`, header detail situs `SiteDetailPage`, dan ringkasan resource `DashboardPage`.

3. **Multi-Dimension Breakdown & Slowlog Explorer**
   - `BreakdownTable`: Menampilkan tab navigasi untuk 8 dimensi (Paths, Referrers, GeoIP Countries, Browsers, OS, Devices, Slow URLs, Error URLs) dilengkapi input filter pencarian real-time dan horizontal progress bar kontribusi persentase.
   - `SlowlogExplorer`: Accordion interaktif yang menampilkan daftar eksekusi PHP lambat, durasi, pool, skrip, fungsi offending, dan collapsible call stack trace frames (frame #, function name, file path, baris, alamat memori).

4. **Hierarchical Page & Tab Integration**
   - Halaman khusus `AnalyticsPage.tsx`: Menyediakan scope selector "Per Situs" (dengan pemilih dropdown situs cepat) dan "Server Host (Admin)" (khusus admin). Dihubungkan ke menu navigasi sidebar Shell.
   - Tab khusus `SiteAnalyticsTab.tsx`: Terintegrasi sebagai tab analitik langsung di `SiteDetailPage.tsx`.
   - Menambahkan tombol ekspor CSV langsung ke endpoint `/api/v1/sites/:id/analytics/export`.
   - Filter preset waktu (1j, 6j, 24j, 7h, 30h, kustom datetime-local), pilihan interval (`auto`, `1m`, `1h`, `1d`), toggle pembanding periode lalu (`compare=true`) dengan delta badges, serta toggle auto-refresh (30s).

## Consequences
- Bundle frontend tetap sangat ramping (< 250 KB gzip) tanpa dependensi eksternal chart library.
- Seluruh grafik rendered tajam (vektor SVG) dan responsif menyesuaikan tema warna gelap/terang panel secara instan lewat CSS variables.
- Memenuhi 100% kriteria penerimaan P9-T6 dan lulus uji `tsc --noEmit` serta seluruh unit test vitest.
