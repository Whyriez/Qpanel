# BOOTSTRAP_INSTALLER.md

## Task: Bootstrap Installer Publik Nusa Panel

Implementasikan bootstrap installer publik untuk Nusa Panel.

Target akhir dari task ini adalah agar installer dapat dijalankan menggunakan pola:

```bash
curl -fsSL <PUBLIC_INSTALLER_URL> | sudo bash
```

Untuk tahap ini, **jangan implementasikan domain `get.nusapanel.com` terlebih dahulu**. Fokus hanya membuat bootstrap installer yang bekerja dengan GitHub Releases yang sudah tersedia.

Repository saat ini sudah memiliki release pipeline dan GitHub Release publik dengan pola artifact:

```text
nusa-panel-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
nusa-panel-v0.1.0-aarch64-unknown-linux-gnu.tar.gz
SHA256SUMS
```

GitHub repository:

```text
Whyriez/Vpanel
```

Contoh release yang sudah tersedia:

```text
v0.1.0
```

Bootstrap installer **tidak boleh meng-hardcode versi `v0.1.0`**. Installer harus selalu mencari release terbaru secara dinamis.

---

# 1. Tujuan

Bootstrap installer bertugas:

1. Memastikan script berjalan dengan privilege root.
2. Memastikan host menggunakan Linux yang didukung.
3. Mendeteksi arsitektur CPU.
4. Mengambil metadata GitHub Release terbaru.
5. Menentukan artifact yang cocok dengan arsitektur host.
6. Mengunduh artifact release.
7. Mengunduh `SHA256SUMS`.
8. Memverifikasi checksum artifact.
9. Mengekstrak release secara aman ke temporary directory.
10. Menemukan `install.sh` yang berada di dalam release.
11. Menjalankan installer internal tersebut.
12. Membersihkan temporary files setelah selesai maupun saat gagal.
13. Menghasilkan error message yang jelas jika salah satu tahap gagal.

Bootstrap installer **bukan installer sistem utama**. Installer utama tetap `install.sh` yang sudah berada di dalam release bundle.

Arsitektur:

```text
User
 │
 │ curl bootstrap installer
 ▼
Bootstrap Installer
 │
 ├── Detect OS
 ├── Detect Architecture
 ├── Resolve latest GitHub Release
 ├── Download release artifact
 ├── Download SHA256SUMS
 ├── Verify SHA256
 └── Extract
        │
        ▼
   bundled install.sh
        │
        ▼
   Existing Nusa Panel Installer
```

---

# 2. Scope

Task ini hanya mencakup bootstrap installer.

Yang termasuk:

```text
✅ bootstrap installer
✅ latest release detection
✅ x86_64 detection
✅ aarch64/arm64 detection
✅ artifact download
✅ checksum verification
✅ safe extraction
✅ cleanup
✅ error handling
✅ compatibility dengan piping melalui curl
✅ testing script
✅ dokumentasi penggunaan developer
```

Yang **tidak** termasuk:

```text
❌ Cloudflare Worker
❌ get.nusapanel.com
❌ Cloudflare R2
❌ panelctl update
❌ stable/beta/nightly channel
❌ Ed25519 release signing
❌ auto updater background service
❌ UI update notification
❌ mengubah release workflow kecuali benar-benar diperlukan untuk kompatibilitas
```

Jangan memperluas task di luar scope ini.

---

# 3. Lokasi File

Buat file bootstrap baru:

```text
scripts/bootstrap-install.sh
```

Jangan mengganti fungsi utama dari:

```text
install.sh
```

`install.sh` adalah installer internal release.

Bootstrap hanya bertanggung jawab mengambil release yang benar lalu menjalankan `install.sh`.

Jika diperlukan, tambahkan test di:

```text
scripts/tests/bootstrap-install-test.sh
```

atau struktur testing shell lain yang konsisten dengan repository.

---

# 4. Repository Release

Gunakan repository:

```text
OWNER=Whyriez
REPO=Vpanel
```

Latest release metadata dapat diperoleh dari:

