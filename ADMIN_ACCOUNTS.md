# Akun administrator QonPanel

## Instalasi baru

Jalankan installer dari paket release lengkap pada Ubuntu yang didukung:

```bash
sudo bash install.sh --unattended
# Bantuan dan port alternatif:
bash install.sh --help
sudo bash install.sh --unattended --port 19090
```

Installer membuat username acak seperti `admin_12ab34cd56ef` dan password acak
secara otomatis. Tidak ada token setup atau pendaftaran admin melalui browser.
Kredensial baru hanya dicetak setelah seluruh pemeriksaan kesehatan berhasil:

```text
  QonPanel | Instalasi berhasil
  ----------------------------------------
  Panel     : http://203.0.113.10:17890
  Status    : API, agent, Nginx, dan SQLite siap
  Snapshot  : /var/backups/panel/install-XXXXXXXX

  Akun administrator
  Username  : admin_12ab34cd56ef
  Password  : <password acak dari installer>
  Simpan sekarang. Password hanya ditampilkan sekali.
```

Simpan kredensial di password manager, buka URL panel, lalu login menggunakan
username/password tersebut. Gunakan alamat HTTPS panel jika telah mengatur akses
HTTPS atau Cloudflare. IP yang dicetak berasal dari alamat antarmuka server;
sesuaikan dengan IP publik atau hostname panel jika server memakai NAT/tunnel.

Password disimpan sebagai hash Argon2id. Tidak ada file password awal atau endpoint
untuk menampilkan password lama. Hindari merekam terminal atau mengarahkan output
instalasi/bootstrap/reset yang berisi password baru ke log bersama.

## Mengubah username atau password melalui panel

1. Masuk, lalu buka **Profil & keamanan** (`/profile`).
2. Pada **Username dan kata sandi**, isi username yang diinginkan. Username admin
   memakai 3-32 karakter: huruf ASCII, angka, `_`, atau `-`.
3. Masukkan kata sandi saat ini untuk mengonfirmasi identitas.
4. Isi kata sandi baru dan konfirmasinya jika ingin mengubah password. Kosongkan
   kedua isian tersebut jika hanya mengganti username.
5. Tekan **Simpan**. Perubahan username/password dilakukan bersamaan; jika username
   sudah dipakai atau penyimpanan gagal, seluruh perubahan dibatalkan.

Sesi browser saat ini diperbarui. Sesi lain serta API token milik akun dicabut;
integrasi yang memakai token perlu membuat token baru. Pengaturan 2FA tetap aktif.
Username login admin tidak mengubah akun Linux, direktori, atau kepemilikan situs.
Perubahan kredensial tidak tersedia selama impersonasi pengguna.

## Lupa username atau password

Masuk ke server melalui SSH dengan akun yang memiliki akses sudo:

```bash
# Cari username admin:
sudo panelctl admin list

# Ganti NAMA dengan username yang ditemukan. Password baru dibuat otomatis:
sudo panelctl admin reset-password --username NAMA
```

Perintah reset mencetak password baru satu kali. Semua sesi dan API token milik
akun dicabut, termasuk browser yang sedang digunakan. Login kembali dengan
username lama dan password baru. Reset password tidak menghapus 2FA dan tidak
mengaktifkan akun yang sedang disuspend. Jika login sedang terkena pembatasan
percobaan gagal, tunggu durasi lockout sebelum mencoba kembali.

Jika akses 2FA juga hilang, gunakan recovery code yang disimpan sebelumnya.
Pemulihan darurat melalui SSH tersedia secara terpisah:

```bash
sudo panelctl admin reset2fa --username NAMA
```

Reset 2FA menghapus secret dan recovery code lama serta mencabut sesi/API token.
Aktifkan kembali 2FA melalui panel setelah berhasil masuk.

Untuk menentukan password sendiri tanpa menaruhnya di argumen proses atau riwayat
shell, gunakan stdin. Contoh Bash pada server:

