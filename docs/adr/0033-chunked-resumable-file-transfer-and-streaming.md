# ADR 0033: Chunked Resumable File Transfer and Streaming

## Status
Accepted

## Konteks
Pada PRD Bagian 5.3 dan Task P5-T4, File Manager Nusa Panel membutuhkan transfer berkas skala besar yang handal dan aman:
1. **Upload chunked resumable**:
   - Mampu mengunggah file besar (hingga puluhan GB) secara bertahap dalam ukuran chunk terbatas (konstan memori O(chunk)).
   - Tahan gangguan jaringan: client dapat melakukan query offset (`Upload-Offset`) dan melanjutkan proses upload yang terputus tanpa mengulang dari awal.
   - Pengecekan kuota di muka (*pre-flight quota check*): total ukuran file divalidasi terhadap kuota disk tersisa milik tenant sebelum menerima payload pertama.
   - Finalisasi atomik: file chunk ditulis ke direktori staging `.panel-upload-tmp/<upload_id>.part` dan hanya di-rename atomik ke path tujuan saat seluruh byte telah diverifikasi lengkap.
2. **Download streaming dengan Range & ETag**:
   - Mendukung HTTP `Range` (`bytes=start-end`) untuk pemutaran media / resume download.
   - Menggunakan `ETag` (hash kombinasi mtime + file size) untuk HTTP caching dan respons 304 Not Modified.
   - Streaming memory-efficient tanpa memuat seluruh file ke RAM.
3. **Folder download sebagai Streaming ZIP**:
   - Pengguna dapat mengunduh seluruh isi folder sebagai berkas `.zip`.
   - Menghindari pembuatan berkas arsip zip sementara raksasa di disk server (yang bisa menghabiskan disk quota tenant atau disk host).
   - Seluruh entri dikompresi on-the-fly secara aman dan di-stream langsung ke socket respons HTTP.

## Keputusan Arsitektur

1. **Dependensi Eksternal**:
   - Menambahkan crate `zip = "2.4"` (dengan fitur `deflate`) untuk pembuatan stream ZIP standar dan performan tanpa *unsafe code* dalam codebase aplikasi Nusa Panel.

2. **Protokol Chunked Upload**:
   - `POST /api/v1/files/upload/init`:
     - Input: `UploadInitPayload { target_path, file_name, total_size, tenant }`.
     - Validasi: Resolusi tenant Linux (`p_<user>`), verifikasi batas ukuran paket langganan dan sisa kuota disk.
     - State: Menyimpan state metadata di `.panel-upload-tmp/<upload_id>.json` di dalam home tenant.
     - Output: `UploadInitResponse { upload_id, current_offset: 0, total_size }`.
   - `GET /api/v1/files/upload/{upload_id}`:
     - Mengembalikan offset saat ini (`UploadStatusResponse { upload_id, current_offset, total_size, completed }`) untuk mendukung resume saat koneksi terputus.
   - `POST /api/v1/files/upload/{upload_id}/chunk`:
     - Menerima chunk biner dengan header atau query `offset`.
     - Jika `offset != current_offset`, server mengembalikan `409 Conflict` bersama offset valid.
     - Menulis chunk via `fs-worker` ke `.panel-upload-tmp/<upload_id>.part`.
     - Memperbarui `current_offset`.
     - Jika `current_offset == total_size`, lakukan `atomic finalize`: rename `.part` ke `target_path`, hapus metadata, dan rekam audit log `file.upload`.

3. **Single File Download**:
   - Endpoint `GET /api/v1/files/download?path=<path>&tenant=<tenant>`.
   - Parsing header `Range: bytes=<start>-<end>`.
   - Jika range valid: respons `206 Partial Content` dengan `Content-Range: bytes <start>-<end>/<total>` dan streaming buffer terbatas.
   - Jika `If-None-Match: <etag>` cocok: respons `304 Not Modified`.
   - Respon standar `200 OK` dengan `Accept-Ranges: bytes` dan `ETag`.

4. **Folder Streaming ZIP**:
   - Endpoint `GET /api/v1/files/download-zip?path=<path>&tenant=<tenant>`.
   - Menggunakan `panel-fs::ConfinedFs` untuk menelusuri folder secara rekursif (terikat di root tenant jail).
   - Mengompresi entri ke dalam stream ZIP secara on-the-fly dengan header `Content-Type: application/zip` dan `Content-Disposition: attachment; filename="<folder_name>.zip"`.

## Konsekuensi & Keamanan
- Penggunaan memori backend tetap konstan dan terikat (*O(chunk_size)*), bahkan untuk berkas berukuran 10+ GB.
- Berkas yang gagal atau dibatalkan berada di `.panel-upload-tmp/` di dalam kuota disk tenant itu sendiri sehingga tidak terjadi kebocoran kuota ke ruang sistem.
- Proteksi symlink dan path traversal diwarisi langsung dari `panel-fs`, menjamin stream ZIP maupun upload path tidak pernah keluar dari jail tenant.
