# Backup dan migrasi QonPanel

Fitur **Pengaturan server > Backup dan migrasi** memindahkan data yang dikelola
panel ke instalasi QonPanel baru. Ini berbeda dari snapshot database/config pada
menu Keandalan, dan tidak mengkloning seluruh sistem operasi.

## Dari server lama

1. Buka halaman Backup dan migrasi sebagai administrator.
2. Masukkan passphrase minimal 16 byte, ulangi, lalu pilih **Buat backup penuh**.
3. Konfirmasikan jeda layanan. Worker terpisah menghentikan writer yang dikelola
   panel selama snapshot, memeriksa SQLite/WAL, lalu mengaktifkan layanan semula.
4. Tunggu status selesai dan unduh arsip `.qpb`. Simpan passphrase secara terpisah.
   Jangan membuang server lama sebelum hasil restore diverifikasi.

Passphrase dikirim melalui sesi admin/IPC lokal, disimpan hanya pada handoff root
0600, lalu dibaca dan dihapus worker. Passphrase tidak dimasukkan ke argv, audit,
status, localStorage atau sessionStorage. Gunakan HTTPS untuk akses panel publik.
Arsip menggunakan enkripsi simetris AES-256 dan pemeriksaan integritas GnuPG;
passphrase diberikan melalui file descriptor dan cache simetris dimatikan.
Lihat [manual GnuPG](https://gnupg.org/documentation/manuals/gnupg/Operational-GPG-Commands.html)
dan [opsi passphrase](https://gnupg.org/documentation/manuals/gnupg/GPG-Esoteric-Options.html).

## Pada server baru

1. Pasang paket QonPanel yang memuat fitur ini, dengan versi yang sama atau lebih
   baru daripada sumber. Gunakan Ubuntu 24.04 dan arsitektur CPU yang sama.
2. Masuk memakai akun admin instalasi baru. Jangan membuat tenant/situs/database,
   memasang runtime, atau menaruh data layanan di server tujuan terlebih dahulu.
3. Pilih arsip `.qpb`, unggah, masukkan passphrase sumber dan **Periksa arsip**.
4. Tinjau hostname, versi, jumlah tenant, kapasitas dan inventaris paket. Bila
   perlu, isikan IPv4 publik baru. Pemeriksaan berlaku 30 menit.
5. Cocokkan SHA-256 dengan arsip sumber, lalu sahkan hash satu kali melalui SSH
   sebagai pemilik server tujuan (berlaku 24 jam dan berakhir setelah restore sukses):

   ```bash
   sudo python3 /usr/local/libexec/qonpanel-server-backup --approve SHA256_ARSIP_SUMBER
   ```

   Hash juga dapat diberikan saat instalasi baru melalui opsi
   `--restore-digest SHA256_ARSIP_SUMBER`. Setelah hash cocok, tombol restore aktif.
6. Masukkan ulang passphrase, pilih **Restore ke server ini**, konfirmasikan
   dampaknya dan masukkan password administrator server tujuan.
7. Setelah selesai, masuk menggunakan akun/password/2FA dari server lama.
   Seluruh sesi lama dibatalkan; pekerjaan queued/running dari snapshot tidak
   diputar ulang otomatis. Arahkan DNS/domain serta aturan firewall provider
   ke IP/port server baru dan verifikasi website, aplikasi, database dan email.

Restore memerlukan ruang staging dan snapshot keselamatan, bukan hanya kapasitas
data akhir. Arsip maksimum 512 GiB. Snapshot file besar menjeda layanan selama
penyalinan; gunakan jadwal pemeliharaan. Paket PHP, database dan layanan terkait
dibawa sebagai arsip paket `.deb` dari server sumber (dpkg-repack), sehingga
versi aplikasi lama tidak harus masih tersedia di repository. APT tetap memerlukan
dependensi OS yang kompatibel. Worker tidak diam-diam mengganti versi database.
Setelah restore, paket aplikasi tersebut di-hold untuk menjaga versi dan conffile
hasil repack; upgrade OS/runtime dilakukan terpisah setelah peninjauan konfigurasi.
Update versi QonPanel tetap memakai updater panel seperti biasa.
Runtime Node/Python yang dikelola `/opt/panel` ikut arsip. GnuPG/Python/dpkg-repack tersedia
pada instalasi baru; alat backup yang belum ada dapat disiapkan worker bila belum ada pada instalasi lama.

## Cakupan

- Database panel, operations dan analytics; akun, password hash, 2FA, metadata
  situs/domain, konfigurasi dan master key enkripsi `/etc/panel`.
- Home tenant yang tercatat di panel, UID/GID, password hash dan shell Linux;
  SSH keys/drop-in milik panel serta kebijakan SFTP. SSH host keys/root account,
  konfigurasi jaringan OS dan akses SSH administrator tujuan dipertahankan.
- Konfigurasi Nginx, PHP, runtime Node/Python, unit/slice tenant, sertifikat/ACME.
- Data/config lokal MariaDB/PostgreSQL, termasuk akun database fisiknya;
  data Redis lokal bila ada. Tidak menggabungkan direktori database tujuan.
- Mailbox, konfigurasi email, kunci DKIM, serta arsip antrean Postfix terkelola. Antrean keluar disimpan untuk pemulihan
  operator, tetapi tidak dikirim ulang otomatis agar email yang sudah terkirim
  setelah snapshot tidak terkirim dua kali.
- Cron, konfigurasi/credential connector Cloudflare terkelola, serta inventaris
  paket dan repositori APT yang diperlukan untuk pemasangan ulang layanan.

Binary API/agent/CLI/frontend, installed-release tujuan, identitas mesin,
provider VM, disk/mount/quota filesystem, firewall provider dan data layanan
eksternal tidak ditimpa dari backup. Data/custom service di luar lokasi terkelola
tidak termasuk. Symlink keluar dari cakupan, identitas tidak sesuai, path traversal,
device, executable setuid/setgid dan format asing ditolak. Arsip harus berasal
dari server yang Anda percaya: restore mengembalikan konfigurasi layanan root.
Gunakan passphrase kuat; ini bukan mekanisme untuk mengimpor arsip pihak lain.

ACL/xattr dan ownership disimpan melalui GNU tar. Tenant dibuat dengan UID/GID
semula; akun sistem memakai nama pada server tujuan. Lihat
[manual GNU tar](https://www.gnu.org/software/tar/manual/tar.html).
Snapshot backup lama dan berkas transfer tidak dimasukkan kembali secara rekursif.

## Jika terputus atau gagal

Worker berjalan di `qonpanel-server-backup.service`, terpisah dari API/agent.
Penulisan API dan klaim job dijeda, request aktif didrain, lalu lock SQLite
menahan claim/write yang sudah berlomba sampai writer berhenti.
Status/inspection/snapshot keselamatan berada di `/var/lib/qonpanel-migration`
di luar database yang diganti. API tidak menjalankan root dan hanya mengirim
operasi bertipe dengan UUID, bukan path/perintah bebas.

Sebelum mengganti data, worker membuat snapshot server tujuan dan journal
transaksi. Jika pemasangan, validasi atau health check gagal, worker mencoba
rollback dan mengaktifkan layanan semula. Kegagalan rollback tidak dianggap sukses.
Proses terputus tidak diputar ulang secara otomatis.

Jika mati listrik/reboot memutus restore dan panel belum kembali, jalankan dari
console/SSH server tujuan:

```bash
sudo python3 /usr/local/libexec/qonpanel-server-backup --recover
sudo journalctl -u qonpanel-server-backup.service --no-pager -n 100
```

Periksa hasil pemulihan sebelum mencoba lagi. Jangan menghapus `journal.json`
atau `safety.tar` sendiri. Stop server lama setelah cutover agar cron/worker tidak
berjalan ganda. Bukti konektivitas lama dibersihkan karena tidak membuktikan akses
server baru. Periksa kembali endpoint IP, DNS, sertifikat IP literal dan layanan
publik. Mode restore ini tidak menggantikan acceptance migrasi Ubuntu nyata.

Paket aplikasi direkonstruksi dari instalasi yang ada, termasuk conffile terpasang.
Karena conffile hasil repack memiliki semantik upgrade yang berbeda, hold paket
mencegah APT menggantinya tanpa peninjauan. Setelah menyiapkan perubahan runtime/OS,
operator dapat mengelola hold melalui `apt-mark`; ini tidak memerlukan instalasi
ulang panel. Rujukan: [dpkg-repack Ubuntu 24.04](https://manpages.ubuntu.com/manpages/noble/man1/dpkg-repack.1.html).

Pengesahan root dibutuhkan karena arsip penuh mengembalikan paket `.deb` dan
konfigurasi layanan yang dapat berjalan sebagai root. Sesuai batas agent pada
AGENTS.md butir 3 dan 4, API/UID panel-api tidak boleh memberi dirinya kewenangan
menjalankan isi arsip root. API hanya dapat mengunggah/memeriksa dan meminta
restore untuk hash yang sudah disahkan melalui jalur root. File approval dimiliki
root (0600), tidak dapat dibuat/diubah melalui API, dan worker memeriksanya kembali
sebelum memasang paket atau mengganti data. Konfirmasi/password admin tetap
memverifikasi sesi UI; keduanya tidak menggantikan pengesahan pemilik host.
