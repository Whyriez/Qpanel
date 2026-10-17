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

## Menerbitkan perbaikan

Push ke `main` atau mengunduh ulang bootstrap tidak memperbaiki tarball yang
sudah dipublikasikan. Pengelola perlu membuat release baru dari commit perbaikan.

1. Commit dan push perubahan. Pastikan CI lulus.
2. Pilih versi baru yang belum pernah dipublikasikan. Selaraskan versi workspace,
   versi dependency internal di `Cargo.toml`, dan `Cargo.lock` bila perlu.
   Nama tag harus `v` diikuti versi tersebut. Jangan menamai biner `0.1.1`
   sebagai paket `v0.1.0`, atau memindahkan tag lama ke commit lain.
3. Untuk memeriksa artifact terlebih dahulu, jalankan **Actions → Release → Run
   workflow** pada branch perbaikan dengan tag yang sama dengan versi Cargo.
   Mode ini hanya menghasilkan artifact; belum membuat GitHub Release.
4. Setelah artifact diperiksa, buat dan push tag baru pada commit perbaikan.
   Contoh Bash berikut mengambil versi dari Cargo, bukan memakai tag lama:

   ```bash
   version=$(awk '/^\[workspace.package\]/{p=1;next} /^\[/{p=0} p && /^version *=/{split($0,a,"\"");print a[2];exit}' Cargo.toml)
   git tag -a "v${version}" -m "Qpanel v${version}"
   git push origin "v${version}"
   ```

5. Tunggu workflow Release dari tag tersebut berhasil dan kedua tarball beserta
   `SHA256SUMS` tersedia. Bootstrap baru boleh dicoba lagi setelah release baru
   tersedia. Untuk memastikan versi, gunakan `NUSA_PANEL_VERSION=v<versi-baru>`.

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
