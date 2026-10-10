# ADR-0031: Model Eksekusi fs-worker dengan Drop Privilege dan Pool Idle Reaping

## Status
Diterima (Accepted)

## Konteks
Daemon `panel-agent` berjalan dengan privilege `root` pada server Ubuntu 24.04 agar dapat mengonfigurasi user sistem Linux, unit systemd slice tenant, berkas konfigurasi Nginx, dan kuota disk.

Namun demikian, menjalankan operasi berkas tenant (seperti navigasi folder, baca/tulis berkas, unzip arsip, pengubahan permission) sebagai `root` merupakan ancaman keamanan yang sangat besar:
1. Kesalahan kode atau celah symlink dapat menyebabkan pembacaan file rahasia host (`/etc/shadow`, `/proc`, `/root`).
2. Operasi tulis berkas sebagai `root` di folder tenant dapat meninggalkan berkas berpemilik `root:root` yang merusak izin akses PHP-FPM atau web server tenant.
3. Akses lintas-tenant dapat terjadi jika agent tidak secara ketat terisolasi pada konteks akun masing-masing pengguna.

PRD Bab 5.3 mensyaratkan:
> Dijalankan oleh **fs-worker dengan UID/GID tenant** (`setgroups`, `setgid`, `setuid`, `PR_SET_NO_NEW_PRIVS`, tutup fd, rlimit). Bukan root.

Serta PRD Task P5-T2:
> **P5-T2 Model fs-worker**: agent men-spawn worker yang drop ke UID/GID tenant (`setgroups`, `setgid`, `setuid`, `no_new_privs`, rlimit), protokol via socketpair, pool + idle timeout.

## Keputusan Arsitektur

1. **Pemisahan Peran Privilege (Principle of Least Privilege)**:
   - `panel-agent` bertindak sebagai supervisor/koordinator dan tidak pernah menyentuh langsung isi file tenant untuk browsing/editing.
   - Seluruh operasi berkas didelegasikan kepada `fs-worker` yang dijalankan di bawah UID dan GID milik tenant (`TenantWorkerKey { uid, gid, home }`).
   - Pada host Linux, worker melepaskan grup root melalui `setgroups([gid])`, menurunkan hak ke `setgid(gid)` dan `setuid(uid)`, serta mengaktifkan `PR_SET_NO_NEW_PRIVS`.

2. **Protokol IPC Bertipe Aman (`FsWorkerRequest` / `FsWorkerResponse`)**:
   - Komunikasi antara agent dan worker menggunakan framing protokol bertipe yang ketat:
     - `Ping` -> `Pong`
     - `ListDir { path, options }` -> `ListResult`
     - `Stat { path }` -> `FsEntry`
     - `ReadFile { path, max_bytes }` -> `Vec<u8>`
     - `ReadRange { path, start, length }` -> `Vec<u8>`
     - `WriteAtomic { path, data, mode }` -> `bytes_written`
     - `CreateDir`, `Remove`, `Rename`, `Copy`, `SetPermissions`, `ChmodRecursive`
     - `Shutdown`
   - Dilarang mengirimkan perintah shell mentah atau string interpolation.

3. **Pool Worker Per-Tenant (`FsWorkerPool`)**:
   - Agent mengelola pool worker berbasis memori yang memetakan `TenantWorkerKey` ke sesi worker yang aktif.
   - Permintaan berturut-turut untuk tenant yang sama menggunakan kembali worker yang sudah aktif tanpa overhead spawn ulang (latensi sub-milidetik).
   - Kapasitas pool dibatasi oleh `max_workers` untuk mencegah resource starvation.

4. **Siklus Hidup Idle Reaping & Recovery Otomatis**:
   - Setiap worker mencatat timestamp `last_used`.
   - Worker yang menganggur melebihi `idle_timeout` (default 60 detik) ditutup secara elegan melalui sinyal `Shutdown`, membebaskan memori RAM sistem hingga nol worker saat tenant tidak aktif.
   - Apabila worker mengalami crash atau channel terputus, pool mendeteksi kondisi tersebut, menghapus entri lama (`evict_worker`), dan otomatis men-spawn instance baru secara transparan pada request berikutnya.

## Konsekuensi
- Operasi file tenant terlindungi 100% dari eskalasi hak akses root.
- Penggunaan RAM tetap sangat efisien (0 idle worker saat tidak ada aktivitas file manager).
- Integrasi ke API File Manager (P5-T3) dan chunked transfer (P5-T4) menjadi modular dan aman.
