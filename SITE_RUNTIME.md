# Pembuatan website PHP, Node.js dan Python

Versi development tetap **0.1.0**. Perbaikan ini memerlukan pembaruan **panel-api
dan panel-agent bersama**, lalu penerapan ulang konfigurasi website existing.
Mengganti biner saja belum meregenerasi unit systemd dan vhost yang sudah ada.

## Perubahan perilaku

- PHP-FPM memiliki `[Install]` agar service tenant dapat di-enable. PID/socket
  berada dalam direktori runtime milik tenant, dan log dapat ditulis oleh tenant.
  Validasi konfigurasi dijalankan sebagai tenant, sehingga tidak membuat log baru
  dengan owner root. Log lama diperbaiki melalui descriptor tanpa menimpa isinya.
- Socket FPM memakai ACL `www-data` untuk koneksi Nginx. Service tetap berjalan
  dengan user/group tenant, dengan sandbox yang sama.
- Induk socket `/run/panel` memberi ACL traverse saja kepada tenant dan Nginx.
  Kepemilikan direktori leaf saja tidak cukup jika induknya `root:panel 0750`.
  Kegagalan memasang ACL menghentikan provisioning. Socket agent tetap 0660
  root:panel dan tenant tidak ditambahkan ke grup panel.
- Node.js membuat `server.js` dan `package.json` minimal dengan script `start`.
  Kode JavaScript tidak lagi ditulis ke `index.html`.
- Python membuat file sesuai entrypoint standar: `app.py`, `main.py`, `wsgi.py`,
  atau `myproject/wsgi.py`. WSGI dan ASGI memiliki handler yang berbeda. Starter
  ini adalah halaman awal yang dapat dijalankan, bukan instalasi penuh framework.
- Python menyiapkan venv dan server Gunicorn/Uvicorn yang belum ada sebagai UID
  tenant. Masing-masing persiapan memiliki batas runtime 30 detik dan stop dua
  detik. Paket memerlukan akses registry; kegagalannya membatalkan provisioning.
  Dependency framework/aplikasi milik pengguna tetap dikelola melalui deployment.
- Proses diperiksa sebelum vhost Nginx dipublikasikan. Node diperiksa melalui HTTP
  loopback; Python diperiksa melalui socket Unix dengan identitas `www-data`.
  Service yang belum bisa melayani request tidak menghasilkan create sukses.

Runtime harus sudah dipasang dari Runtime Manager. Entrypoint custom harus
tersedia di folder aplikasi; starter tidak menebak atau menimpa kode custom.
Wizard membaca versi Node.js/Python yang terpasang dari server, tanpa daftar versi
statis. Jika inventaris kosong atau gagal dimuat, gunakan tombol coba lagi atau
pasang runtime dari Runtime Manager; create tidak dikirim dengan versi tebakan.

Penolakan provisioning PHP/Node.js/Python dikirim sebagai JSON 422 dengan detail
tahap; gangguan komunikasi agent sebagai 503. Penolakan aplikasi tidak lagi
memakai 502 yang dapat diganti oleh halaman Cloudflare. Ini tidak memperbaiki
proxy/origin yang benar-benar mati; penyebab kejadian server tetap perlu
dibedakan dari hasil regresi lokal. Unit agent mempertahankan runtime directory
selama restart agar socket aplikasi existing tidak ikut terhapus.

## Menghapus lalu memakai ulang domain

Menghapus website membersihkan konfigurasi Nginx, proses dan environment service.
Untuk Python, socket milik situs juga dibersihkan. Bila cleanup gagal, record
situs/domain dipertahankan agar domain tidak dibuat ulang di atas proses lama.
Provisioning gagal dengan cleanup yang belum lengkap mempertahankan situs dalam
status suspended untuk ditinjau dan dicoba hapus lagi.

File project tetap dipertahankan. Saat situs dibuat ulang dengan runtime berbeda,
starter yang sesuai dibuat hanya jika file tersebut belum ada. Contoh: file
`index.html` lama boleh masih tersimpan, tetapi Node menjalankan `server.js` dan
Python menjalankan modul WSGI/ASGI. File lama bukan penentu runtime website.

