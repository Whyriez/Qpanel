# ADR-0035: File Permission Editor, Recursive Differentiation, and Ownership Restrictions

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.3 dan Task **P5-T6**, file manager memerlukan fungsionalitas manajemen hak akses berkas (POSIX DAC permissions):
1. Pengguna harus dapat melihat dan mengubah permission mode bit berkas/folder melalui grid centang 3x3 (Owner, Group, Others) yang tersinkronisasi dua arah dengan input angka oktal (`0644`, `0755`, dll).
2. Perubahan rekursif wajib membedakan secara tegas antara mode berkas biasa (*files*) dan mode direktori (*folders*) — misalnya `0644` untuk file dan `0755` untuk folder — demi menjaga eksekusi direktori (traversal) tetap berfungsi tanpa memberikan hak eksekusi sembarangan pada skrip berkas PHP/HTML.
3. Permintaan hak akses yang terlalu terbuka (`0777` atau *world-writable*) harus menyertakan peringatan keamanan eksplisit kepada pengguna.
4. Pengubahan kepemilikan berkas (*chown*) wajib dibatasi: penyewa (*tenants*) tidak diperbolehkan mengubah pemilik berkas ke pengguna sistem lain atau root untuk mencegah *privilege escalation* dan pelanggaran batas isolasi tenant.

## Keputusan Arsitektur
1. **Modul Permission POSIX di `panel-fs`**:
   - Menambahkan struktur `PermissionGrid` (representasi boolean 3x3 untuk Owner r/w/x, Group r/w/x, dan Others r/w/x) dengan konversi bolak-balik ke mode oktal POSIX numerik (`u32`).
   - Menyediakan fungsi parsing oktal aman `parse_octal_mode` dan formatting `format_octal_mode`.
   - Mengimplementasikan pendeteksian peringatan keamanan `check_mode_security_warning` bila bit `0777` atau bit *world-writable* (`S_IWOTH` / `mode & 0o002 != 0`) terdeteksi.
2. **Pembedaan Mode Rekursif di `ConfinedFs`**:
   - Method `chmod_recursive` menerima `file_mode` dan `dir_mode` secara terpisah.
   - Melakukan penelusuran hierarki direktori secara aman (mengabaikan symlink) dan menerapkan `dir_mode` untuk direktori serta `file_mode` untuk berkas reguler, mengembalikan jumlah total entri yang terpengaruh.
3. **Penegakan Batasan Kepemilikan (*Chown Boundaries*)**:
   - Di `ConfinedFs::chown`: Menolak eksekusi jika pemanggil bukan admin dan mencoba mengubah UID ke pengguna lain selain UID dirinya sendiri (`FsError::PermissionDenied`).
   - Di level worker (`fs-worker`), proses worker berjalan dengan dropped privileges (UID/GID tenant), sehingga kernel POSIX secara inheren menolak syscall `chown` ke user lain (`EPERM`).
   - Di REST API `POST /api/v1/files/chown`: Akses ditolak langsung dengan HTTP `403 Forbidden` jika pengguna bukan administrator.
4. **REST Endpoints di `panel-api`**:
   - `POST /api/v1/files/chmod`: Mengubah izin satu berkas/folder, mengembalikan metadata mode dan warning jika `0777`.
   - `POST /api/v1/files/chmod-recursive`: Menerapkan mode terpisah untuk file dan direktori secara rekursif.
   - `POST /api/v1/files/chown`: Endpoint kepemilikan berbatas ketat dengan audit log.
5. **Komponen UI Modular (`FilePermissionModal.tsx`)**:
   - Menampilkan detail file, pemilik, dan grup.
   - Grid centang interaktif 3x3 yang terikat dua arah dengan input oktal.
   - Tombol preset cepat: `0644 (Berkas)`, `0755 (Folder)`, `0600 (Privat)`.
   - Banner peringatan dinamis (*Security Warning*) ketika mode `777` atau izin tulis publik aktif.
   - Opsi rekursif khusus folder dengan form input terpisah untuk berkas dan direktori.

## Konsekuensi
- **Positif**:
  - Webmaster terlindungi dari kecerobohan menerapkan `0644` ke folder (yang dapat merusak akses webserver) atau `0777` secara tidak sengaja tanpa peringatan.
  - Isolasi multi-tenant tetap kokoh: tenant tidak dapat mengeksekusi chown untuk mengambil alih file user lain atau root.
  - Komponen UI siap dipakai dan teruji secara komprehensif.
- **Batasan**:
  - Pada sistem pengujian non-Linux (misalnya pengembang di Windows), syscall chown di-mock tanpa mengubah hak akses OS host, sementara aturan keamanan logika dan pengujian isolasi tetap ditegakkan 100%.