```bash
read -rsp 'Password baru (minimal 8 karakter): ' qpanel_new_password; printf '\n'
printf '%s' "$qpanel_new_password" | sudo panelctl admin reset-password --username NAMA --password-stdin
unset qpanel_new_password
```

Jalankan `sudo -v` sebelum contoh tersebut jika sudo perlu meminta password.
Input stdin mempertahankan spasi dan menghapus newline penutup; maksimum 4096 byte.
Opsi `--password` lama masih diterima untuk kompatibilitas, tetapi tersembunyi dari
bantuan karena dapat memperlihatkan secret melalui argumen proses.

## Upgrade dan instalasi ulang

```bash
sudo bash install.sh --upgrade --unattended
```

Installer mempertahankan username, hash password, status akun, dan 2FA jika admin
sudah ada. Tidak ada password baru yang dicetak saat upgrade. Ini juga berlaku
untuk akun suspended: installer tidak membuat akun pengganti untuk melewati status
tersebut. Jika admin belum pernah dibuat pada instalasi lama, installer membuatnya
secara otomatis dan menghapus token setup legacy dalam transaksi yang sama.

Jika output instalasi hilang, gunakan `admin list` dan `admin reset-password`.
Menjalankan ulang bootstrap tidak akan memperlihatkan password lama:

```bash
sudo panelctl admin bootstrap
```

Bootstrap membuat tepat satu akun ketika belum ada admin, termasuk bila dua proses
berjalan bersamaan. Akun baru baru dilaporkan setelah commit database. Kegagalan
penyimpanan atau pembersihan token membatalkan transaksi sehingga dapat dicoba lagi.
URL `/setup` kini hanya berisi panduan akses/pemulihan; endpoint token setup lama
mengembalikan HTTP 410 dan tidak membuat akun.

Untuk menambahkan administrator lain secara sengaja melalui SSH:

```bash
sudo panelctl admin create --username admin_kedua --email admin@example.com
```

Password dibuat otomatis jika tidak menggunakan `--password-stdin`. Pembuatan
akun tambahan ini berbeda dengan bootstrap yang idempoten.

## Bantuan CLI

| Perintah | Kegunaan |
| --- | --- |
| `panelctl --help` | Semua perintah dan opsi global |
| `panelctl admin --help` | Bootstrap, create, list, reset password dan 2FA |
| `panelctl admin bootstrap --help` | Bootstrap serta output JSON untuk otomasi |
| `panelctl admin reset-password --help` | Opsi pemulihan password |
| `sudo panelctl doctor` | Diagnosis kesehatan panel |
| `panelctl backup --help` | Pembuatan, pemeriksaan, dan pemulihan backup |
| `panelctl upgrade --help` | Opsi upgrade |
| `panelctl reconcile --help` | Rekonsiliasi konfigurasi server |

`--help` tidak membuka atau memigrasikan database. Database produksi default:
`/var/lib/panel/panel.db`. CLI otomatis menjalankan operasi database produksi
sebagai identitas layanan `panel-api` ketika dipanggil melalui sudo, agar file
SQLite/WAL/SHM tidak menjadi milik root. Jangan menjalankan daemon `panel-api`
sebagai root. `--db-path` disediakan untuk database alternatif yang dikelola
operator; jangan memakainya kecuali mengetahui database yang dituju.

Otomasi lokal dapat memakai `sudo panelctl admin bootstrap --json`. Hasil pertama
memiliki `created: true` dan `credentials: { username, password }`; pemanggilan
berikutnya mengembalikan `created: false` dan `credentials: null`. Perlakukan output
JSON pertama sebagai secret, sama seperti output terminal installer.

## Verifikasi setelah upgrade

Perbarui installer, CLI, API, dan aset web dari release yang sama. Verifikasi pada
server: login dengan akun lama, ubah username/password melalui profil, pastikan
sesi browser lain diminta login ulang, dan periksa `panelctl admin --help`.
Pengujian lokal dengan SQLite/mock tidak membuktikan layanan, privilege, jaringan,
atau instalasi Ubuntu sebenarnya telah berhasil.
