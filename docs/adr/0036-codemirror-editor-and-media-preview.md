# ADR 0036: CodeMirror 6 Editor dan Multi-Format File Preview

## Status
Accepted

## Konteks
Pada Phase 5 (File Manager Custom), task **P5-T7** menuntut kapabilitas pengeditan teks langsung di browser dan pratinjau (preview) berbagai jenis berkas media tanpa harus mengunduh berkas secara terpisah:
1. Pengeditan berkas teks/kode dengan penomoran baris, tema konsisten (dark), keyboard shortcuts (`Ctrl+S`), dan status bar (baris, karakter, ukuran, encoding).
2. Batas ukuran berkas pengeditan: berkas teks berukuran >5 MB harus dibuka dalam mode hanya-baca (*read-only*) untuk melindungi kinerja memori dan kestabilan tab peramban.
3. Deteksi konflik konkurensi (*optimistic concurrency control*): jika berkas di server telah dimodifikasi oleh proses atau sesi lain sejak dibuka (berdasarkan `mtime` dan `ETag`), simpanan harus ditolak (`409 Conflict`) dengan opsi muat ulang (*reload*) atau simpan paksa (*force overwrite*).
4. Pratinjau multi-tipe (gambar dengan zoom/pan/rotate, dokumen PDF dengan embedded object viewer, media video/audio via streaming HTML5 controls, dan fallback interaktif untuk file biner).
5. Kepatuhan keamanan: API dan ConfinedFs tetap membatasi akses строго di dalam ruang direktori pengguna (*root jail*) dan menolak percobaan *path traversal*.

## Keputusan Arsitektur

### 1. Engine Editor CodeMirror 6
- Dipilih `@uiw/react-codemirror` berbasis CodeMirror 6 modular dengan extensi dasar (`basicSetup`) yang menyertakan line numbers, bracket matching, history/undo-redo, auto-closing brackets, dan line folding.
- Menyediakan status bar informatif di bagian bawah: jumlah baris, karakter, ukuran berkas (KB), encoding UTF-8, dan status mode (Edit vs Read-Only).
- Batas keras 5 MB: jika ukuran berkas > 5 MB, editor otomatis masuk mode read-only dan menampilkan banner peringatan edukatif.

### 2. Deteksi Konflik Simpanan (Optimistic Concurrency Control)
- Endpoint `GET /api/v1/files/content`:
  - Mengembalikan isi berkas teks (sampai batas 5 MB), `modified_at` (epoch seconds), dan `ETag` (hash `mtime-size`).
- Endpoint `PUT /api/v1/files/content`:
  - Menerima `path`, `content`, `expected_mtime`, `expected_etag`, dan flag `force: bool`.
  - Jika `force` tidak aktif dan `expected_mtime` atau `expected_etag` tidak cocok dengan kondisi berkas terkini di disk, API mengembalikan `409 Conflict` berserta detail `current_mtime` dan `current_etag`.
  - Frontend menampilkan banner peringatan interaktif dengan 2 opsi: "Muat Ulang Berkas" atau "Simpan Paksa (Overwrite)".
  - Penyimpanan dilakukan secara atomik via `panel-agent` `write_atomic` (penulisan ke berkas sementara lalu rename atomik) dengan audit logging.

### 3. Komponen Pratinjau Terpadu (`FilePreviewModal`)
- Deteksi otomatis jenis file berdasarkan ekstensi atau MIME type:
  - **Gambar** (`.png`, `.jpg`, `.svg`, `.webp`, dll): Ditampilkan dengan fitur zoom in, zoom out, reset, dan rotasi 90 derajat searah jarum jam, serta pola background checkerboard untuk transparansi.
  - **Dokumen PDF** (`.pdf`): Ditampilkan via tag `<object data="...">` dengan fallback tombol unduh jika plugin PDF browser tidak tersedia.
  - **Video & Audio** (`.mp4`, `.webm`, `.mp3`, `.wav`, dll): Menggunakan tag `<video controls>` dan `<audio controls>` standar HTML5 yang memanfaatkan dukungan range streaming dari `/api/v1/files/download`.
  - **Teks/Kode**: Mode pratinjau CodeMirror hanya-baca dengan tombol langsung "Buka di Editor".
  - **Biner/Tidak Didukung**: Kartu fallback bersih dengan informasi ukuran berkas dan tombol unduh.

## Konsekuensi & Mitigasi
- **Kinerja Browser**: Berkas di atas 5 MB dicegah masuk ke mode edit CodeMirror agar tidak membekukan JavaScript thread.
- **Keamanan Sandboxing**: Seluruh pembacaan dan penulisan berkas dilewatkan melalui `ConfinedFs` dan `resolve_confined_path` di `panel-agent` worker terisolasi tenant, sehingga tidak ada pembocoran path di luar `/home/p_<user>`.
- **Integritas Data**: Mengeliminasi bahaya *silent overwrite* saat ada dua tab/pengguna yang mengedit file yang sama secara bersamaan.
