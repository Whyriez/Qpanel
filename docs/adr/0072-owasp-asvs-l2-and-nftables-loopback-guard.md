# ADR 0072: Kepatuhan OWASP ASVS L2, Isolasi Loopback Kernel (nftables), dan Sandboxing Systemd

## Status
Diterima (Accepted)

## Konteks
Sesuai PRD Bab 6 (Arsitektur Keamanan) dan Bab 11 (Task P10-T4), Vpanel beroperasi dalam model hosting bersama multi-tenant di mana tenant yang berbeda berbagi satu host Linux. Model ini rentan terhadap dua vektor ancaman utama jika hanya mengandalkan isolasi filesystem tradisional:
1. **Penyadapan / Koneksi Lintas-Tenant pada Port Loopback**: Aplikasi tenant (Node.js, Python, Go) umumnya diikat ke `127.0.0.1` pada port `20000..=29999` yang diproksi oleh Nginx reverse proxy. Pada sistem Linux standar, setiap tenant dengan akses shell atau script PHP lokal dapat mengirim paket TCP ke `127.0.0.1:20050` milik tenant lain secara langsung, melewati reverse proxy dan autentikasi.
2. **Ketiadaan Batas Sandboxing Daemon**: Jika proses daemon dieksploitasi, ketiadaan restriksi kernel capabilities, filesystem protections, atau sandbox systemd dapat menyebabkan eskalasi hak akses sistem secara lateral.
3. **Kepatuhan Terhadap Standar Industri**: Dibutuhkan kerangka kerja audit formal (OWASP Application Security Verification Standard v4.0.3 Level 2) untuk membuktikan integritas arsitektur keamanan Nusa Panel secara transparan dan terukur.

## Keputusan Arsitektur

### 1. Isolasi Loopback Per-UID Menggunakan nftables
Kami menerapkan tabel nftables khusus kernel `inet panel_loopback_guard`:
- Filter pada hook `output` di antarmuka loopback `lo`.
- Paket TCP yang ditujukan ke rentang port tenant `20000..=29999` diperiksa menggunakan `meta skuid`:
  - Diizinkan (`accept`) jika `meta skuid == 0` (root daemon/system).
  - Diizinkan (`accept`) jika `meta skuid == 33` (`www-data` Nginx reverse proxy).
  - Diizinkan (`accept`) jika `meta skuid == <tenant_uid>` (pemilik sah dari port aplikasi).
  - Selain itu ditolak seketika (`reject with tcp reset`).
- Validasi sintaksis sebelum penerapan (`nft -c -f /tmp/...`) dan penulisan atomik via `panel-agent` root daemon.

### 2. Evaluasi dan Penegakan OWASP ASVS v4.0.3 Level 2
Kami mendefinisikan modul evaluasi dan laporan formal OWASP ASVS Level 2 di `panel-core::hardening` yang mencakup 7 domain utama:
- **V1 Architecture**: Pemisahan tegas dua daemon (`panel-api` non-root dan `panel-agent` root dengan typed IPC via Unix domain socket `0660`).
- **V2 Authentication**: Argon2id memory-hard password hashing dan TOTP RFC 6238 2FA.
- **V3 Session Management**: CSPRNG 256-bit token entropy dan cookie flags `HttpOnly; Secure; SameSite=Strict`.
- **V4 Access Control**: Centralized RBAC dan non-root typed command execution tanpa `Exec(String)`.
- **V5 Input Validation**: Validasi newtype regex, penolakan path traversal (`..`), zero string interpolation.
- **V8 Cryptography**: Authenticated encryption (ChaCha20-Poly1305 AEAD) dengan dual-key vault untuk rotasi secret zero-downtime.
- **V14 Configuration**: Header HTTP security ketat (`Content-Security-Policy`, `nosniff`, `DENY`, `HSTS`) dan isolasi port nftables.

### 3. Sandboxing Unit Systemd (`systemd-analyze security`)
Unit file template resmi untuk `panel-api.service` dan `panel-agent.service` dikonfigurasi dengan direktif sandboxing ketat:
- `NoNewPrivileges=true`
- `ProtectSystem=strict`
- `ProtectHome=true` (untuk api) / `read-only` (untuk agent)
- `PrivateTmp=true`
- `MemoryDenyWriteExecute=true`
- `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`
- `CapabilityBoundingSet` dibatasi seminimal mungkin.
- Skor paparan (*exposure score*) dievaluasi dan diverifikasi tetap **< 2.0 (OK)**.

### 4. Dual-Key Secret Vault & Graceful Rotation
Diimplementasikan `panel_core::secret_rotation::DualKeyRotationVault`:
- Enkripsi selalu menggunakan Kunci Primer aktif.
- Dekripsi mencoba Kunci Primer terlebih dahulu; jika gagal (misal data dienkripsi sebelum jadwal rotasi), otomatis mencoba Kunci Sekunder (historis).
- Menyediakan metode re-enkripsi online untuk migrasi rahasia tanpa gangguan layanan.

## Konsekuensi

### Positif
- **Zero Cross-Tenant Leakage**: Port loopback tenant sepenuhnya terisolasi pada level kernel Linux.
- **Laporan Kepatuhan Transparan**: Administrator dapat menginspeksi skor kepatuhan ASVS L2 dan sandboxing systemd langsung melalui REST API dan Web UI.
- **Keamanan Pertahanan Berlapis (*Defense in Depth*)**: Kompromi pada satu komponen (misal PHP script tenant) tidak dapat mengakses port runtime tenant lain maupun mengeksploitasi service systemd yang tersandbox.
- **Rotasi Rahasia Tanpa Downtime**: Pergantian master encryption key tidak menyebabkan kegagalan dekripsi variabel `.env` situs yang sudah ada.

### Negatif / Biaya Tambahan
- Memerlukan ketersediaan kernel Linux modern dengan dukungan nftables (`nft`). Host lama yang hanya mendukung iptables legacy memerlukan instalasi modul nftables.
- Sinkronisasi aturan nftables harus dijalankan setiap kali ada alokasi atau pelepasan port tenant baru.