## Memasang perbaikan

1. Commit/push perubahan. Setelah CI lulus, jalankan **Actions → Release → Run
   workflow** dari **main**, tag **v0.1.0**, dengan **publish** aktif.
2. Sesudah kedua build dan publikasi berhasil, pada server unduh ulang bootstrap
   lalu jalankan upgrade:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/Whyriez/Vpanel/main/scripts/bootstrap-install.sh -o /tmp/bootstrap-install.sh
   sudo env NUSA_PANEL_VERSION=v0.1.0 bash /tmp/bootstrap-install.sh --upgrade
   ```

3. Buat ulang situs percobaan yang sebelumnya gagal. Untuk aplikasi existing,
   simpan ulang konfigurasi runtime/pool dan Site Rules agar unit, pool dan
   vhost diregenerasi bersama. Jangan menghapus file aplikasi untuk regenerasi.
4. Socket baru FPM: `/run/panel/php/<tenant>-<versi>/fpm.sock`.
   Socket Python: `/run/panel/python/site_<id>/app.sock`. Path ini divalidasi
   sebelum persiapan venv agar konsisten dengan cleanup. Override socket Python
   lama perlu diselaraskan dengan path situs tersebut.

Jangan memakai Re-run jobs dari run lama karena commit sumbernya tetap lama.
Lihat [panduan pengemasan](RELEASE_PACKAGING.md) untuk pembaruan aset v0.1.0.

## Pemeriksaan di server

Gunakan nama tenant, versi dan ID website yang terlihat di panel. Contoh read-only:

```bash
systemctl cat panel-fpm@p_tokoalim-8.4.service
systemctl status panel-fpm@p_tokoalim-8.4.service --no-pager
journalctl -u panel-fpm@p_tokoalim-8.4.service --no-pager -n 60
# Ganti site_1 dengan ID situs:
systemctl status site-site_1.service --no-pager
journalctl -u site-site_1.service --no-pager -n 60
```

Pengujian lokal memakai runner/protokol tiruan dan SQLite sementara. Starter
Node/Python juga dapat diuji sebagai child process lokal tanpa daemon panel.
Ini belum membuktikan systemd, FPM, ACL/socket, registry Python atau Nginx di
server produksi telah berhasil.
## Katalog runtime dan instalasi Python

Runtime Manager menyediakan tab Python dengan pemasangan melalui tugas latar
belakang, progres yang tetap tersedia setelah navigasi, serta penghapusan yang
ditolak bila runtime dipakai situs. Pemasangan Python memerlukan admin dan CSRF;
respons API instalasi kini JobItem HTTP 202, termasuk alias endpoint legacy.

Di **Runtime > Python**, tombol **Pasang Python** selalu tampil di atas daftar,
termasuk ketika katalog belum tersinkronisasi. Dialog menyediakan pilihan
rilis stabil terbaru, **Muat daftar versi Python**, serta **Masukkan versi lengkap**
untuk versi CPython eksplisit seperti `3.13.9`. Isian manual bukan rekomendasi versi
terbaru; administrator memilih rilis yang masih didukung. Ketersediaan binary
diperiksa saat tugas berjalan. Membuka tab/dialog atau memperbarui katalog tidak
memasang Python secara otomatis.

Tab dapat dibuka langsung melalui `/runtime?tab=python` dan tetap dipertahankan
setelah reload. Administrator juga dapat membuka pengelola versi dari pengaturan
aplikasi Python situs. Tenant memilih konfigurasi situs; pemasangan runtime sistem
tetap dilakukan administrator. Perubahan UI ini perlu masuk ke bundle frontend
release yang dipasang di server; mengambil bootstrap dari branch main sendiri
tetap mengunduh binary/bundle dari GitHub Release sesuai tag yang dipilih.

Tab **PHP**, **Node.js**, **Python**, dan **Pengaturan** tampil berdampingan.
Jadwal serta status sumber katalog dikelola di **Runtime > Pengaturan**
(`/runtime?tab=settings`); tab bahasa berisi pengelolaan runtime masing-masing.
Monitoring Proses dan Server Basis Data tetap tersedia setelah keempat tab ini.

Pembaruan katalog berjalan harian secara default, dapat diubah menjadi 6/12/24 jam
atau mingguan, dan dapat dimatikan. Tombol **Perbarui katalog sekarang** hanya
mengambil metadata; tidak memasang runtime atau mengganti versi situs. Status tiap
sumber dan jadwal tersimpan setelah restart API. Bila sumber gagal, snapshot sukses
terakhir dipertahankan. Snapshot lebih dari tujuh hari perlu disinkronkan ulang
sebelum menawarkan pemasangan baru.

Node.js menawarkan lima patch terbaru per lini LTS yang masih mendapat dukungan
resmi, dengan pemeriksaan tanggal EOL pada katalog dan tugas instalasi. Node yang
sudah terpasang tetap terlihat, termasuk versi EOL milik situs existing. PHP kini
mendukung 8.5 dan menampilkan patch terbaru cabang aktif dari php.net; pemasangan
manual mengambil kandidat paket APT. Cabang PHP baru di luar whitelist membutuhkan
upgrade panel. Python menggunakan patch stabil terbaru per lini yang didukung dari
metadata Astral/Python, bukan daftar patch lama yang statis.

PHP 7.4 tetap tersedia untuk kompatibilitas aplikasi lama, termasuk ketika belum
terpasang. Seluruh versi pada whitelist panel (7.4 dan 8.0–8.5) tetap ditampilkan;
refresh katalog PHP hanya memperbarui informasi patch cabang aktif dan tidak
menyembunyikan versi kompatibilitas. Administrator dapat memasangnya melalui job
instalasi biasa; pemeriksaan kandidat paket dan izin tetap berlaku.

PHP 8.5 memuat OPcache sebagai bawaan binary. Installer tidak meminta paket
`php8.5-opcache`; daftar ekstensi menampilkan OPcache sebagai modul inti dan
pengaturan cache tetap dikelola melalui pengaturan PHP situs.

Upgrade pasangan API/agent serta bundle web untuk memakai perubahan ini. Tidak ada
upgrade/penghapusan otomatis terhadap runtime atau situs existing. ADR-0118 dan
docs/PROGRESS.md mencatat validasi lokal dan acceptance Linux yang masih terpisah.

## Pengelola database internal

Setelah memperbarui API, agent dan bundle web, administrator membuka **Runtime >
Server Basis Data > Pasang pengelola database**. Instalasi phpMyAdmin (MariaDB/MySQL),
Adminer (PostgreSQL), dan layanan privat berjalan sebagai tugas latar belakang.
Tombol **Perbarui pengelola** juga memasang ulang bridge milik panel ketika upgrade.

Tenant membuka **Situs > Database > Buka phpMyAdmin / Buka Adminer**. Pengelola
terbuka di tab baru pada alamat panel dan langsung memakai sesi panel. Tidak perlu
mengaktifkan remote access atau membuka port 3306/5432. Browser perlu mengizinkan
tab baru. Akun web khusus dibatasi ke database tujuan; sandi aplikasi tetap sama.

Jika alat belum siap, administrator memeriksa kartu pengelola dan log
`journalctl -u panel-db-web.service --no-pager -n 100` pada server. Jangan membuat
alias Nginx publik langsung ke direktori phpMyAdmin/Adminer, karena akses harus
melewati pemeriksaan sesi dan kepemilikan panel. Impor melalui pengelola dibatasi
30 MiB per berkas; dump besar tetap memakai fitur job impor/ekspor yang sudah ada.

Layanan menggunakan socket Unix, user non-root khusus, dan paket APT distribusi.
Validasi sesi ini menggunakan mock/fixture lokal; instalasi dan akses database nyata
di Linux belum diverifikasi. Rincian desain dan batas ada pada ADR-0119.
