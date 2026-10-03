# ADR 0019: ACME dan Manajemen Sertifikat SSL/TLS

- **Status**: Accepted
- **Tanggal**: 2026-10-03
- **Konteks**: P3-T6 ACME: Let's Encrypt auto-SSL, HTTP-01 challenge, fallback self-signed, custom upload, dan renewal scheduler

## Konteks & Masalah

Nusa Panel membutuhkan penerbitan dan pembaruan sertifikat SSL/TLS otomatis untuk domain tenant. Persyaratan meliputi:
1. Protokol ACME (RFC 8555) dengan tantangan HTTP-01 untuk Let's Encrypt production dan staging.
2. Fallback otomatis ke sertifikat self-signed jika validasi ACME gagal atau belum tervalidasi DNS publik (agar HTTPS vhost tetap bisa distart tanpa crash konfigurasi Nginx).
3. Penyimpanan kunci privat dengan mode proteksi ketat `0600` milik root di direktori `/etc/panel/ssl/<domain>/`.
4. Upload sertifikat custom beserta kunci privat dengan validasi integritas format PEM dan kesesuaian domain X.509 (SAN / Common Name).
5. Background renewal scheduler untuk sertifikat yang akan kedaluwarsa dalam 30 hari ke depan, dengan backoff.
6. Dukungan konfigurasi OCSP stapling dan opsi Force HTTPS pada virtual host Nginx.

## Keputusan Arsitektur

1. **ACME Engine Menggunakan `instant-acme`**:
   - `instant-acme` dipilih karena ringan, berbasis `tokio`/`hyper`, murni Rust, tanpa dependensi besar C (OpenSSL runtime), dan mendukung HTTP-01 challenge serta enkripsi kunci ECDSA (P-256).
   - Akun ACME disimpan di tabel `acme_accounts` SQLite dengan kunci privat terenkripsi/terproteksi.
   - Tantangan HTTP-01 ditempatkan oleh agent melalui IPC `RequestPayload::AcmeChallengeDeploy` di `/var/www/acme-challenge/.well-known/acme-challenge/<token>` dan dibersihkan setelah order selesai (`AcmeChallengeCleanup`).

2. **Penyimpanan Sertifikat dan Hak Akses**:
   - Sertifikat dan kunci privat dikirim ke agent melalui IPC bertipe `RequestPayload::SslCertDeploy`.
   - Agent bertugas menulis file ke direktori `/etc/panel/ssl/<domain>/`:
     - `fullchain.pem`: mode `0644` (dibaca oleh Nginx master/worker).
     - `privkey.pem`: mode `0600` (hanya dibaca oleh root Nginx master process).
   - Dilarang keras membaca atau menulis path sembarang dari API; semua penulisan dilakukan secara deterministik oleh agent.

3. **Fallback Self-Signed Certificate**:
   - Saat pemesanan ACME gagal atau saat pengguna memilih opsi self-signed, sertifikat self-signed dibuat menggunakan `rcgen` (SAN domain utama dan alias).
   - Menghasilkan vhost yang langsung fungsional dengan HTTPS aktif tanpa error handshake Nginx.

4. **Sertifikat Custom**:
   - Pengguna dapat mengunggah `fullchain_pem` dan `privkey_pem`.
   - API melakukan validasi menggunakan parser X.509 (`x509-parser`) untuk memverifikasi validitas sertifikat, masa berlaku, dan pencocokan SAN domain sebelum di-deploy ke agent.

5. **Scheduler Pembaruan Otomatis**:
   - Fungsi `renew_expiring_certificates` menyeleksi sertifikat aktif dengan issuer `letsencrypt` yang masa berlakunya tersisa kurang dari 30 hari (`not_after <= now + 30 days`).
   - Sertifikat yang diperbarui otomatis dire-deploy dan vhost Nginx dikompilasi ulang secara atomik.

## Konsekuensi

- **Positif**:
  - Dukungan zero-touch HTTPS otomatis untuk situs statis dan tenant baru.
  - Vhost Nginx tidak pernah gagal start karena file sertifikat yang hilang.
  - Keamanan kunci privat terjamin dengan izin file POSIX `0600`.
- **Negatif / Mitigasi**:
  - Rate-limit Let's Encrypt: Ditangani dengan opsi staging URL (`https://acme-staging-v02.api.letsencrypt.org/directory`) saat pengujian, serta fallback self-signed yang mencegah pengulangan order tanpa henti.