```text
https://api.github.com/repos/Whyriez/Vpanel/releases/latest
```

Release asset download mengikuti pola GitHub Releases:

```text
https://github.com/Whyriez/Vpanel/releases/download/<VERSION>/<ASSET_NAME>
```

Contoh:

```text
https://github.com/Whyriez/Vpanel/releases/download/v0.1.0/nusa-panel-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
```

Jangan meng-hardcode versi.

---

# 5. Resolusi Versi

Bootstrap harus menentukan release terbaru.

Preferred flow:

```text
GET /repos/Whyriez/Vpanel/releases/latest
       │
       ▼
read tag_name
       │
       ▼
v0.1.0
```

Untuk parsing JSON, usahakan **tidak mewajibkan `jq`** karena fresh VPS mungkin belum memilikinya.

Pilihan yang diperbolehkan:

1. Parsing sederhana menggunakan `sed`/`grep` selama robust untuk field `tag_name`.
2. Menggunakan endpoint redirect GitHub yang tidak membutuhkan parsing JSON jika implementasinya lebih aman.
3. Menggunakan Python hanya jika Python memang dijamin tersedia pada semua OS target. Jangan menambah dependency hanya untuk parsing versi.

Installer harus gagal dengan jelas jika latest release tidak bisa ditemukan.

Contoh error:

```text
ERROR: Failed to resolve latest Nusa Panel release from GitHub.
```

---

# 6. Validasi Tag

Hanya terima tag release dengan format aman.

Minimal:

```text
vMAJOR.MINOR.PATCH
```

Contoh valid:

```text
v0.1.0
v1.0.0
v12.4.7
```

Jika nantinya repository menghasilkan prerelease, parser boleh mendukung SemVer prerelease:

```text
v1.0.0-beta.1
```

Tetapi jangan menerima nilai arbitrary yang dapat menyebabkan shell injection atau path traversal.

Version string dari remote **tidak boleh langsung digunakan tanpa validasi**.

---

# 7. Deteksi Arsitektur

Gunakan:

```bash
uname -m
```

Mapping:

```text
x86_64
  → x86_64-unknown-linux-gnu

amd64
  → x86_64-unknown-linux-gnu

aarch64
  → aarch64-unknown-linux-gnu

arm64
  → aarch64-unknown-linux-gnu
```

Jika arsitektur lain:

```text
ERROR: Unsupported CPU architecture: <arch>
Supported architectures:
- x86_64 / amd64
- aarch64 / arm64
```

Jangan mencoba fallback artifact lain.

---

# 8. Deteksi Sistem Operasi

Bootstrap hanya untuk Linux.

Minimal lakukan:

```bash
uname -s
```

dan pastikan:

```text
Linux
```

Jika bukan Linux:

```text
ERROR: Nusa Panel can only be installed on Linux.
```

Bootstrap boleh membaca:

```text
/etc/os-release
```

untuk menampilkan informasi OS.

Namun validasi detail Ubuntu/Debian tetap boleh diserahkan ke existing `install.sh`, kecuali repository saat ini mengharuskan bootstrap melakukan pengecekan awal.

OS target yang didokumentasikan Nusa Panel:

```text
Ubuntu 24.04 LTS
Ubuntu 22.04 LTS
Debian 12
```

---

# 9. Root / sudo Handling

Karena target penggunaan adalah:

```bash
curl -fsSL ... | sudo bash
```

bootstrap harus memastikan:

```bash
id -u
```

menghasilkan:

```text
0
```

Jika bukan root:

```text
ERROR: This installer must be run as root.

Use:
curl -fsSL <URL> | sudo bash
```

Jangan mencoba menjalankan `sudo` secara otomatis dari dalam script.

---

# 10. Dependencies

Gunakan dependency seminimal mungkin.

Bootstrap boleh mengandalkan tool umum:

```text
bash
curl
tar
sha256sum
mktemp
uname
grep
sed
awk
```

Jika `curl` tidak tersedia, fail dengan pesan jelas.

Contoh helper:

```bash
require_command curl
require_command tar
require_command sha256sum
```

