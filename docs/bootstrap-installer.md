# Bootstrap Installer Nusa Panel

Dokumen ini menjelaskan implementasi, arsitektur, cara penggunaan, dan verifikasi keamanan untuk bootstrap installer publik Nusa Panel (`scripts/bootstrap-install.sh`).

> [!NOTE]
> **Status:** Developer / Pre-production Bootstrap Installer.
> Saat ini bootstrap installer mengambil release langsung dari GitHub Releases (`Whyriez/Vpanel`). Domain publik `get.nusapanel.com` belum diaktifkan dan akan diimplementasikan pada tahap rilis publik berikutnya.

---

## 1. Tujuan Bootstrap Installer

Bootstrap installer (`scripts/bootstrap-install.sh`) adalah skrip pembuka (entrypoint) satu baris yang ringan untuk instalasi Nusa Panel pada VPS atau server baru:

```bash
curl -fsSL https://raw.githubusercontent.com/Whyriez/Vpanel/main/scripts/bootstrap-install.sh | sudo bash
```

Tugas utama bootstrap installer adalah:
1. Memastikan environment sistem memenuhi syarat (OS Linux, CPU x86_64 atau aarch64, privilege root).
2. Menentukan versi release stabil terbaru dari GitHub Releases secara otomatis (tanpa hardcode).
3. Mengunduh artifact release bundle dan checksum `SHA256SUMS`.
4. Memverifikasi integritas dan keaslian archive dengan SHA-256 sebelum file diekstrak.
5. Memvalidasi bahwa isi archive aman dari serangan path traversal atau absolute path.
6. Mengekstrak archive ke direktori sementara (`mktemp`).
7. Menjalankan skrip installer internal (`install.sh`) yang dibundel di dalam release.
8. Membersihkan seluruh berkas sementara setelah proses instalasi selesai atau bila terjadi interupsi/kegagalan.

Bootstrap installer **bukan pengganti installer utama**, melainkan jembatan pengunduh aman menuju `install.sh` internal yang mengonfigurasi layanan sistem (systemd, binary, UI dist, firewall, dsb).

---

## 2. Arsitektur Alur Kerja

```text
User / Terminal
  │
  │ curl -fsSL <URL> | sudo bash
  ▼
scripts/bootstrap-install.sh
  │
  ├── [1/7] Checking environment (OS Linux, Arch x86_64/aarch64, Root)
  ├── [2/7] Resolving latest release (GitHub API & Fallback Redirect)
  ├── [3/7] Downloading release (tar.gz + SHA256SUMS)
  ├── [4/7] Verifying SHA-256 checksum (Validasi hash mutlak)
  ├── [5/7] Validating release archive (Cegah path traversal / absolute path)
  ├── [6/7] Extracting release (ke temporary directory terisolasi)
  └── [7/7] Starting Nusa Panel installer (cd ke folder release & eksekusi)
         │
         ▼
    bundled install.sh "$@"
```

---

## 3. Arsitektur CPU yang Didukung

Bootstrap installer secara otomatis mendeteksi arsitektur mesin via `uname -m` dan memetakan ke target artifact:

| Arsitektur Host (`uname -m`) | Target Artifact Rust | Target Asset |
|---|---|---|
| `x86_64`, `amd64` | `x86_64-unknown-linux-gnu` | `nusa-panel-<VERSION>-x86_64-unknown-linux-gnu.tar.gz` |
| `aarch64`, `arm64` | `aarch64-unknown-linux-gnu` | `nusa-panel-<VERSION>-aarch64-unknown-linux-gnu.tar.gz` |

Jika dijalankan pada arsitektur yang tidak didukung (misalnya 32-bit `i686` atau `armv7`), installer akan langsung berhenti dengan pesan error yang jelas tanpa memodifikasi sistem.

---

## 4. Cara Penggunaan & Pengujian

### A. Penggunaan Standar (Interaktif)

```bash
curl -fsSL https://raw.githubusercontent.com/Whyriez/Vpanel/main/scripts/bootstrap-install.sh | sudo bash
```

### B. Meneruskan Argumen (Argument Forwarding)

Seluruh argumen baris perintah diteruskan secara aman ke installer internal `install.sh` menggunakan `"$@"` tanpa string concatenation:

```bash
# Instalasi non-interaktif dengan custom port 8443
curl -fsSL https://raw.githubusercontent.com/Whyriez/Vpanel/main/scripts/bootstrap-install.sh | sudo bash -s -- --port 8443 --unattended

# Melewati pengecekan OS ketat
curl -fsSL https://raw.githubusercontent.com/Whyriez/Vpanel/main/scripts/bootstrap-install.sh | sudo bash -s -- --force-os

# Melewati konfigurasi firewall
curl -fsSL https://raw.githubusercontent.com/Whyriez/Vpanel/main/scripts/bootstrap-install.sh | sudo bash -s -- --skip-firewall
```

