# ADR 0049: Reverse Proxy Engine, SSRF Guard, and Upstream Health Probe

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.1 dan Spesifikasi Fase 7 (P7-T4 "Reverse proxy & guard SSRF"), Nusa Panel harus mendukung situs bertipe Reverse Proxy mandiri (`kind == "proxy"`) yang mengalirkan traffic HTTP/HTTPS publik dari Nginx ke backend server lokal atau eksternal:
1. **Dukungan Protokol Fleksibel**: Mendukung upstream `http://`, `https://`, dan Unix domain socket `unix:/path/to/socket.sock`.
2. **Opsi Reverse Proxy Terkurasi**:
   - `preserve_host`: Meneruskan header `Host $host` (atau host upstream asli bila nonaktif).
   - `websocket_support`: Konfigurasi otomatis direktif WebSocket (`proxy_http_version 1.1;`, `proxy_set_header Upgrade $http_upgrade;`, `proxy_set_header Connection $connection_upgrade;`).
   - Timeout terukur: `proxy_connect_timeout`, `proxy_send_timeout`, `proxy_read_timeout` (default 60s, format valid Nginx time).
   - Buffering: `proxy_buffering on/off`.
   - Custom Headers: Custom request headers (`proxy_set_header`) dan custom response headers (`add_header`).
   - SNI HTTPS: `proxy_ssl_server_name on;` otomatis aktif saat upstream menggunakan skema `https://`.
3. **SSRF Guard Ketat (Server-Side Request Forgery)**:
   - Akses proxy tanpa pembatasan berisiko tinggi membocorkan endpoint internal server, link-local metadata cloud, atau jaringan LAN privat.
   - Default: Hanya alamat loopback (`127.0.0.1`, `::1`, `localhost`, dan Unix domain socket) yang diizinkan.
   - Upstream eksternal (FQDN publik atau IP publik) memerlukan izin khusus pada paket/plan tenant (`allow_external_upstream`). Tanpa izin ini, permintaan ditolak dengan kode status 403 Forbidden.
   - **Hardcoded Blacklist (ALWAYS Blocked)**: Alamat metadata cloud (`169.254.0.0/16`, `fe80::/10`, AWS IPv6 `fd00:ec2::254`, `instance-data`, `metadata.google.internal`), port internal panel (`7700`, `8443`), port `0`, IP LAN privat (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `100.64.0.0/10`), serta karakter injeksi (`\r`, `\n`, `;`, `{`, `}`) diblokir mutlak.
4. **Pemeriksaan Kesehatan Upstream (Health Probe)**:
   - Panel menyediakan endpoint pengujian upstream `/api/v1/sites/{id}/proxy/probe` yang didelegasikan ke `panel-agent` untuk mengukur latensi (ms), kode status respons, dan keterjangkauan upstream tanpa harus menunggu trafik pengguna.

## Keputusan Arsitektur

### 1. Templating Vhost Nginx (`panel-webserver::template`)
- Menambahkan template `PROXY_VHOST_TEMPLATE` berbasis Jinja2/Minijinja.
- Struct `ReverseProxyVhostContext`:
  ```rust
  pub struct ReverseProxyVhostContext {
      pub site_id: String,
      pub primary_domain: String,
      pub alias_domains: Vec<String>,
      pub redirects: Vec<DomainRedirectConfig>,
      pub log_dir: String,
      pub snippets_dir: String,
      pub has_ssl: bool,
      pub ssl_cert: Option<String>,
      pub ssl_key: Option<String>,
      pub force_https: bool,
      pub ocsp_stapling: bool,
      pub upstream_url: String,
      pub preserve_host: bool,
      pub websocket_support: bool,
      pub proxy_connect_timeout: String,
      pub proxy_send_timeout: String,
      pub proxy_read_timeout: String,
      pub proxy_buffering: String,
      pub custom_headers: Vec<HeaderConfig>,
      pub custom_request_headers: Vec<HeaderConfig>,
      pub proxy_ssl_server_name: bool,
      pub allow_external_upstream: bool,
  }
  ```