Jangan diam-diam melakukan `apt install` dependencies di bootstrap.

Dependency installation tetap tanggung jawab installer utama jika dibutuhkan.

---

# 11. Asset Naming

Artifact release mengikuti format:

```text
nusa-panel-${VERSION}-${TARGET}.tar.gz
```

Contoh:

```text
VERSION=v0.1.0
TARGET=x86_64-unknown-linux-gnu

ASSET:
nusa-panel-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
```

Untuk ARM:

```text
nusa-panel-v0.1.0-aarch64-unknown-linux-gnu.tar.gz
```

Checksum file:

```text
SHA256SUMS
```

Bootstrap harus menggunakan nama asset yang deterministik berdasarkan version + architecture.

---

# 12. Temporary Directory

Gunakan:

```bash
mktemp -d
```

Contoh:

```text
/tmp/nusapanel-install.XXXXXXXX
```

Semua download dan extraction harus dilakukan di temporary directory.

Implementasikan cleanup handler:

```bash
trap cleanup EXIT INT TERM
```

Cleanup harus tetap berjalan ketika:

```text
download gagal
checksum gagal
extract gagal
install.sh gagal
user menekan Ctrl+C
```

Jangan menulis temporary release langsung ke `/opt/panel`.

---

# 13. Download

Gunakan curl dengan mode gagal-keras.

Recommended minimum:

```bash
curl \
  --fail \
  --location \
  --silent \
  --show-error
```

Atau ekuivalen:

```bash
curl -fL --silent --show-error
```

Download:

```text
artifact tar.gz
SHA256SUMS
```

Bootstrap harus memverifikasi response berhasil sebelum melanjutkan.

Jangan menjalankan file yang belum selesai ter-download.

---

# 14. SHA256 Verification

Ini **mandatory**.

Download:

```text
SHA256SUMS
```

Kemudian verifikasi artifact yang dipilih.

Jangan hanya memeriksa bahwa SHA256SUMS berhasil di-download.

Bootstrap harus memastikan hash archive sama dengan release manifest.

Contoh log:

```text
[4/7] Verifying SHA-256 checksum...
      OK
```

Jika hash berbeda:

```text
ERROR: Release checksum verification failed.
The downloaded archive will not be installed.
```

Setelah checksum gagal:

```text
DO NOT extract
DO NOT execute install.sh
DO NOT continue
```

Karena `SHA256SUMS` berisi checksum untuk beberapa artifact, pastikan hanya entry artifact yang sedang digunakan yang dipilih secara aman.

Hindari substring matching ambigu.

---

# 15. Safe Archive Extraction

Sebelum extract, validasi archive.

Jangan percaya penuh terhadap isi remote tarball meskipun berasal dari repository sendiri.

Minimal reject entry yang:

```text
absolute path
dimulai dengan /
mengandung ../
mengandung path traversal
```

Recommended flow:

```text
tar -tzf archive
      ↓
validate paths
      ↓
tar -xzf archive
```

Jika archive mengandung entry berbahaya:

```text
ERROR: Release archive contains unsafe paths.
```

Jangan extract archive unsafe.

---

# 16. Menemukan Installer Internal

Struktur release saat ini berasal dari release workflow:

```text
nusa-panel-<VERSION>-<TARGET>/
├── bin/
│   ├── panel-agent
│   ├── panel-api
│   └── panelctl
├── templates/
│   └── systemd/
├── install.sh
├── uninstall.sh
└── web/
    └── dist/
```

Setelah extract, expected path:

```text
${TMP_DIR}/nusa-panel-${VERSION}-${TARGET}/install.sh
```

Pastikan file:

```text
exists
regular file
executable atau dapat dijalankan dengan bash
```

Jika tidak ada:

```text
ERROR: Release package is invalid: install.sh not found.
```

---

# 17. Menjalankan Installer Internal

Jalankan installer dari root directory hasil extraction.

Contoh:

```bash
cd "$EXTRACTED_DIR"
bash ./install.sh
```

Working directory penting karena existing `install.sh` mungkin menggunakan relative paths seperti:

