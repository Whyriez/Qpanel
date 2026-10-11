# ADR 0067: Analitik Keamanan dan Pemblokiran Alamat IP dengan TTL

## Konteks
Situs web yang dihosting pada platform panel hosting rentan terhadap serangan pemindaian berkas rahasia (*probing paths*) seperti pencarian berkas `.env`, kebocoran repositori `.git`, eksploitasi WordPress (`xmlrpc.php`, `wp-login.php`), hingga serangan *credential stuffing* / *brute force* pada *endpoint* login. Pengelola situs memerlukan visibilitas langsung terhadap IP mencurigakan serta kemampuan untuk mengambil tindakan pertahanan instan tanpa harus mengetik konfigurasi firewall terminal secara manual.

PRD Bab 5.6, 6, dan 8.4 mensyaratkan:
1. Pelacakan klien IP teratas per situs dengan metrik error 4xx/5xx, asal negara (GeoIP), frekuensi probing, dan evaluasi tingkat risiko (`low`, `suspicious`, `high_risk`, `critical`).
2. Deteksi pemindaian jalur sensitif berulang dengan kategorisasi ancaman terstruktur.
3. Deteksi insiden brute force login (status 401/403/429 pada endpoint auth).
4. Mekanisme pemblokiran IP dengan durasi sementara (TTL 1 jam, 6 jam, 24 jam, 7 hari, atau permanen) yang terintegrasi langsung ke konfigurasi Nginx vhost (`deny <ip>;`) dan dapat dibuka blokirnya (*unban*) kapan saja.
5. Pembersihan data IP sensitif dengan masa retensi pendek untuk mematuhi privasi data.

## Keputusan Arsitektur

### 1. Deteksi Log Keamanan di `panel-analytics`
- Modul `crates/panel-analytics/src/security.rs` menyediakan fungsi klasifikasi deterministik:
  - `classify_probing_path(uri)`: mencocokkan pola signature jalur sensitif (`.env`, `.git/config`, `wp-login.php`, `phpmyadmin`, `backup.sql`, dsb.) dan memberikan kategori ancaman serta skor bobot ancaman.
  - `is_auth_endpoint(uri)`: mengenali endpoint autentikasi (`/login`, `/signin`, `/wp-login.php`, `/api/login`, dll).
  - `calculate_ip_risk_level(...)`: menghitung tingkat risiko IP (`Critical`, `HighRisk`, `Suspicious`, `Low`) berbasis kombinasi probing count, auth failure count, rasio error 4xx, dan volume permintaan.
- Tabel SQLite `site_security_events` diinisialisasi pada `AnalyticsStorage` dengan indeks terindeks cepat pada `(site_id, timestamp)`, `(site_id, client_ip)`, dan `(site_id, event_type)`.
- Method `query_security_overview` mengagregasi ringkasan keamanan, IP mencurigakan teratas, statistik jalur pemindaian, dan insiden brute force secara terpadu.

### 2. Manajemen Pemblokiran IP dengan TTL di `panel-db` & Sinkronisasi Atomik Nginx
- Dibuat migrasi `20261003000013_security_banned_ips.sql` dan repositori `SqliteSiteBannedIpRepository` di `panel-db`:
  - Kolom mencakup `id`, `site_id`, `ip_address`, `reason`, `banned_at`, `expires_at`, `banned_by`, dan `is_active`.
  - Dukungan TTL: `expires_at` bernilai `NULL` menandakan pemblokiran permanen, sedangkan nilai Unix epoch menentukan kedaluwarsa otomatis.
- Sinkronisasi Nginx:
  - Setiap pemblokiran IP menyuntikkan entri `IpAccessRule { action: IpAction::Deny, cidr_or_ip: ip }` ke dalam `SiteRulesConfig.ip_access` pada `settings_json` situs.
  - Nginx vhost situs direkompilasi dan diterapkan secara atomik melalui `recompile_and_apply_site_vhost` dengan validasi sintaks `nginx -t` dan rollback otomatis jika gagal.
  - Setiap query daftar banned IP memeriksa dan menonaktifkan record yang sudah kedaluwarsa (`deactivate_expired`) serta menghapus aturan deny dari Nginx secara atomik.

### 3. REST API Endpoints
- `GET /api/v1/sites/{id}/analytics/security/overview`: Mengembalikan agregat keamanan, top client IPs, probing paths, dan brute force incidents.
- `GET /api/v1/sites/{id}/security/banned-ips`: Mengembalikan daftar IP terblokir aktif beserta countdown sisa TTL.
- `POST /api/v1/sites/{id}/security/ban-ip`: Memvalidasi format IP/CIDR, mencatat banned record, dan menerapkan Nginx deny rule.
- `DELETE /api/v1/sites/{id}/security/ban-ip/{ip}`: Membuka blokir IP, menghapus rule dari Nginx, dan merekompilasi vhost secara atomik.

### 4. Antarmuka Reaktif Frontend
- Komponen `SecurityAnalyticsTab.tsx` diintegrasikan ke sub-tab situs `SiteAnalyticsTab.tsx`:
  - Kartu KPI ringkasan: Total Probing, Percobaan Brute Force, IP Mencurigakan Unik, IP Terblokir Aktif.
  - Tabel IP pengakses teratas dengan badge kode warna tingkat risiko dan tombol aksi satu klik "Blokir IP".
  - Modal dialog interaktif dengan chip pemilih TTL (1 Jam, 6 Jam, 24 Jam, 7 Hari, Permanen) dan input alasan.
  - Tabel rincian pemindaian jalur sensitif berkategori dan insiden brute force.
  - Tabel pengelola IP terblokir aktif dengan countdown TTL format jam & menit bersahabat serta tombol buka blokir.
  - Aksesibilitas WCAG 2.2 AA dan copy bahasa Indonesia sentence case.

## Konsekuensi
- **Positif**:
  - Penolakan penyerang terjadi langsung pada lapisan Nginx (kecepatan tinggi, tidak membebani worker runtime PHP/Node/Python).
  - Pemblokiran dengan TTL mencegah aturan blokir menumpuk selamanya (*rule bloat*).
  - Visibilitas mendalam tanpa overhead berat ke sistem produksi.
- **Batasan**:
  - Validasi CIDR membatasi subnet mask IPv4 maksimal /32 dan IPv6 maksimal /128 untuk mencegah pemblokiran yang tidak disengaja terhadap seluruh internet.
