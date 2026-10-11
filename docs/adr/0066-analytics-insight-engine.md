# 66. Rule-based Analytics Insight Engine, Anomaly Detection, and Automated Notification Center Integration

Date: 2026-10-11

## Status
Accepted

## Context
Pada Phase 9 (Observabilitas, Metrik, & Analitik), backend pengumpulan dan agregasi metrik lalu lintas (Nginx JSON logs, cgroup v2, PHP-FPM status, dan PHP slowlog) telah diselesaikan pada P9-T1 sampai P9-T6. Namun, metrik mentah dan grafik timeseries memerlukan pemantauan manual oleh tenant atau administrator.

Untuk mewujudkan panel hosting cerdas dan proaktif sesuai spesifikasi PRD Bagian 5.6, 8.1–8.5, dan task P9-T7:
1. Sistem harus memiliki **Insight Engine berbasis aturan (Rule-based Detection Engine)** yang menganalisis anomali operasional dan regresi kinerja secara deterministik tanpa dependensi AI pihak ketiga yang berat.
2. Aturan mencakup 8 pola kritis:
   - **Lonjakan 5xx (`five_xx_spike`)**: Galat server 5xx $\ge 3\%$ atau lonjakan $\ge 3\times$ dibanding periode sebelumnya (Severity: Critical, Aksi: "Periksa Log Error").
   - **Regresi latensi pasca deploy (`deploy_latency_regression`)**: Korelasi dengan git deployment marker terkini, di mana latensi respons p95 meningkat $\ge 40\%$ dengan selisih $\ge 80$ ms atau latensi p95 $\ge 1500$ ms (Severity: Critical, Aksi: "Rollback Deploy").
   - **Lonjakan bot (`bot_spike`)**: Lalu lintas bot $\ge 40\%$ atau lonjakan $\ge 3\times$ (Severity: Warning, Aksi: "Tinjau Aturan Akses").
   - **Lonjakan trafik (`traffic_spike`)**: Lonjakan total request $\ge 200\%$ ($3\times$) dibanding periode lalu (Severity: Info, Aksi: "Tinjau Grafik").
   - **Endpoint lambat (`slow_endpoint`)**: Terdeteksi URL dengan latensi rata-rata $\ge 1000$ ms dan count $\ge 10$ (Severity: Warning, Aksi: "Periksa Slowlog").
   - **Cache hit rendah (`low_cache_hit`)**: Rasio cache hit $< 20\%$ saat total request $\ge 100$ (Severity: Info, Aksi: "Tinjau Caching Nginx").
   - **Batas `pm.max_children` tercapai (`fpm_max_children_reached`)**: Terjadi `max_children_reached > 0` pada pool PHP-FPM (Severity: Warning, Aksi: "Tingkatkan Worker PHP").
   - **Kapasitas disk/kuota hampir penuh (`disk_quota_warning`)**: Penggunaan $\ge 85\%$ (Warning) atau $\ge 95\%$ (Critical) (Aksi: "Buka Pengelola Berkas").
3. **Integrasi Pusat Notifikasi**:
   - Menambahkan varian kategori baru `NotificationCategory::AnalyticsInsight` (`analytics_insight`) pada `panel-core`.
   - Setiap insight berkategori `critical` atau `warning` otomatis dikirimkan ke `NotificationService` untuk disimpan sebagai notifikasi in-app dan didispatch via email SMTP (bila dikonfigurasi).
4. **REST API & RBAC**:
   - `GET /api/v1/sites/:id/analytics/insights`: Mengambil insight hasil evaluasi untuk situs.
   - `POST /api/v1/sites/:id/analytics/insights/evaluate`: Memicu evaluasi eksplisit on-demand dan mendispatch notifikasi.
   - `GET /api/v1/system/analytics/insights`: Mengambil ringkasan insight seluruh situs (khusus Admin).
5. **Frontend UI**:
   - Komponen reaktif `SiteInsightsCard.tsx` dengan badge keparahan berwarna, rekomendasi aksi langsung satu klik (link ke log, rollback deploy, rules, settings), tombol dismiss lokal, dan indikator status normal.
   - Terintegrasi di tab analitik situs `SiteAnalyticsTab.tsx` dan halaman analitik host server `AnalyticsPage.tsx`.

## Decisions

1. **Deterministic Rule Engine Architecture di `panel-analytics`**
   - Dibuat modul `crates/panel-analytics/src/insights.rs` dengan `InsightEngine`, `InsightEvaluationContext`, `InsightType`, `InsightSeverity`, dan `InsightAction`.
   - Evaluasi bersifat murni (pure function) terhadap konteks yang dikumpulkan:
     - Metrik saat ini vs metrik periode sebelumnya (24 jam terakhir vs 24 jam sebelumnya).
     - Riwayat deployment Git (waktu rilis terakhir dan branch).
     - PHP-FPM rollups status metrics (`active`, `idle`, `max_children_reached`).
     - Metrik top slow URLs.
     - Penggunaan kuota disk dan batasan paket langganan.

2. **Integration Handler & Notification Dispatch di `panel-api`**
   - Fungsi `evaluate_site_insights_internal`:
     - Mengumpulkan rollups dari `AnalyticsStorage`.
     - Mengambil deploy history dari database situs.
     - Memeriksa kuota tenant dan batasan disk dari `SqliteUserRepository` dan `SqlitePlanRepository`.
     - Mengevaluasi context melalui `InsightEngine::evaluate(&ctx)`.
     - Mengirim notifikasi berkategori `analytics_insight` untuk setiap insight berstatus `critical` atau `warning` melalui `NotificationService::send_notification`.
   - Mendaftarkan endpoint ke rute Axum dan OpenAPI `ApiDoc`.

3. **Komponen UI `SiteInsightsCard` & System Health Overview**
   - `SiteInsightsCard.tsx`:
     - Menampilkan banner hijau "Kondisi Operasional Normal" jika tidak ada anomali.
     - Menampilkan kartu aksen merah/amber/biru lengkap dengan tombol aksi eksternal/internal (`target_url`), severity pill, penjelasan, dan dismiss.
     - Tombol "Evaluasi Ulang" interaktif yang memicu `POST /api/v1/sites/:id/analytics/insights/evaluate`.
   - `AnalyticsPage.tsx`:
     - Menambahkan ringkasan anomali lintas situs pada tab "Server Host (Admin)", mengelompokkan jumlah isu kritis dan peringatan dengan tombol langsung "Tinjau Situs".

## Consequences
- Tenant dan admin mendapatkan deteksi dini atas masalah performa tanpa harus memantau grafik analitik secara manual setiap saat.
- Tidak ada penambahan dependensi baru, tetap mematuhi zero-dependency visualisasi dan arsitektur modular Rust workspace.
- Teruji penuh melalui unit test di `panel-analytics`, integration tests di `panel-api`, serta test komponen React di `web`.