```text
bin/
templates/
web/
```

Jangan menjalankan:

```bash
bash /tmp/.../install.sh
```

dari working directory arbitrary jika itu dapat merusak relative path resolution.

Exit code dari `install.sh` harus diteruskan.

Jika installer internal gagal:

```text
ERROR: Nusa Panel installation failed.
```

Bootstrap harus keluar non-zero.

---

# 18. Argument Forwarding

Bootstrap harus mendukung meneruskan argument ke installer internal.

Contoh:

```bash
curl -fsSL <URL> | sudo bash -s -- --port 8443
```

harus setara dengan:

```bash
./install.sh --port 8443
```

Implementasikan dengan aman menggunakan:

```bash
"$@"
```

dan **jangan** membangun ulang argument menjadi satu string.

Contoh:

```bash
bash ./install.sh "$@"
```

Ini penting agar existing option seperti:

```text
--port <PORT>
--unattended
--force-os
```

tetap dapat digunakan.

---

# 19. Output / UX

Bootstrap harus memiliki output sederhana dan production-friendly.

Contoh:

```text
Nusa Panel Installer

[1/7] Checking environment...
      Linux detected
      Architecture: x86_64

[2/7] Resolving latest release...
      Latest version: v0.1.0

[3/7] Downloading release...
      nusa-panel-v0.1.0-x86_64-unknown-linux-gnu.tar.gz

[4/7] Verifying SHA-256 checksum...
      OK

[5/7] Validating release archive...
      OK

[6/7] Extracting release...
      OK

[7/7] Starting Nusa Panel installer...
```

Tidak perlu dependency warna terminal.

Jika menggunakan ANSI color, harus otomatis disable ketika:

```text
stdout bukan TTY
NO_COLOR tersedia
```

Tetapi warna bukan requirement.

---

# 20. Error Handling

Gunakan strict shell mode:

```bash
set -Eeuo pipefail
```

Tetapi implementasi error handler harus menghasilkan pesan yang berguna.

Hindari kegagalan seperti:

```text
line 83: variable: unbound variable
```

tanpa konteks.

Error penting yang harus ditangani:

```text
unsupported OS
unsupported CPU architecture
not running as root
curl missing
tar missing
sha256sum missing
GitHub API unreachable
GitHub rate limit / invalid response
latest release unavailable
invalid release tag
release asset unavailable
SHA256SUMS unavailable
checksum entry missing
checksum mismatch
archive corrupt
archive path traversal
install.sh missing
internal installer failure
```

---

# 21. GitHub API Rate Limit

Installer publik nantinya dapat dijalankan banyak server.

Bootstrap tidak boleh membutuhkan authentication token.

Jangan menanam:

```text
GitHub PAT
GITHUB_TOKEN
API key
```

di installer.

Untuk MVP, anonymous GitHub API access diperbolehkan.

Namun desain fungsi version resolver agar nanti mudah diganti ke:

```text
releases.nusapanel.com
```

tanpa rewrite keseluruhan installer.

Pisahkan logic menjadi fungsi semacam:

```bash
resolve_latest_version
build_asset_url
download_release
verify_release
```

Jangan hardcode request di banyak tempat.

---

# 22. Security Requirements

Bootstrap akan dijalankan sebagai root.

Perlakukan seluruh data dari internet sebagai untrusted.

Mandatory:

```text
✅ quote seluruh variable shell
✅ no eval
✅ no source terhadap remote file
✅ no sh -c dengan remote input
✅ validate VERSION
✅ validate TARGET
✅ verify SHA256 sebelum extract
✅ validate tar paths
✅ use mktemp
✅ cleanup temporary directory
✅ fail closed
✅ do not persist GitHub token
✅ do not disable TLS verification
```

Dilarang:

```bash
curl -k
eval "$REMOTE_VALUE"
bash -c "$REMOTE_VALUE"
source downloaded-file
```

Jangan menggunakan:

```text
--insecure
```

pada curl.

---

# 23. Pipe Execution Compatibility

Script memang akan digunakan dengan:

