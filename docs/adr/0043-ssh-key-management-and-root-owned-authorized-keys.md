# ADR 0043: Manajemen Kunci SSH Tenant & Berkas Authorized Keys Root-Owned

## Status

Accepted

## Konteks

Sesuai spesifikasi `PRD (1).md` baris 795 dan requirement Phase 6 (P6-T5):
> *"Manajemen SSH key: tambah/hapus public key per tenant (file dikelola panel di lokasi root-owned), validasi format, fingerprint di UI."*

Pada sistem hosting multi-tenant konvensional, berkas otorisasi SSH biasanya disimpan di direktori home pengguna (contoh: `~/.ssh/authorized_keys`). Pendekatan ini memiliki kelemahan keamanan serius:
1. **Manipulasi Langsung oleh Tenant**: Pengguna atau skrip tenant dapat memodifikasi, menambah, atau menghapus entri tanpa terpantau oleh kontrol panel atau sistem audit.
2. **Bypass Kebijakan Akses**: Tenant dapat menambahkan opsi berbahaya pada berkas authorized_keys atau mengubah permission berkas yang dapat mengakibatkan lockout maupun eskalasi akses.
3. **Integritas Format**: Masukan public key mentah tanpa validasi struktur dapat mengandung injeksi karakter baris baru (*newline injection*) yang merusak entri lain atau menyisipkan baris `command="..."` paksa.

Oleh karena itu, sistem Nusa Panel harus mengelola kunci publik SSH di lokasi khusus yang sepenuhnya dimiliki oleh root (*root-owned*), memvalidasi format OpenSSH secara deterministik, menghitung fingerprint SHA256/MD5, dan menyediakan antarmuka pengguna yang aman dan intuitif.

## Keputusan Arsitektur

### 1. Penyimpanan Berkas Authorized Keys Root-Owned

Berkas otorisasi SSH tenant tidak ditempatkan di dalam direktori home tenant (`/home/p_<slug>/.ssh/`), melainkan disimpan pada path sistem khusus:
- **Lokasi Berkas**: `/etc/panel/ssh/authorized_keys/%u` (atau `/etc/panel/ssh/authorized_keys/p_<username>`).
- **Kepemilikan & Hak Akses**: `root:root` dengan permission `0644`.
- **Integrasi OpenSSH Daemon**: Konfigurasi `sshd_config` sistem diarahkan menggunakan direktif:
  ```text
  AuthorizedKeysFile /etc/panel/ssh/authorized_keys/%u
  ```
- **Proteksi Anti-Tampering**: Tenant Linux (`p_<username>`) tidak memiliki izin menulis (*write permission*) ke direktori `/etc/panel/ssh/authorized_keys/`, sehingga seluruh penambahan atau pencabutan kunci publik harus melalui API Nusa Panel.

### 2. Validasi Format RFC 4251 Wire Format & Pencegahan Injeksi

Modul `panel_core::ssh` mengimplementasikan fungsi verifikasi deterministik `parse_ssh_public_key(&str)`:
1. **Sanitasi Baris**: Memastikan kunci hanya terdiri dari satu baris tunggal; menolak masukan yang mengandung `\n` atau `\r` untuk mencegah serangan *newline injection*.
2. **Dekode Wire Format RFC 4251**: Membaca payload base64 dan memeriksa string tipe kunci berawalan panjang (*length-prefixed string*). Tipe wire harus cocok secara identik dengan identifier tipe kunci di awalan baris (`ssh-ed25519`, `ssh-rsa`, `ecdsa-sha2-nistp256/384/521`, atau `sk-ssh-*`).
3. **Standar Kriptografi**:
   - `ssh-ed25519`: Tipe modern yang direkomendasikan secara default.
   - `ssh-rsa`: Menolak kunci RSA dengan panjang di bawah 2048-bit untuk mencegah kelemahan kriptografi usang.
   - Menolak algoritma yang sudah usang dan tidak aman seperti DSA (`ssh-dss`).
4. **Kalkulasi Fingerprint**:
   - `SHA256`: Menggunakan unpadded Base64 dari ringkasan SHA-256 wire bytes (format OpenSSH standar: `SHA256:<hash>`).
   - `MD5`: Menggunakan format heksadesimal berpemisah titik dua (format legacy `md5:xx:xx:...`).

