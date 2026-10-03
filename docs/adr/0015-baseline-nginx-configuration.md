# ADR 0015: Baseline Nginx Configuration and Host Webserver Installer

## Status
Accepted

## Context
Nusa Panel membutuhkan webserver Nginx yang ter-tuning, terisolasi, aman, dan siap menyajikan situs web tenant (static, PHP-FPM, reverse proxy) serta antarmuka web Nusa Panel itu sendiri. Konfigurasi default bawaan distribusi Linux sering kali memiliki pengaturan longgar, membocorkan informasi versi (`server_tokens on`), belum memiliki format log terstruktur, dan merespons sembarang Host header / SNI yang tidak dikenal.

Berdasarkan PRD Bagian 5.1 dan task `P3-T2`:
1. Diperlukan modul baseline installer yang menulis struktur konfigurasi Nginx terpusat di `/etc/panel/nginx/`.
2. Server default (catch-all) harus menolak host tak dikenal dengan status HTTP 444 (`return 444;`) dan menghentikan TLS handshake palsu (`ssl_reject_handshake on;`).
3. Vhost panel pada port `:8443` mem-proxy traffic antarmuka web dan WebSocket ke daemon internal `panel-api` di `http://127.0.0.1:3000`.
4. Snippet reusable harus disediakan untuk ACME HTTP-01 challenge, HTTP security headers, perlindungan file tersembunyi/sensitif (`.git`, `.env`, `.bak`, `.sql`), dan parameter FastCGI.
5. Log format harus terstruktur JSON (`panel_json`) guna memudahkan analitik, log parser, dan live tail WebSocket.
6. Seluruh instalasi file konfigurasi harus dilakukan secara atomik tanpa merusak konfigurasi aktif saat proses penulisan.
7. Operasi eksekusi host dilakukan oleh `panel-agent` dengan hak istimewa root melalui RPC bertipe.

## Decision
1. **Modul `panel-webserver`**:
   - Dibuat crate dedicated `crates/panel-webserver` yang memuat logika baseline Nginx (`NginxBaselineConfig`) dan engine templating webserver.
   - Menyediakan generator konfigurasi dan installer atomik (`install_baseline()`) yang menulis file konfigurasi melalui file temporary (`.tmp`) lalu di-rename secara atomik.

2. **Struktur Direktori Konfigurasi**:
   - `base_dir`: `/etc/panel/nginx/` (dapat dikonfigurasi pada saat pengujian).
   - `conf.d/`: Direktori konfigurasi server global (`00-default.conf`, `10-panel.conf`).
   - `snippets/`: Snippet modular (`acme.conf`, `security-headers.conf`, `deny-sensitive.conf`, `fastcgi-params.conf`).
   - `sites-available/` & `sites-enabled/`: Direktori vhost situs tenant.
   - `log_dir`: `/var/log/panel/`.
   - `acme_dir`: `/var/lib/panel/acme/`.

3. **Catch-All Default Server (HTTP 444)**:
   - File `conf.d/00-default.conf` mendengarkan `listen 80 default_server;` dan `listen 443 ssl default_server;`.
   - Mengaktifkan `ssl_reject_handshake on;` untuk memutus TLS handshake jika domain/SNI tidak cocok dengan sertifikat vhost yang ada.
   - Mengembalikan respon `return 444;` (koneksi Nginx ditutup tanpa mengirim header HTTP response), mencegah Host header injection dan pemindaian port IP terbuka.

4. **Structured JSON Logging (`panel_json`)**:
   - Diatur pada blok `http` di `nginx.conf`:
     ```nginx
     log_format panel_json escape=json '{'
         '"time_local":"$time_iso8601",'
         '"remote_addr":"$remote_addr",'
         '"request_method":"$request_method",'
         '"request_uri":"$request_uri",'
         '"server_protocol":"$server_protocol",'
         '"status":$status,'
         '"body_bytes_sent":$body_bytes_sent,'
         '"request_time":"$request_time",'
         '"upstream_response_time":"$upstream_response_time",'
         '"http_referrer":"$http_referer",'
         '"http_user_agent":"$http_user_agent"'
     '}';
     ```

5. **Snippet Keamanan & Proteksi File Sensitif**:
   - `snippets/deny-sensitive.conf`:
     - Menolak akses ke file tersembunyi (`location ~ /\.(?!well-known) { deny all; }`).
     - Menolak ekstensi sensitif, backup, dump database, dan script shell (`.bak`, `.conf`, `.sql`, `.sh`, `.env`, dll).
   - `snippets/security-headers.conf`:
     - Menambahkan header `X-Frame-Options "SAMEORIGIN"`, `X-Content-Type-Options "nosniff"`, `X-XSS-Protection "1; mode=block"`, dan `Referrer-Policy "strict-origin-when-cross-origin"`.
   - `snippets/acme.conf`:
     - Mengarahkan `location ^~ /.well-known/acme-challenge/` ke direktori tantangan terpusat `/var/lib/panel/acme`.

6. **Integrasi IPC dan Keamanan Host**:
   - Ditambahkan `NginxBaselineApplyRequest` dan `NginxBaselineAppliedResponse` pada `panel-proto`.
   - Menambahkan `"nginx"` ke whitelist `ALLOWED_BINARIES` di `crates/panel-agent/src/system.rs`.
   - `panel-agent` mengimplementasikan `NginxManager` untuk menjalankan instalasi baseline, verifikasi sintaks via `nginx -t -c <conf>`, dan reload via `systemctl reload nginx` tanpa string interpolation shell.

## Consequences
- Konfigurasi Nginx server di host menjadi terstandarisasi dan terlindungi dari serangan scanner umum.
- Log server dapat diparsing dengan mudah sebagai JSON oleh modul log viewer di Fase 3 (`P3-T7`).
- Host test environment (maupun VM) dapat memvalidasi instalasi secara deterministik tanpa memerlukan instalasi Nginx di mesin developer Windows berkat mocking dan trait `SystemRunner`.