```bash
curl -fsSL URL | sudo bash
```

Karena stdin berisi script, jangan gunakan pola interaktif yang mencoba membaca installer data dari stdin tanpa `/dev/tty`.

Jika existing `install.sh` interaktif, itu dijalankan setelah bootstrap script sudah selesai dibaca oleh Bash sehingga umumnya aman.

Tetapi bootstrap sendiri sebaiknya non-interaktif.

Untuk input yang membutuhkan user confirmation, tanggung jawab berada di internal `install.sh`.

---

# 24. Testing

Minimal buat test/validation untuk:

### Test 1 — Architecture mapping

Input:

```text
x86_64
amd64
aarch64
arm64
```

Expected:

```text
x86_64-unknown-linux-gnu
x86_64-unknown-linux-gnu
aarch64-unknown-linux-gnu
aarch64-unknown-linux-gnu
```

Unknown architecture harus gagal.

### Test 2 — Release tag validation

Valid:

```text
v0.1.0
v1.20.3
```

Invalid:

```text
main
latest
../../tmp
v1.0.0;rm -rf /
$(whoami)
```

### Test 3 — Artifact naming

Untuk:

```text
v0.1.0 + x86_64
```

harus menghasilkan:

```text
nusa-panel-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
```

### Test 4 — Checksum valid

Artifact dummy + SHA256SUMS valid harus lolos.

### Test 5 — Checksum mismatch

Artifact dimodifikasi setelah checksum dibuat harus gagal.

### Test 6 — Unsafe tar path

Archive dengan:

```text
../../etc/passwd
```

atau absolute path harus ditolak sebelum extraction.

### Test 7 — Missing install.sh

Archive valid tetapi tidak memiliki `install.sh` harus gagal.

### Test 8 — Forward arguments

Pastikan:

```bash
bootstrap-install.sh --port 8443 --unattended
```

diteruskan sebagai argument terpisah ke internal installer.

---

# 25. Manual Integration Test

Setelah implementasi, lakukan test tanpa domain custom terlebih dahulu.

Contoh raw GitHub URL bootstrap:

```text
https://raw.githubusercontent.com/Whyriez/Vpanel/main/scripts/bootstrap-install.sh
```

Fresh VPS test:

```bash
curl -fsSL \
  https://raw.githubusercontent.com/Whyriez/Vpanel/main/scripts/bootstrap-install.sh \
  | sudo bash
```

Test forwarding:

```bash
curl -fsSL \
  https://raw.githubusercontent.com/Whyriez/Vpanel/main/scripts/bootstrap-install.sh \
  | sudo bash -s -- --unattended
```

Expected:

```text
bootstrap resolves latest release
→ downloads correct architecture
→ SHA256 verification succeeds
→ extracts safely
→ executes bundled install.sh
```

Jangan menganggap task selesai hanya karena shell syntax valid.

---

# 26. Required Validation Before Completion

Agent wajib menjalankan minimal:

```bash
bash -n scripts/bootstrap-install.sh
```

Jika test script dibuat:

```bash
bash scripts/tests/bootstrap-install-test.sh
```

Pastikan existing project validation yang relevan tidak rusak.

Jika perubahan hanya shell bootstrap dan docs, tidak perlu menjalankan seluruh test suite Rust yang sangat berat kecuali ada file Rust yang ikut berubah.

---

# 27. Compatibility dengan Existing Release Workflow

Release workflow saat ini menghasilkan:

```text
nusa-panel-${VERSION}-${TARGET}.tar.gz
```

dan:

```text
SHA256SUMS
```

Target:

```text
x86_64-unknown-linux-gnu
aarch64-unknown-linux-gnu
```

Bootstrap harus mengikuti format existing ini.

Jangan mengubah naming artifact tanpa alasan kuat.

Jika agent menemukan mismatch nyata antara archive content dan kebutuhan existing `install.sh`, lakukan perubahan sekecil mungkin dan dokumentasikan alasannya.

---

# 28. Future Compatibility

Desain bootstrap supaya source release dapat diganti nanti.

Hari ini:

