# ADR 0077: Public Bootstrap Installer Aman via GitHub Releases

## Status
Diterima

## Konteks
Pada ADR 0074, sistem telah menyediakan penginstal internal `install.sh` dan pipeline rilis multi-arsitektur yang memaketkan biner rilis ke dalam file tarball (`nusa-panel-<VERSION>-<TARGET>.tar.gz`) beserta dokumen integritas `SHA256SUMS`.
Namun, untuk memudahkan penyebaran di server VPS baru (*fresh VPS*), dibutuhkan metode instalasi satu baris (*one-liner bootstrap installer*) yang dapat dijalankan melalui pola umum:

```bash
curl -fsSL <PUBLIC_INSTALLER_URL> | sudo bash
```

Tantangan dan batasan teknis:
1. **Keamanan Eksekusi Piped**: Skrip yang dijalankan via `curl ... | bash` berjalan langsung di bawah hak akses root, sehingga data dari internet harus diperlakukan sebagai untrusted.
2. **Ketergantungan Minimal**: VPS baru belum tentu memiliki kakas seperti `jq`. Ekstraksi metadata dan versi harus dapat berjalan mengandalkan utilitas standar POSIX (`grep`, `sed`, `awk`, `curl`, `tar`, `sha256sum`).
3. **Deteksi Arsitektur Otomatis**: Host x86_64 dan ARM64 (aarch64) harus dipetakan secara deterministik ke target artifact yang sesuai tanpa intervensi manual.
4. **Validasi Kriptografis & Integritas**: Arsip release wajib diverifikasi terhadap `SHA256SUMS` sebelum diekstrak, dan struktur arsip wajib divalidasi terhadap potensi serangan *path traversal* (`../`) atau *absolute paths* (`/`).
5. **Argument Forwarding**: Opsi konfigurasi instalasi (seperti `--port`, `--unattended`, `--force-os`, `--skip-firewall`) harus dapat diteruskan secara aman ke `install.sh` internal.
6. **Desain Fleksibel untuk Masa Depan**: Menggunakan GitHub Releases (`Whyriez/Vpanel`) untuk saat ini tanpa hardcoding versi, sekaligus menyediakan modularitas fungsi agar mudah dialihkan ke domain CDN `get.nusapanel.com` / Cloudflare R2 di kemudian hari.

## Keputusan Arsitektur

### 1. Skrip Bootstrap Mandiri (`scripts/bootstrap-install.sh`)
- Dibuat skrip baru `scripts/bootstrap-install.sh` dengan opsi ketat `set -Eeuo pipefail`.
- Menjalankan 7 tahapan proses terisolasi:
  1. `[1/7] Checking environment`: Memastikan hak akses root (`id -u == 0`), OS Linux (`uname -s == Linux`), keberadaan dependensi minimal (`curl`, `tar`, `sha256sum`, `mktemp`, `uname`, `grep`, `sed`, `awk`), serta memetakan arsitektur CPU (`uname -m` -> `x86_64-unknown-linux-gnu` atau `aarch64-unknown-linux-gnu`).
  2. `[2/7] Resolving latest release`: Menanyakan release terbaru secara otomatis via GitHub API (`/repos/Whyriez/Vpanel/releases/latest`) dengan parsing `tag_name` via `grep` & `sed` (zero external dependency). Dilengkapi mekanisme fallback ke HTTP redirect `Location:` header bila API mengalami rate limit. Nilai tag divalidasi secara ketat dengan ekspresi reguler SemVer (`^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$`).
  3. `[3/7] Downloading release`: Mengunduh file tarball release dan `SHA256SUMS` ke direktori sementara terisolasi (`mktemp -d`) menggunakan `curl -fL --silent --show-error` tanpa pernah menggunakan flag tidak aman (`-k` / `--insecure`).
  4. `[4/7] Verifying SHA-256 checksum`: Menghitung hash SHA-256 berkas arsip dan mencocokkannya dengan baris yang bersesuaian di `SHA256SUMS`. Jika tidak cocok atau tidak ditemukan, proses langsung dihentikan (*fail-closed*).
  5. `[5/7] Validating release archive`: Menginspeksi daftar berkas dalam arsip (`tar -tzf`) menggunakan fungsi `validate_entry_path`. Jika ditemukan path absolut (diawali `/`) atau path traversal (`..`), proses langsung ditolak sebelum ekstraksi.
  6. `[6/7] Extracting release`: Mengekstrak paket ke direktori sementara dan memverifikasi keberadaan skrip `install.sh`.
  7. `[7/7] Starting Nusa Panel installer`: Berpindah direktori kerja (`cd`) ke dalam folder rilis hasil ekstraksi dan mengeksekusi `bash ./install.sh "$@"` dengan meneruskan seluruh argumen pengguna secara aman.

### 2. Penanganan Cleanup Otomatis
- Handler `cleanup()` dipasang pada signal `EXIT`, `INT`, `TERM` via `trap`.
- Direktori sementara `TMP_DIR` selalu dihapus bersih baik instalasi sukses, gagal, maupun dihentikan pengguna (Ctrl+C).

### 3. Argument Forwarding Menggunakan `"$@"`
- Meneruskan argumen baris perintah ke installer internal secara utuh tanpa penggabungan string (`eval` dilarang mutlak).
- Mendukung pemanggilan piped seperti:
  ```bash
  curl -fsSL <URL> | sudo bash -s -- --port 8443 --unattended
  ```

### 4. Modularitas untuk Pengujian Unit & Integrasi
- Skrip mendeteksi mode pemanggilan (`${BASH_SOURCE[0]}` vs `$0`). Bila di-*source*, skrip hanya mengekspor fungsi-fungsi pembantu tanpa mengeksekusi `main`.
- Menyediakan suite pengujian lengkap di `scripts/tests/bootstrap-install-test.sh` yang mencakup 10 kategori pengujian:
  - Pemetaan arsitektur CPU
  - Validasi release tag SemVer
  - Penamaan artifact deterministik
  - Verifikasi checksum SHA-256
  - Penolakan berkas korup / modifikasi
  - Penolakan arsip traversal dan path absolut
  - Validasi keberadaan `install.sh`
  - Integritas penerusan argumen
  - Resolusi rilis langsung terhadap GitHub Releases
  - Simulasi eksekusi end-to-end lengkap dengan mock HTTP server

## Konsekuensi
- **Positif**:
  - Instalasi Nusa Panel pada VPS baru dapat dilakukan dengan satu baris perintah tanpa perlu mengunduh repositori atau biner secara manual.
  - Keamanan terjamin dengan verifikasi checksum ganda dan proteksi anti-traversal sebelum berkas diekstrak.
  - Tidak menambah dependensi baru pada sistem (bekerja pada fresh Ubuntu/Debian minimal tanpa `jq`).
  - Membuka jalan transisi mulus ke domain produksi `get.nusapanel.com` di masa mendatang tanpa mengubah arsitektur installer internal.
- **Negatif / Batasan**:
  - Bergantung pada ketersediaan koneksi internet keluar (HTTPS port 443) ke GitHub pada saat instalasi.