- Validasi upstream URL (`validate_reverse_proxy_upstream_url`) memeriksa format skema, authority, port, dan menerapkan aturan SSRF guard & blacklist sebelum template di-render.
- Validasi header (`validate_header_config`) memastikan nama header mengikuti standar RFC 7230 token dan nilai header tidak mengandung CRLF injection serta bukan hop-by-hop header terlarang.

### 2. Protokol Agen & Upstream Probe (`panel-proto` & `panel-agent`)
- Pesan protokol:
  - `ProxyUpstreamProbeRequest { upstream_url: String, timeout_seconds: u32 }`
  - `ProxyUpstreamProbeResponse { healthy: bool, status_code: Option<u16>, latency_ms: u64, message: String }`
- `panel-agent::proxy::probe_upstream`:
  - Untuk `http` / `https`: Mengirim HTTP GET request dengan timeout (default 5 detik), mengukur durasi koneksi dan parsing HTTP response status.
  - Untuk `unix:` socket: Mencoba sambungan `tokio::net::UnixStream::connect`, mengukur waktu konektivitas dan mengirim pesan ketersediaan socket.

### 3. API & Guard Policy (`panel-api::proxy_site`)
- Endpoint:
  - `GET /api/v1/sites/{id}/proxy/settings`: Mengambil konfigurasi reverse proxy situs saat ini beserta status izin `allow_external_upstream` tenant.
  - `PUT /api/v1/sites/{id}/proxy/settings`: Memvalidasi upstream target, timeouts, dan custom headers, memperbarui `settings_json` di SQLite, serta menyusun ulang dan menerapkan vhost Nginx secara atomik melalui agen.
  - `GET /api/v1/sites/{id}/proxy/probe`: Menguji kesehatan target upstream (baik upstream tersimpan maupun query `?upstream_url=...`) dengan validasi SSRF ketat sebelum probe dikirim ke agen.
- Pengecekan izin tenant: Fungsi `check_user_allow_external_upstream` memeriksa flag paket tenant (`plans.allow_external_upstream`). Jika admin yang mengelola, flag selalu diizinkan (`true`).

### 4. Antarmuka Pengguna Frontend (`web/src/components/sites/ProxySettingsCard.tsx`)
- Tab "Reverse Proxy" khusus untuk situs bertipe `proxy`.
- Komponen konfigurasi interaktif:
  - Input Upstream Target URL dengan tombol uji "Uji Upstream (Probe)" live.
  - Status badge latensi dan respons probe upstream (Hijau untuk sehat / 200 OK, Merah untuk gangguan).
  - Banner peringatan SSRF guard informatif yang menjelaskan pembatasan loopback vs izin eksternal tenant.
  - Toggle preserve host ($host) dan dukungan WebSocket.
  - Pengaturan timeout (connect, send, read) dan proxy buffering.
  - Editor dinamis Custom Request Headers (`proxy_set_header`) dan Custom Response Headers (`add_header`).
- Integrasi ke `SiteWizardDialog`: Pilihan pembuatan situs jenis "Reverse Proxy (Nginx)" dengan konfigurasi awal target upstream dan opsi default aman.

## Konsekuensi & Keamanan
- **Mitigasi SSRF Komprehensif**: Penyerang atau tenant reguler tidak dapat memanfaatkan reverse proxy untuk mengeksploitasi cloud metadata link-local (AWS/GCP/Azure) atau port administrasi internal panel.
- **Isolasi Nginx**: Konfigurasi vhost diuji (`nginx -t`) dan di-reload secara atomik oleh agent; jika konfigurasi gagal validasi, rollback otomatis dilakukan tanpa mengganggu situs lain.
- **Performa & Fleksibilitas**: Dukungan Unix domain socket memungkinkan arsitektur zero-network overhead untuk aplikasi lokal, sementara dukungan HTTP/HTTPS dengan SNI memungkinkan reverse proxy ke microservices dan container eksternal.
