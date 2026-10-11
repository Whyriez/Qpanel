# ADR 0059: Site Overview & Summary Final Tab (P8-T8)

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.5 dan PRD Bagian 8 (Fase 8 P8-T8: Halaman Ringkasan final, line 832):
1. **Tujuan**:
   Menyediakan tab ringkasan situs final (`overview`) yang komprehensif, intuitif, dan real-time pada antarmuka detail situs Nusa Panel (`SiteDetailPage`).
2. **Kebutuhan Informasi & Metrik**:
   - **Status Live**: Status live webserver Nginx (`active` / `suspended`), status proses runtime (`running`, `inactive`, `failed`), uptime terformat, PID proses, dan health indicator berdenyut (*pulsating live dot*).
   - **Aksi Cepat (Quick Actions)**: Kontrol 1-klik untuk *Start*, *Stop*, *Restart*, *Reload Nginx*, *Clear Cache*, *Buka Berkas (File Manager)*, *Buka Terminal (SSH)*, *Cadangkan Sekarang (Backup)*, dan *Deploy Cepat (Git)* dengan umpan balik visual instan.
   - **Resource Tenant (cgroup slice)**: Penggunaan Memori RAM terpakai vs batas kuota paket (`memory_bytes` / `memory_limit_bytes`), penghitung proses aktif (`pids_current` vs `pids_limit`), dan persentase utilisasi CPU.
   - **Sertifikat SSL**: Penerbit sertifikat (*issuer*), masa berlaku (*valid_until*), hitung mundur sisa hari (*countdown*), status auto-renew ACME, dan tombol pintas pembaruan.
   - **Versi Runtime**: Jenis runtime (`php`, `nodejs`, `python`, `static`, `proxy`), versi rilis spesifik, kerangka kerja/preset (`laravel`, `nextjs`, `django`, `wordpress`), dan socket Unix atau port loopback target.
   - **Rincian Penyimpanan (Storage Breakdown)**: Visualisasi bilah tersegmentasi yang memisahkan ukuran berkas situs (*docroot*), basis data (*databases*), dan arsip cadangan (*backups*) terhadap total kuota paket disk tenant.
   - **Peringatan Kontekstual (Contextual Warnings)**: Banner peringatan dinamis otomatis jika SSL hampir kedaluwarsa (< 14 hari), force HTTPS aktif tanpa sertifikat valid, kuota disk tenant > 90%, deployment terakhir gagal, atau situs dinonaktifkan.
   - **Deployment Terakhir (Latest Deployment)**: Detail commit Git, branch, pembuat, pemicu rilis, durasi, status hasil, dan indikator kesiapan rollback otomatis.
   - **Cadangan Terakhir (Latest Backup)**: Nama arsip, ukuran berkas, cakupan data (berkas/database), dan tanggal pembuatan.
3. **Batasan Arsitektur (Rules & Definition of Done)**:
   - `panel-api` tidak berjalan sebagai root. Aksi cepat runtime diteruskan via IPC ke agent bertipe aman (`AgentClient`).
   - Tidak ada `unwrap()` / `expect()` pada kode non-test Rust.
   - Seluruh endpoint API tervalidasi dengan integration test.
   - Antarmuka web mematuhi design tokens, accessible (WCAG 2.2 AA), dan lulus `tsc`, ESLint, serta Vitest tests.

---

## Keputusan Arsitektur

### 1. API Endpoint & DTO Terpadu (`crates/panel-api/src/sites.rs`)
Disediakan dua endpoint utama:
1. `GET /api/v1/sites/{id}/overview`:
   Mengumpulkan data agregat dari repositori `site_repo`, `domain_repo`, `certificate_repo`, `git_deploy_repo`, `backup_repo`, `database_repo`, `plan_repo`, serta status real-time dari `agent_client` menjadi satu respon terpadu `SiteOverviewResponse`:
   - `status: SiteLiveStatus`: Webserver active/suspended, runtime status, uptime, PID, dan health status.
   - `resources: SiteResourceMetrics`: CPU %, RAM used & limit, PIDs used & limit.
   - `ssl: SiteSslSummary`: Keberadaan cert, issuer, tanggal kedaluwarsa, hitung mundur hari, status auto-renew.
   - `runtime: SiteRuntimeSummary`: Kind, version, framework preset, loopback target/socket.
   - `storage: SiteStorageBreakdown`: Docroot bytes, database bytes, backup archive bytes, total bytes, plan limit, persentase.
   - `warnings: Vec<SiteContextualWarning>`: Deteksi otomatis anomali situs dan kuota.
   - `latest_deployment: Option<LatestDeploymentSummary>`: Detail commit, branch, release ID, can_rollback.
   - `latest_backup: Option<LatestBackupSummary>`: Filename, size, date, status.
   - `quick_actions_available: Vec<String>`: Daftar aksi cepat yang valid untuk situs tersebut.
2. `POST /api/v1/sites/{id}/action`:
   Mengeksekusi aksi cepat:
   - `start`: Mengaktifkan vhost Nginx dan memulai layanan runtime.
   - `stop`: Menghentikan layanan runtime atau menonaktifkan situs.
   - `restart`: Memuat ulang service runtime (Node.js/Python/PHP-FPM).
   - `reload`: Melakukan reload konfigurasi Nginx via agent.
   - `clear_cache`: Membersihkan cache halaman Nginx FastCGI.

### 2. Frontend React Component (`web/src/components/sites/SiteOverviewTab.tsx`)
- Komponen antarmuka kaya yang memadukan:
  - Banner peringatan kontekstual berwarna merah (*danger*) dan amber (*warning*) dengan navigasi 1-klik ke tab terkait.
  - Header status dengan *live pulsing dot* hijau/merah dan status badge Vpanel.
  - Action Bar cepat 9-tombol responsif dengan status disabled pintar berdasarkan kondisi saat ini.
  - Kartu Metrik Sumber Daya (RAM bar, Tasks bar, CPU meter).
  - Kartu SSL Expiry dengan hitung mundur hari dan deep-link ke tab SSL.
  - Kartu Runtime Engine dengan detail framework preset dan loopback path.
  - Bilah Tersegmentasi Penyimpanan Disk (Docroot + Database + Backup vs Quota).
  - Kartu Rilis Deployment Terakhir dengan commit badge dan indikator rollback.
  - Kartu Cadangan Terakhir dengan rincian ukuran arsip.

### 3. Integrasi Halaman Detail (`web/src/pages/SiteDetailPage.tsx`)
- Tab `overview` kini memuat `<SiteOverviewTab />`, menggantikan kartu informasi statis sebelumnya.
- Pembersihan kode yang tidak digunakan (`copiedDocroot`, `handleCopyDocroot`).
- Header situs tetap menyediakan aksi cepat SSH/SFTP dan pintasan ganti versi PHP.

---

## Konsekuensi

### Positif:
- Pengguna mendapatkan visibilitas 360 derajat atas kesehatan, performa, dan sumber daya situs dalam satu layar utama.
- Tindakan pemecahan masalah (seperti restart proses atau pembaruan sertifikat) dapat dilakukan langsung dalam hitungan detik tanpa berpindah tab.
- Penanganan peringatan dini (SSL expired, kuota 90%) mencegah kegagalan downtime tak terduga.

### Pengujian & Verifikasi:
- Test integrasi backend: `crates/panel-api/tests/site_overview_api_test.rs`.
- Test komponen frontend: `web/src/components/sites/__tests__/SiteOverviewTab.test.tsx`.
- Lulus pemeriksaan tipe TypeScript `tsc --noEmit` dan ESLint `npm run lint`.