### C. Eksekusi Lokal Langsung

Jika repositori sudah di-clone secara lokal:

```bash
sudo bash scripts/bootstrap-install.sh --unattended --port 17890
```

---

## 5. Keamanan & Verifikasi

Bootstrap installer dirancang dengan prinsip **fail-closed** dan zero-trust terhadap data dari jaringan:

1. **Strict Shell Options**: Menggunakan `set -Eeuo pipefail`.
2. **Strict Root Check**: Harus dijalankan sebagai root (`id -u == 0`). Script tidak melakukan `sudo` otomatis.
3. **Validasi Tag SemVer**: Hanya menerima tag berformat `vMAJOR.MINOR.PATCH` (misal `v0.1.0`, `v1.0.0`). Nilai arbitrary, path traversal, atau command injection ditolak keras.
4. **Validasi SHA-256 Mandatory**: Checksum release wajib cocok dengan entri yang terdaftar di `SHA256SUMS`. Jika hash berbeda, archive **tidak akan diekstrak dan tidak akan dieksekusi**.
5. **Anti-Path Traversal**: Setiap entri path dalam arsip tar divalidasi sebelum ekstraksi (`tar -tzf`). Entry yang mengandung `..`, `../`, atau absolute path (`/`) akan langsung membatalkan instalasi.
6. **No eval / No remote source**: Tidak ada eksekusi kode dinamis (`eval`, `sh -c "$REMOTE"`, atau `source <remote-file>`).
7. **TLS Verification Selalu Aktif**: Tidak menggunakan opsi `-k` atau `--insecure` pada `curl`.
8. **Isolated Workspace**: Unduhan dan ekstraksi dilakukan di direktori `mktemp` (`/tmp/nusapanel-install.XXXXXXXXXX`).
9. **Automated Cleanup**: Trap `EXIT`, `INT`, `TERM` memastikan berkas temporer selalu dihapus bersih bahkan saat proses dibatalkan pengguna (Ctrl+C) atau terjadi error.

---

## 6. Menjalankan Pengujian Otomatis

Suite pengujian komprehensif tersedia di `scripts/tests/bootstrap-install-test.sh`:

```bash
bash scripts/tests/bootstrap-install-test.sh
```

Cakupan pengujian:
- **Test 1**: Pemetaan arsitektur CPU (`x86_64`, `amd64`, `aarch64`, `arm64`, dan penolakan arsitektur tidak didukung).
- **Test 2**: Validasi release tag (tag valid vs injeksi karakter berbahaya).
- **Test 3**: Penamaan artifact deterministik.
- **Test 4**: Verifikasi SHA-256 valid.
- **Test 5**: Penolakan saat checksum mismatch / berkas rusak.
- **Test 6**: Penolakan entri tar berbahaya (path traversal `../` dan absolute path `/`).
- **Test 7**: Validasi keberadaan berkas `install.sh` di dalam paket rilis.
- **Test 8**: Preservasi argumen terpisah saat diteruskan ke installer internal.
- **Test 9**: Resolusi release langsung terhadap GitHub Releases `Whyriez/Vpanel`.
- **Test 10**: Simulasi end-to-end lengkap (download, validasi hash, ekstraksi, eksekusi installer, dan pembersihan berkas).

---

## 7. Troubleshooting Umum

### 1. `ERROR: This installer must be run as root.`
Installer membutuhkan hak akses administrator. Jalankan menggunakan `sudo bash` atau beralih ke root (`sudo -i`).

### 2. `ERROR: Unsupported CPU architecture: <arch>`
Host menggunakan arsitektur selain x86_64/amd64 atau aarch64/arm64. Nusa Panel hanya menyediakan build resmi untuk Linux x86_64 dan aarch64.

### 3. `ERROR: Failed to resolve latest Nusa Panel release from GitHub.`
Kemungkinan penyebab:
- Server tidak memiliki koneksi internet keluar (outbound HTTPS port 443 diblokir).
- Rate limit GitHub API untuk IP publik tersebut habis. Script secara otomatis mencoba fallback ke redirect endpoint GitHub, namun jika jaringan sepenuhnya memblokir domain `api.github.com` atau `github.com`, periksa koneksi atau tentukan versi eksplisit melalui:
  ```bash
  NUSA_PANEL_VERSION="v0.1.0" curl -fsSL <URL> | sudo bash
  ```

### 4. `ERROR: Release checksum verification failed.`
Archive yang terunduh mengalami korupsi data saat transmisi atau tidak sesuai dengan manifest rilis resmi. Installer otomatis menolak paket dan menghapus temporary file. Pastikan koneksi stabil lalu coba kembali.
