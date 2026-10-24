# Pembaruan versi QonPanel

Setelah paket yang memuat updater ini terpasang, administrator membuka
**Pengaturan server → Pembaruan**, memilih **Periksa pembaruan**, lalu
**Terapkan pembaruan** dan mengonfirmasi versi. Tidak perlu menghapus panel,
membuat akun ulang, atau memasang ulang website.

Halaman menampilkan versi terpasang, rilis stabil, catatan rilis, progres dan
diagnostik. Proses berjalan dalam layanan terpisah sehingga restart API/agent,
menutup halaman, atau navigasi tidak menghentikan update. Setelah selesai,
**Muat ulang panel** mengambil antarmuka versi baru.

## Server existing yang belum memiliki tombol ini

Bangun dan publikasikan paket release lengkap yang memuat patch P10-T6cs.
Lakukan **upgrade** paket sekali menggunakan prosedur yang sudah tersedia:

```bash
curl -fsSL https://raw.githubusercontent.com/Whyriez/QonPanel/main/scripts/bootstrap-install.sh -o /tmp/bootstrap-install.sh
sudo env QONPANEL_VERSION=v0.1.0 bash /tmp/bootstrap-install.sh --upgrade
```

Ganti tag bila versi yang dipublikasikan sudah berubah. Ini upgrade instalasi
existing, bukan instalasi dari awal. Mengambil bootstrap terbaru tanpa membangun
ulang release tidak mengubah isi paket biner. Lihat RELEASE_PACKAGING.md.
Setelah updater tersedia, update berikutnya dapat diterapkan dari panel.
Server dari unit agent lama dapat memerlukan restart runtime satu kali setelah
upgrade pertama untuk membuat ulang socket. Perbaikan lifecycle socket berikutnya
ikut paket ini; lihat [SITE_RUNTIME.md](SITE_RUNTIME.md). File aplikasi tetap ada.

## Data yang dipertahankan

Installer mempertahankan akun/password/2FA, website dan file aplikasi, runtime
PHP/Node/Python terpasang, domain/sertifikat serta pengaturan database. Port publik
existing tetap digunakan. Panel update tidak menjalankan upgrade paket APT atau
mengganti runtime website. Unit dan konfigurasi panel diperbarui bersama API,
agent, CLI serta frontend sebagai satu paket.

Snapshot biner/config terkelola dan SQLite dibuat sebelum migrasi. Migrasi yang
belum diterapkan dijalankan sebagai identity panel-api. Bila pemasangan/migrasi/
health check gagal, installer mencoba memulihkan snapshot sebelum menyatakan
gagal. Kegagalan pemulihan tetap perlu diperiksa, bukan dianggap sukses.
Snapshot ini mencakup keadaan panel; ini bukan pengganti backup file website dan
database aplikasi tenant.

## Kompatibilitas setiap rilis

- `UPGRADE_FROM` di dalam paket menentukan versi minimum asal yang didukung.
  Baseline saat ini v0.1.0. Paket inkompatibel dan downgrade ditolak sebelum
  menghentikan layanan. Rilis yang memerlukan versi perantara harus menjelaskannya.
- **Jangan mengedit atau menghapus SQL migrasi yang sudah dirilis.** Tambahkan
  migrasi baru; pertahankan data lama dan nilai default yang kompatibel. Untuk
  perubahan format, lakukan bertahap: baca format lama/baru, migrasikan, baru
  hentikan dukungan format lama dalam rilis berikutnya yang direncanakan.
- Pertahankan identifier runtime/storage dan kontrak IPC yang dibutuhkan data
  existing. Konfigurasi baru perlu upgrade idempoten yang terdaftar dalam snapshot/
  rollback installer. Jangan merender ulang semua website atau mengubah versi
  runtime tenant hanya karena frontend mendapat fitur baru.
- Versi stabil berikutnya memakai tag baru: patch untuk bug, minor untuk fitur
  yang kompatibel, major untuk perubahan yang memang tidak kompatibel. Selaraskan
  versi Cargo/dependency internal/lockfile/frontend/tag melalui proses release.
  Jangan menimpa artifact versi stabil yang sudah digunakan.
- Development tetap v0.1.0 sesuai keputusan proyek. Selama tahap ini, rebuild tag
  yang sama dibedakan melalui digest paket. Panel mengizinkan reapply paket baru
  v0.1.0; untuk versi stabil berikutnya tombol memerlukan nomor versi baru.

Rilis harus lulus CI, test upgrade dari database/config versi sebelumnya, serta
uji rollback sebelum dipakai di server stabil. Menambah fitur tidak otomatis
menjamin semua kombinasi server kompatibel; batas dukungan paket harus benar.

## Bila koneksi panel terputus atau update gagal

Tunggu panel tersambung ulang. Halaman mengambil status yang disimpan server,
tanpa mengirim ulang apply secara otomatis. Permintaan yang jawabannya hilang
dapat dikirim ulang dengan identitas yang sama tanpa menjalankan update ganda.
Tombol **Lihat diagnostik** membaca log layanan updater.

Bila server reboot/mati ketika update berjalan, status ditandai terputus setelah
layanan diperiksa, dan pekerjaan tidak otomatis diputar ulang. Periksa keadaan
server sebelum mencoba kembali. Jika panel tidak kembali tersedia, gunakan SSH:

```bash
sudo journalctl -u panel-update.service -u panel-api -u panel-agent --no-pager -n 100
```

Perbaikan berikutnya tetap dapat diterapkan melalui upgrade paket. Jangan mencoba
menjalankan biner versi lama di atas database baru secara langsung. Gunakan
snapshot pasangan config/biner/database bila perlu pemulihan manual.

## Sumber release dan batas pengujian

Updater menggunakan repository tetap Whyriez/QonPanel, metadata rilis stabil dan
digest SHA-256 dari [GitHub Releases API](https://docs.github.com/en/rest/releases/releases).
Digest yang diperiksa admin dipatok lagi ketika paket diunduh; checksum berbeda
atau artifact berubah menggagalkan pemasangan. API tidak menerima URL, perintah,
repository, atau path bebas untuk updater.

Implementasi ini diuji lokal dengan runner, installer, jaringan dan API browser
tiruan serta SQLite sementara. Uji Ubuntu 24.04, restart layanan nyata, update
GitHub Release, rollback host dan reboot masih memerlukan acceptance terpisah.

## Rebrand QonPanel

Logo, loading, dokumentasi, installer dan paket utama kini QonPanel. Repository
resmi `Whyriez/QonPanel`; redirect GitHub dari nama sebelumnya mempertahankan
akses bootstrap lama. Jangan menggunakan kembali nama repository lama karena
redirect tersebut diperlukan instalasi existing.

Layanan `panel-*`, perintah `panelctl`, lokasi database, context enkripsi,
header internal dan preferensi `nusa_theme` dipertahankan. Migrasi branding
hanya mengganti nama bawaan produk; nama pelanggan dan kredensial tetap utuh.
Variabel bootstrap `QONPANEL_OWNER`, `QONPANEL_REPO`, `QONPANEL_VERSION`,
`QONPANEL_EXPECTED_SHA256` tersedia; alias `NUSA_PANEL_*` masih diterima.

## Pindah server

Gunakan **Pengaturan server > Backup dan migrasi**. Snapshot upgrade di atas
tidak menggantikan backup migrasi terenkripsi. Lihat [SERVER_BACKUP.md](SERVER_BACKUP.md)
untuk arsip penuh terkelola, restore pada instalasi baru, kebutuhan kapasitas,
cutover DNS dan batas kompatibilitas.