### 3. Sinkronisasi Atomik di `panel-agent` (`SshManager`)

Komponen `SshManager` di `panel-agent` menangani permintaan RPC `SshKeysSyncRequest`:
1. Menerima daftar kunci publik yang telah tervalidasi dan nama pengguna Linux tenant (`p_<user>`).
2. Menolak nama pengguna yang tidak valid atau terindikasi path traversal (`..`, `/`).
3. Menulis seluruh kunci ke berkas sementara (`.tmp.<random>`) di dalam direktori `/etc/panel/ssh/authorized_keys/`.
4. Mengubah hak akses berkas sementara ke mode `0644` (root-owned).
5. Melakukan operasi `fs::rename` atomik ke nama berkas target (`p_<user>`).
6. Jika daftar kunci kosong, berkas dihapus secara aman untuk mencabut seluruh akses SSH tenant.

### 4. Skema Basis Data & Isolasi Multi-Tenant

Tabel `ssh_keys` di `panel-db`:
- Kolom: `id`, `user_id` (foreign key ke `users.id` dengan `ON DELETE CASCADE`), `name`, `key_type`, `fingerprint_sha256`, `fingerprint_md5`, `public_key`, `created_at`.
- Constraint unik: `UNIQUE(user_id, fingerprint_sha256)` mencegah duplikasi kunci pada akun yang sama.
- Kontrol Akses (RBAC):
  - Tenant biasa hanya dapat melihat, menambah, dan menghapus kunci publik miliknya sendiri (`user_id` terikat pada sesi login).
  - Admin panel dapat melihat atau mengelola kunci publik milik tenant tertentu melalui parameter `?tenant=p_<slug>`.

### 5. Jejak Audit (`AuditWriter`)

Setiap tindakan mutasi dicatat secara persisten:
- Penambahan kunci: Aksi `ssh_key.create` dengan detail ID kunci, tipe kunci, fingerprint SHA256, dan IP klien.
- Penghapusan kunci: Aksi `ssh_key.delete` dengan detail ID kunci, fingerprint, dan IP klien.

### 6. Desain Antarmuka Pengguna (`SshKeyManager`)

Komponen `SshKeyManager` terintegrasi langsung ke dalam halaman `SecurityProfilePage.tsx`:
- Menampilkan daftar kunci aktif dengan badge tipe algoritma (`ED25519`, `RSA`), tanggal pendaftaran, dan fingerprint SHA256 yang dapat disalin dengan satu klik.
- *Live format validation* saat pengguna menempelkan (*paste*) public key di dialog penambahan kunci. Status validasi menampilkan panjang bit, komentar asli, serta mendeteksi kesalahan format secara langsung sebelum formulir dikirim.
- Panduan pembuatan kunci lokal menggunakan perintah `ssh-keygen -t ed25519 -C "developer@example.com"` yang dapat disalin langsung.
- Dialog konfirmasi sebelum penghapusan kunci untuk mencegah pencabutan akses secara tidak sengaja.

## Konsekuensi

### Positif
- **Integritas Lingkungan Server**: Tenant tidak dapat menambahkan backdoor atau memodifikasi authorized_keys di luar panel kendali.
- **Standar Keamanan Tinggi**: RSA < 2048-bit dan DSA ditolak sejak lapisan API, mencegah eksploitasi kunci lemah.
- **Transparansi & Akuntabilitas**: Seluruh penambahan dan penghapusan kunci tercatat dalam audit log yang dapat ditelusuri.
- **Pengalaman Pengguna Ramah**: Validasi live feedback memudahkan tenant mengetahui apakah kunci publik yang disalin sudah benar dan lengkap.

### Pertimbangan Operasional
- Daemon OpenSSH sistem operasi harus dikonfigurasi dengan `AuthorizedKeysFile /etc/panel/ssh/authorized_keys/%u`.
- Direktori `/etc/panel/ssh/authorized_keys/` harus dipastikan dibuat saat inisialisasi panel dengan izin `0755 root:root`.