```text
GitHub Releases
```

Nanti:

```text
releases.nusapanel.com
```

Idealnya definisikan satu blok konfigurasi:

```bash
GITHUB_OWNER="Whyriez"
GITHUB_REPO="Vpanel"
```

dan helper:

```text
resolve_latest_version()
release_asset_url()
checksum_url()
```

Agar migration dari GitHub ke CDN/R2 cukup mengubah layer resolver/downloader.

Jangan mengimplementasikan CDN sekarang.

---

# 29. Documentation

Tambahkan dokumentasi singkat, misalnya:

```text
docs/bootstrap-installer.md
```

Minimal berisi:

```text
tujuan bootstrap installer
cara test melalui raw.githubusercontent.com
argument forwarding
arsitektur yang didukung
hubungan bootstrap dengan install.sh
security verification SHA256
troubleshooting umum
```

Jangan mengganti dokumentasi instalasi publik utama ke `get.nusapanel.com` sebelum domain tersebut benar-benar tersedia.

Boleh menambahkan status sebagai:

```text
Developer / pre-production bootstrap URL
```

---

# 30. Definition of Done

Task dianggap selesai hanya jika:

- [ ] `scripts/bootstrap-install.sh` tersedia.
- [ ] Script menggunakan `set -Eeuo pipefail`.
- [ ] Script memastikan dijalankan sebagai root.
- [ ] Script hanya berjalan di Linux.
- [ ] x86_64 terdeteksi dengan benar.
- [ ] aarch64 terdeteksi dengan benar.
- [ ] Latest GitHub Release ditentukan otomatis.
- [ ] Version/tag dari remote divalidasi.
- [ ] Artifact name dibangun secara deterministik.
- [ ] Artifact release berhasil di-download.
- [ ] `SHA256SUMS` berhasil di-download.
- [ ] SHA256 archive diverifikasi sebelum extract.
- [ ] Checksum mismatch menghentikan instalasi.
- [ ] Archive path divalidasi sebelum extraction.
- [ ] Archive diekstrak ke `mktemp` directory.
- [ ] Bundled `install.sh` ditemukan.
- [ ] Internal installer dijalankan dari working directory release.
- [ ] Argument user diteruskan menggunakan `"$@"`.
- [ ] Temporary directory selalu dibersihkan.
- [ ] Exit status internal installer diteruskan dengan benar.
- [ ] Tidak ada `eval`.
- [ ] Tidak ada `curl -k` / `--insecure`.
- [ ] Tidak ada hardcoded release version.
- [ ] Tidak ada GitHub token/PAT di source.
- [ ] `bash -n scripts/bootstrap-install.sh` berhasil.
- [ ] Unit/shell tests bootstrap berhasil.
- [ ] Manual integration test terhadap GitHub Release berhasil.
- [ ] Existing release workflow tetap kompatibel.
- [ ] Dokumentasi bootstrap ditambahkan.

---

# 31. Jangan Kerjakan Task Berikutnya

Setelah bootstrap installer selesai, berhenti.

Jangan langsung mengimplementasikan:

```text
get.nusapanel.com
Cloudflare Worker
Cloudflare R2
panelctl update
release channels
signature Ed25519
```

Tahap berikutnya akan dikerjakan sebagai task terpisah setelah bootstrap installer terbukti bekerja pada fresh VPS.

---

# Expected Result

Setelah task ini selesai, command berikut harus bekerja untuk testing:

```bash
curl -fsSL \
  https://raw.githubusercontent.com/Whyriez/Vpanel/main/scripts/bootstrap-install.sh \
  | sudo bash
```

Bootstrap kemudian otomatis mengambil release terbaru dari:

```text
Whyriez/Vpanel
```

memilih artifact sesuai CPU, melakukan verifikasi SHA-256, mengekstrak package, lalu menjalankan installer internal Nusa Panel.

Target tahap selanjutnya, **bukan bagian task ini**, adalah mengganti URL raw GitHub tersebut menjadi:

```bash
curl -fsSL https://get.nusapanel.com | sudo bash
```
