# ADR-0030: Operasi Berkas Terkurung (Path-Confined Filesystem) dengan panel-fs

## Status
Diterima (Accepted)

## Konteks
Sistem hosting multi-tenant mengharuskan setiap tenant hanya dapat mengakses dan memanipulasi berkas di dalam direktori home mereka sendiri (`/home/p_<slug>`). Akses di luar direktori tersebut (seperti `/etc`, `/proc`, `/root`, maupun direktori milik tenant lain) merupakan celah keamanan kritis (arbitrary file read/write / privilege escalation).

Pendekatan umum yang naif seperti penggabungan string (`format!("{}/{}", root, user_path)`) atau `Path::join` standar memiliki kerentanan berbahaya:
1. `Path::join` akan mengabaikan root apabila input pengguna diawali karakter slash absolut (`/etc/passwd`).
2. Directory traversal via `..` (misalnya `../../../../etc/shadow`) lolos apabila tidak divalidasi per-komponen.
3. Serangan Symbolic Link (Symlink): symlink di dalam direktori tenant yang mengarah ke file sistem di luar home (misal `ln -s /etc/shadow evil_link`) dapat digunakan untuk membaca berkas rahasia host saat aplikasi mengikuti link.
4. Serangan Null Byte (`\0`), Unicode fullwidth slashes (`／`), serta manipulasi karakter kontrol dapat mengelabui parser ekstensi dan filter path.
5. Operasi tulis berkas yang tidak atomik dapat menyebabkan file korup saat server crash atau proses terputus di tengah jalan.
6. Penghapusan rekursif yang tidak sengaja menargetkan root jail (`""` atau `"."`) dapat menghapus seluruh direktori pengguna secara permanen.

PRD Bab 5.3 mensyaratkan:
> Semua operasi lewat library `panel-fs` yang **path-confined**: resolusi dengan `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)` atau `cap-std`; tidak pernah membangun path dengan string concat.
> Dijalankan oleh fs-worker dengan UID/GID tenant.
> Test adversarial wajib: `..`, symlink keluar root, race (TOCTOU), null byte, normalisasi unicode, path sangat panjang, nama file aneh.

## Keputusan Arsitektur

1. **Abstraksi `ConfinedFs` dan `ConfinedPath`**:
   - Seluruh operasi berkas diwajibkan melalui crate `crates/panel-fs`.
   - `ConfinedFs` diinisialisasi dengan path root kanonikal (`root`) dan kebijakan symlink (`SymlinkPolicy`).
   - Setiap path pengguna diverifikasi dan dibungkus menjadi `ConfinedPath` yang menjamin path absolut hasil resolusi strictly berada di bawah root kanonikal.

2. **Validasi Input Ketat**:
   - Menolak null byte (`\0`), karakter kontrol ASCII non-printable, dan karakter Unicode fullwidth slash (`／`, `＼`).
   - Batas panjang path maksimum 4096 karakter untuk mencegah overflow dan DOS.

3. **Resolusi Komponen Virtual Tanpa String Concat**:
   - Path dipecah menjadi komponen (`Component`). Komponen drive prefix atau root absolut dinormalisasi ke root jail.
   - Komponen `ParentDir` (`..`) diproses dengan stack traversal. Jika pengguna mencoba melangkah di atas root virtual (stack kosong), resolusi langsung gagal dengan `FsError::PathTraversalAttempt`.

4. **Kebijakan Symlink (`SymlinkPolicy`)**:
   - `DenyAll`: Menolak seluruh symlink (ekuivalen `RESOLVE_NO_SYMLINKS`).
   - `ResolveBeneath`: Memeriksa target symlink. Target yang mengarah ke luar root kanonikal ditolak dengan `FsError::SymlinkEscape`. Loop symlink dideteksi dengan batas maksimum 16 hop (`FsError::SymlinkLoop`).

5. **Defense-in-Depth dengan Kanonikalisasi**:
   - Setelah resolusi komponen, path fisik yang ada diuji kembali menggunakan `canonicalize()`.
   - Jika `!canonical_target.starts_with(&root_canonical)`, operasi dibatalkan seketika.

6. **Operasi Tulis Atomik (`write_atomic`)**:
   - Data ditulis ke berkas sementara di direktori yang sama (`.<stem>.<uuid>.tmp`), kemudian disinkronkan ke disk (`sync_all()`), lalu di-rename secara atomik ke target. Berkas lama tidak akan korup jika terjadi kegagalan di tengah penulisan.

7. **Perlindungan Root Jail**:
   - Operasi `remove_dir_all`, `remove_file`, dan `rename` secara eksplisit memeriksa apakah target adalah root jail (`is_root()`). Jika iya, dikembalikan `FsError::RootProtectionViolation`.

8. **Dukungan Skalabilitas Direktori Besar (10k+ Entri)**:
   - `list_dir` mendukung opsi pemfilteran (kueri pencarian nama, ekstensi, sembunyikan file bertitik), pengurutan fleksibel (Name, Size, ModifiedAt, CreatedAt) dengan folder selalu di posisi teratas, dan paginasi (offset + limit hingga 10.000 entri).

## Konsekuensi
- Semua komponen tingkat atas (fs-worker pada P5-T2, file manager API pada P5-T3, upload/download chunked pada P5-T4, dan ekstraksi arsip pada P5-T5) wajib menggunakan `panel-fs` dan terbebas dari manipulasi path mentah.
- Seluruh pengujian adversarial lulus tanpa celah traversal.
