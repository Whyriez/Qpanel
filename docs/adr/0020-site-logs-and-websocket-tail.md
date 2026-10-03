# ADR 0020: Log Virtual Host per Site, Logrotate, dan WebSocket Live Tail

- **Status**: Accepted
- **Tanggal**: 2026-10-03
- **Konteks**: P3-T7 Log: log per site di `/var/log/panel/<user>/<site>/`, logrotate, API tail + pencarian (dibatasi), live tail via WebSocket

## Konteks & Masalah

Setiap virtual host situs web pada Nusa Panel menghasilkan log akses (`access.log` berformat terstruktur `panel_json`) dan log error (`error.log`). Panel membutuhkan:
1. Penyimpanan terisolasi per tenant dan domain di `/var/log/panel/<user>/<site>/`.
2. Rotasi log otomatis via logrotate harian (14 hari retensi, kompresi, sinyal USR1 ke Nginx master process).
3. Pengambilan log melalui REST API dengan limit jumlah baris (`1..1000`) dan filter pencarian substring untuk mencegah konsumsi memori berlebih.
4. Streaming real-time melalui WebSocket (`/ws/logs/{site_id}`) dengan backlog awal, autentikasi sesi, dan validasi header `Origin` untuk mencegah Cross-Site WebSocket Hijacking (CSWSH).
5. Hak akses yang terisolasi ketat: tenant user hanya boleh melihat log situs miliknya, sedangkan admin dapat melihat semua log situs.

## Keputusan Arsitektur

1. **Akses File Log Melalui IPC Typed Agent**:
   - `panel-api` tidak pernah berjalan sebagai root dan tidak membaca path mentah di filesystem host.
   - Akses log dilakukan via IPC RPC `RequestPayload::SiteLogsRead` ke `panel-agent`.
   - Agent memvalidasi `username` (format tenant yang sah), `site_domain` (DomainName sah), dan `log_type` (`"access"` atau `"error"`). Jalur path traversal dilarang keras.
   - Pembacaan tail dibatasi maksimal 1000 baris, dengan dukungan pencarian `query` case-insensitive.

2. **Logrotate Virtual Host**:
   - Konfigurasi logrotate otomatis dihasilkan melalui `NginxBaselineConfig::render_logrotate_conf`.
   - Pola rotasi: `/var/log/panel/*/*/*.log` dengan parameter:
     - `daily`, `missingok`, `rotate 14`, `compress`, `delaycompress`, `notifempty`
     - `create 0640 www-data adm`
     - `postrotate`: `kill -USR1 $(cat /run/nginx.pid)` untuk reload file descriptor log Nginx tanpa downtime.

3. **Keamanan WebSocket (`/ws/logs/{site_id}`)**:
   - Autentikasi sesi: Menggunakan cookie sesi `panel_session` dari extractor `AuthenticatedUser`.
   - Cek Origin: Fungsi `validate_ws_origin` menolak permintaan dari domain lintas-situs (`Origin` tak dikenal) dengan `403 Forbidden`, namun mengizinkan loopback/localhost dan host panel resmi.
   - Tenant isolation: Menolak pengguna reguler yang mencoba mengakses WebSocket situs milik tenant lain dengan `403 Forbidden`.

4. **Polling & Streaming Loop**:
   - Pada saat koneksi terbentuk, backlog sebanyak N baris (default 50) dikirim sebagai frame JSON `{ "event": "backlog", ... }`.
   - Background task menjalankan ticker 500ms untuk mendeteksi penambahan baris baru, mengirimkan perubahannya sebagai frame `{ "event": "line", ... }`.
   - Saat koneksi ditutup oleh browser/klien, task streaming dihentikan secara bersih tanpa kebocoran resource.

## Konsekuensi

- **Positif**:
  - Pengguna dan admin dapat melakukan troubleshooting langsung dari antarmuka panel tanpa membutuhkan akses SSH.
  - Keamanan host terjaga melalui sanitasi input di IPC agent.
  - Logrotate memastikan disk server tidak penuh karena pertumbuhan file log yang tidak terkendali.
- **Negatif / Mitigasi**:
  - Polling 500ms di WebSocket: Diberi batas backlog per tick (200 baris) agar tidak membebani memori saat situs mengalami lonjakan traffic tinggi.
