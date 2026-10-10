# ADR 0034: Safe Archive Compression and Extraction

## Status
Accepted

## Konteks
Pada PRD Bagian 5.3 dan Task P5-T5, File Manager Nusa Panel membutuhkan fitur kompresi dan ekstraksi arsip berkas (ZIP dan TAR.GZ) yang aman dan tangguh terhadap berbagai vektor serangan arsip umum:
1. **Zip-Slip (Path Traversal)**:
   - Arsip yang dibuat secara jahat dapat memuat entri dengan path relatif seperti `../../../../etc/passwd` atau path absolut `/etc/shadow`.
   - Tanpa validasi ketat, ekstraktor arsip dapat menimpa berkas sensitif di luar direktori target.
2. **Symlink Escaping Jail**:
   - Arsip dapat berisi symlink yang mengarah ke target di luar direktori ekstraksi atau di luar home jail tenant (misalnya symlink `root_link -> /etc/passwd`).
   - Bila symlink ini diekstraksi tanpa sanitasi, pembacaan atau penulisan berikutnya dapat membocorkan atau merusak berkas sistem host.
3. **Zip-Bomb (Denial of Service / Disk & Memory Exhaustion)**:
   - Berkas arsip kecil (misal 42 KB) yang mengembang menjadi ukuran raksasa (misal puluhan GB atau petabyte) saat didekompresi, atau arsip dengan jutaan entri berkas kosong.
   - Menghabiskan memori RAM, disk space, atau inode server.
4. **Operasi I/O Skala Besar**:
   - Mengompresi atau mengekstrak ratusan ribu berkas membutuhkan waktu signifikan dan tidak boleh memblokir thread HTTP runtime axum.

## Keputusan Arsitektur

1. **Dependensi Eksternal**:
   - Menambahkan crate `flate2 = "1.0"` (dengan fitur bawaan `rust_backend` tanpa dependensi C eksternal) dan `tar = "0.4"` untuk dukungan TAR.GZ.
   - Menggunakan crate `zip = "2.4"` yang sudah terpasang untuk format ZIP.

2. **Arsitektur Proteksi Anti Zip-Slip & Symlink (`panel-fs::archive`)**:
   - Modul `ArchiveExtractor` dan `ArchiveCompressor` diintegrasikan dalam `panel-fs` dan dieksekusi di dalam konteks privilege tenant (`fs-worker`).
   - Setiap nama entri dinormalisasi menggunakan `panel_core::path::validate_relative_path` dan dipastikan terselesaikan strictly beneath extraction destination directory (`ConfinedFs::resolve_path`).
   - Seluruh entri berupa symlink divalidasi: jika symlink target mengarah ke luar root jail tenant atau relative link melompat keluar extraction directory, symlink tersebut ditolak keras atau dinetralisir sesuai `SymlinkPolicy`.
   - Mengabaikan hardlink yang melompat keluar root jail.

3. **Proteksi Anti Zip-Bomb**:
   - Batas ambang (*configurable guardrails*):
     - `MAX_COMPRESSION_RATIO`: 100x (jika rasio uncompressed bytes terhadap compressed bytes melebihi 100:1, ekstraksi dibatalkan).
     - `MAX_ENTRY_COUNT`: 50.000 file/entri per arsip.
     - `MAX_TOTAL_UNCOMPRESSED_BYTES`: 10 GB (atau dibatasi oleh sisa kuota disk tenant).
   - Validasi bertahap (*streaming counting*): Total byte yang diekstrak dihitung secara progresif saat streaming dekompresi berlangsung, bukan hanya membaca header metadata arsip yang dapat dipalsukan.

4. **Eksekusi Background Job**:
   - Permintaan kompresi dan ekstraksi diterima melalui endpoint REST:
     - `POST /api/v1/files/archive/compress`
     - `POST /api/v1/files/archive/extract`
   - Kedua endpoint mendaftarkan job asinkron ke `JobRegistry` dan mengembalikan `202 Accepted` bersama `job_id`.
   - Kemajuan (jumlah file yang diproses, total bytes) dilaporkan secara berkala ke database job dan dapat disimak melalui SSE `/api/v1/jobs/{id}/events`.

## Konsekuensi
- Mencegah 100% eksploitasi Zip-Slip, symlink traversal, dan penyerangan Zip-Bomb.
- Seluruh manipulasi arsip dijalankan dengan privilege ter-drop ke UID/GID tenant sehingga mematuhi isolasi kernel.
