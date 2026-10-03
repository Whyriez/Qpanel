# ADR-0002: Arsitektur Two-Factor Authentication (2FA) & Recovery Codes

- **Status**: Diterima (Accepted)
- **Tanggal**: 2026-10-03
- **Konteks**: PRD Bagian 4.1 (Keamanan Akun & Auth), Task P1-T4

## Konteks dan Masalah

Panel hosting membutuhkan lapisan perlindungan akun dari pencurian kredensial (credential stuffing / brute force). Sesuai PRD bagian 4.1:
1. Two-Factor Authentication (2FA) menggunakan standar TOTP (RFC 6238).
2. Penyediaan recovery codes sekali pakai (single-use) bila pengguna kehilangan authenticator.
3. Mekanisme rescue darurat melalui CLI server (`panelctl admin reset-2fa <username>`) jika pengguna kehilangan akses ke aplikasi authenticator dan semua recovery codes.
4. Login flow yang aman tanpa membocorkan status sesi sebelum 2FA berhasil diverifikasi.

## Keputusan Arsitektur

### 1. Standar & Parameter TOTP
- Menggunakan crate `totp-rs` (v5.7, features: `gen_secret`, `qr`).
- **Algoritma**: HMAC-SHA1.
- **Panjang Kode**: 6 digit desimal.
- **Interval Waktu**: 30 detik.
- **Toleransi Skew**: 1 langkah (+/- 30 detik) untuk mengatasi perbedaan jam perangkat pengguna.
- **Issuer Default**: `Nusa Panel`.
- Format URL standar: `otpauth://totp/Nusa%20Panel:<username>?secret=...&issuer=Nusa%20Panel&algorithm=SHA1&digits=6&period=30`.
- QR Code disajikan sebagai data URI PNG berbasis base64 (`image/png;base64,...`) untuk kemudahan scanning di frontend tanpa request pihak ketiga.

### 2. Enrolment & Verifikasi
- Endpoint `POST /api/v1/auth/2fa/setup`:
  - Menghasilkan secret acak (base32), URI `otpauth`, gambar QR code base64, dan 8 recovery codes baru.
  - Akun **belum** diaktifkan 2FA-nya pada tahap ini untuk mencegah lockout jika pengguna belum mengonfigurasi authenticator.
- Endpoint `POST /api/v1/auth/2fa/enable`:
  - Pengguna mengirimkan token TOTP kandidat, secret, dan 8 recovery codes.
  - Backend memverifikasi bahwa kode TOTP valid sebelum menyimpan secret dan mengaktifkan `totp_enabled = true` di database.
  - Recovery codes disimpan dalam bentuk hash SHA-256 di tabel `user_recovery_codes`.

### 3. Recovery Codes (Kode Pemulihan)
- Jumlah: 8 kode per enrolment.
- Format: `XXXX-XXXX` (alfanumerik huruf besar, 8 karakter dipisah tanda hubung).
- Normalisasi: Tanda hubung dan spasi diabaikan, huruf dikonversi ke kapital sebelum hashing.
- Penyimpanan: Hanya SHA-256 hash yang disimpan di tabel `user_recovery_codes`. Nilai mentah hanya ditampilkan sekali saat setup.
- Karakteristik: Sekali pakai (`used_at` diisi timestamp UNIX saat digunakan). Kode yang telah digunakan tidak dapat dipakai kembali.

### 4. Challenge Saat Login
- Jika akun memiliki `totp_enabled = true`:
  - Request login awal tanpa `code` mengembalikan HTTP 200 dengan payload `{"requires_2fa": true, "user": null, "csrf_token": null}` tanpa menyertakan cookie sesi (`Set-Cookie`).
  - Request kedua menyertakan `code`:
    - Backend memeriksa apakah kode cocok dengan TOTP saat ini.
    - Jika tidak, backend memeriksa apakah kode cocok dengan recovery code yang belum pernah dipakai.
    - Jika salah satu valid: session cookie dibuat, lockout counter di-reset, dan data profil dikembalikan.
    - Jika keduanya salah: lockout counter bertambah, mengembalikan HTTP 401 (atau 423 jika lockout terpicu).

### 5. CLI Rescue
- Perintah: `panelctl admin reset-2fa --username <username>`
- Aksi: Menyetel `totp_enabled = false`, `totp_secret = NULL`, dan menghapus semua baris di tabel `user_recovery_codes` untuk user terkait.
- Akses: Hanya dapat dijalankan oleh sysadmin server melalui akses shell/terminal host atau VM.
