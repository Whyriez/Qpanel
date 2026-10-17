# Paket release dan kegagalan installer

Jika bootstrap berhenti pada `Template systemd/Nginx paket tidak lengkap`,
arsip release yang diunduh kehilangan file wajib. Checksum yang benar hanya
membuktikan unduhannya utuh; checksum tidak membuktikan isi paket lengkap.

Pada laporan 17 Oktober 2026, workflow GitHub masih mengemas hanya
`nusa-panel.conf`, sedangkan installer juga memerlukan
`templates/nginx/nusa-reject-unknown.conf`. Panduan akun admin juga tertinggal.
Script build lokal sudah menyalin keduanya, sehingga pengujian sebelumnya belum
mencakup jalur packaging GitHub yang sebenarnya.

## Pengemasan yang digunakan

Build lokal (`scripts/build-release.sh`) dan workflow Release sekarang memanggil
`scripts/package-release.sh`. Script bersama ini:

- Memastikan tag sama dengan versi `[workspace.package]` di `Cargo.toml`.
- Mewajibkan tiga biner, installer/uninstaller, tiga unit systemd, proxy panel,
  guard hostname asing, panduan admin, serta index dan JavaScript web hasil build.
- Memeriksa daftar file dalam arsip dan membuat checksum setelah pengemasan
  berhasil; staging sementara dibersihkan bila gagal.
- Tidak menjalankan biner panel saat mengemas. GLIBC tetap diperiksa oleh jalur
  build sebelum packaging.

Installer menyebut file template yang hilang dan berhenti sebelum menjalankan
biner, memasang paket, atau mengubah konfigurasi. Guard Nginx tetap wajib.

## Membangun ulang release development v0.1.0

Versi development tetap **0.1.0** sampai pemilik proyek memutuskan menaikkannya
setelah fitur aman atau pada pengembangan fitur berikutnya. Workspace Cargo,
dependency internal, lockfile, frontend, tag dan nama paket memakai versi yang
sama. Validasi tag/Cargo tetap aktif; perbaikan bug tidak memerlukan bump versi.

Push ke `main` atau mengunduh ulang bootstrap belum memperbarui tarball yang
sudah terbit. Untuk mengganti aset development `v0.1.0` dengan build terbaru:

1. Commit dan push perubahan ke `main`. Pastikan CI lulus.
2. Buka **Actions → Release → Run workflow**, pilih branch **main**, dan isi
   tag **v0.1.0**. Pilih branch yang sudah memuat perbaikan; `Re-run jobs` dari
   run/tag lama tetap menggunakan commit lama.
3. Aktifkan **Publikasikan atau perbarui aset release pada tag ini** (`publish`).
   Opsi ini default mati; jika tidak diaktifkan, run hanya menghasilkan artifact
   untuk diperiksa dan tidak mengganti unduhan publik.
4. Tunggu kedua build arsitektur dan job publikasi berhasil. Workflow memakai
   `tag_name: v0.1.0` dan mengganti tarball serta `SHA256SUMS` di release tersebut.
   Tidak perlu menghapus atau memindahkan tag development yang sudah ada.
5. Setelah publikasi selesai, ulang bootstrap. Untuk memilih versi secara
   eksplisit pada server Linux:

   ```bash
   sudo env NUSA_PANEL_VERSION=v0.1.0 bash /tmp/bootstrap-install.sh
   # Instalasi existing:
   sudo env NUSA_PANEL_VERSION=v0.1.0 bash /tmp/bootstrap-install.sh --upgrade
   ```

Hanya jalankan salah satu command sesuai kondisi server. Rebuild development
dapat menghasilkan checksum berbeda meski versinya tetap 0.1.0; unduh tarball
dan checksum dari publikasi yang sama, lalu verifikasi kembali. Source archive
otomatis GitHub tetap mengikuti commit tag; untuk menginstal build branch terbaru,
gunakan tarball biner dari workflow dan lihat commit pada run tersebut.

Penggantian aset existing mengikuti input `tag_name` dan `overwrite_files` pada
[action release](https://github.com/softprops/action-gh-release/tree/v2#inputs).
Kenaikan versi berikutnya dilakukan saat diminta, dengan menyelaraskan Cargo,
dependency internal, lockfile, frontend dan tag, lalu menerbitkan tag versi baru.

## Memeriksa artifact sebelum instalasi

Pada Linux, dari direktori unduhan release:

```bash
sha256sum -c SHA256SUMS
archive='nama-file-release-yang-diunduh.tar.gz'
tar -tzf "$archive"
```

Daftar arsip harus memuat `templates/nginx/nusa-reject-unknown.conf`,
`templates/systemd/panel-proc-hardening.service`, `docs/admin-accounts.md`,
`web/dist/index.html`, aset JavaScript, dan ketiga biner. Gunakan nama target
`aarch64-unknown-linux-gnu` untuk ARM64. Jangan melonggarkan preflight installer
untuk meloloskan paket lama yang kehilangan guard.

## Snapshot gagal pada Debian 12

Jika log sudah menampilkan `panel-api 0.1.0` dan APT berhasil, lalu berhenti
dengan `cp: 'etc/nginx/sites-enabled': No such file or directory`, masalah berada
pada snapshot installer. Ini cocok dengan regresi `cp -a --parents` untuk path
absolut pada Coreutils 9.1, yang dicatat di
[catatan upstream 9.4](https://github.com/coreutils/coreutils/blob/v9.4/NEWS).

Installer sekarang membuat direktori snapshot secara eksplisit lalu menyalin
file dengan `cp -a` tanpa `--parents`. Metadata dan symlink Nginx tetap disimpan,
dan layout pemulihan tetap sama. Kegagalan snapshot menghentikan proses sebelum
layanan panel dihentikan atau file panel diganti.

Publikasikan ulang **v0.1.0 dari main** dengan input **publish** aktif seperti
langkah di atas. Setelah selesai, unduh ulang bootstrap dan jalankan kembali.
Tidak perlu menghapus Nginx/default config atau menaikkan versi. Paket APT yang
sudah berhasil dipasang tetap ada; tahap snapshot ini belum membuat akun admin
atau memasang layanan panel. Bila sudah ada instalasi panel, gunakan `--upgrade`.

Untuk pemeriksaan read-only pada server bila masih gagal:

```bash
cp --version | head -n 1
ls -ld /etc/nginx /etc/nginx/sites-enabled /var/backups/panel
```

## Peringatan waktu dari tar

`time stamp ... is ... s in the future` menunjukkan waktu file arsip lebih maju
daripada jam mesin saat ekstraksi. Pada lampiran, ekstraksi tetap berhasil;
kegagalan fatal terjadi sesudahnya karena template hilang. Periksa waktu UTC
dan sinkronisasi server sebelum melanjutkan, tanpa menyembunyikan peringatannya:

```bash
date -u
timedatectl status
```

Periksa juga waktu mesin build bila jam server benar. Jam yang keliru dapat
mengganggu TLS dan autentikasi.

## Pengujian

```bash
bash scripts/tests/install-regression-test.sh
```

Fixture menjalankan blok packaging yang dibaca langsung dari workflow Release
untuk x86_64 dan ARM64, menggunakan compiler/readelf tiruan. Kasus negatif
mencakup file wajib hilang/kosong, tag salah, kegagalan tar, serta diagnostik
preflight tanpa mengeksekusi biner panel asli. Ini tidak membuktikan instalasi
Ubuntu, Nginx, systemd, atau release publik berhasil.
