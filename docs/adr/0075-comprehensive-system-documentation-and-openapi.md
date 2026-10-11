# ADR 0075: Dokumentasi Sistem Terpadu, Panduan Operasional, dan Ekspor Otomatis OpenAPI 3.1

## Status
Diterima

## Konteks
Nusa Panel telah memasuki tahap penyelesaian Fase 10 (P10-T7: Dokumentasi). Untuk memastikan kontrol panel dapat diadopsi dengan mudah oleh administrator sistem, webmaster/tenant, serta pengembang yang ingin memperluas ekosistem panel, diperlukan seperangkat dokumentasi yang lengkap, akurat, dan dapat diverifikasi:
1. **Panduan Administrator (`docs/admin-guide.md`)**: Memandu instalasi host, bootstrap setup token, konfigurasi paket, manajemen runtime (PHP, Node.js, Python), manajemen database, alat keandalan `panelctl`, tuning optimizer performa, dan analitik keamanan.
2. **Panduan Tenant / Pengguna (`docs/tenant-guide.md`)**: Memandu pengelolaan situs web, konfigurasi domain, otomatisasi SSL/TLS Let's Encrypt, Web File Manager, akses SSH / Web Terminal xterm.js, manajemen database, cron jobs, site rules, dan observabilitas PulseStrip.
3. **Dokumen Arsitektur Lengkap (`docs/architecture.md`)**: Menjelaskan filosofi desain pemisahan hak istimewa (*privilege separation*), arsitektur non-root `panel-api`, root daemon `panel-agent`, protokol typed IPC UNIX domain socket, isolasi multi-tenant (Linux UID, POSIX ACL, cgroup v2, `hidepid=2`, nftables loopback filter), serta Nginx atomic configuration staging dan SQLite WAL.
4. **Dokumen Model Keamanan & Threat Modeling (`docs/security-model.md`)**: Memetakan ancaman keamanan (*threat vectors*) shared hosting, pembatasan lateral movement, mitigasi SSRF, hashing password Argon2id, TOTP 2FA, vault enkripsi ChaCha20-Poly1305 dual-key, dan pemenuhan standar OWASP ASVS Level 2.
5. **Referensi REST API & Ekspor OpenAPI 3.1 (`docs/api-reference.md` dan `docs/openapi.json`)**: Memetakan konvensi pemanggilan REST API, format RFC 9457 Problem Details, otentikasi Bearer JWT, CSRF protection, serta skema OpenAPI 3.1 yang dihasilkan secara terprogram (*auto-generated*) dari definisi `utoipa` backend Rust untuk mencegah desinkronisasi antara dokumentasi dan kode produksi.
6. **Panduan Kontribusi Pengembang (`docs/contributing.md`)**: Memandu pengembang baru mengenai filosofi proyek, aturan ketat `AGENTS.md`, penggunaan dev VM (`scripts/dev-vm.sh`), konvensi pengujian, conventional commits, dan pencatatan ADR.

## Keputusan Arsitektur

### 1. Ekspor Terprogram OpenAPI 3.1 (`docs/openapi.json`)
- Kami memanfaatkan integrasi `utoipa` yang telah terpasang di `crates/panel-api` (`panel_api::routes::ApiDoc`).
- Alih-alih menulis OpenAPI schema JSON secara manual yang rentan mengalami *drift* (ketidaksesuaian dengan endpoint aktual), skema OpenAPI diekspor secara otomatis melalui integration test Rust: `crates/panel-api/tests/openapi_export_test.rs`.
- Test ini memvalidasi keberadaan entitas kunci API dan secara atomik menulis representasi JSON yang diformat indah (*pretty-printed*) ke `docs/openapi.json`.
- Dengan pendekatan ini, setiap kali ada penambahan atau perubahan endpoint pada `ApiDoc`, pengujian otomatis akan memverifikasi dan memperbarui skema OpenAPI tanpa desinkronisasi dokumentasi.

### 2. Struktur Dokumentasi Pengguna & Administrator Berbahasa Indonesia Jelas
- Dokumentasi disusun dalam bahasa Indonesia baku teknis yang komunikatif (*sentence case*) dengan istilah teknis standar industri (misal: *runtime*, *virtual host*, *cgroup*, *socket*, *firewall*).
- Setiap panduan menyertakan contoh baris perintah (*command line snippets*), konfigurasi blok, dan diagram alur grafis (*Mermaid diagrams*) untuk mempermudah pemahaman arsitektur interkoneksi komponen.

### 3. Matriks Referensi Keamanan & Verifikasi ASVS L2
- Pada `docs/security-model.md`, mitigasi teknis dihubungkan langsung ke komponen sistem aktual (`panel-core::hardening`, `panel-agent::nftables`, `panel-api::auth`).
- Model ancaman menguraikan pertahanan berlapis (*defense-in-depth*) terhadap eskalasi hak akses lokal, pengintaian proses antar-tenant, penyalahgunaan loopback jaringan lokal, serta manipulasi *path traversal*.

## Konsekuensi

### Positif:
- **Dokumentasi Terverifikasi dan Terpusat**: Seluruh aspek operasional dan teknis sistem Nusa Panel kini terdokumentasi lengkap di folder `docs/`.
- **Zero Drift OpenAPI**: Spesifikasi REST API OpenAPI 3.1 dijamin 100% sinkron dengan implementasi backend Axum karena digenerate langsung dari metadata `utoipa`.
- **Onboarding Pengembang & Operator yang Mulus**: Administrator baru memiliki panduan langkah-demi-langkah dari instalasi hingga pemeliharaan darurat, sementara pengembang memiliki aturan kontribusi dan alur kerja pengujian yang jelas.

### Pertimbangan:
- Menjaga spesifikasi OpenAPI tetap sinkron memerlukan eksekusi test `openapi_export_test` pada CI pipeline ketika rute API dimodifikasi.
