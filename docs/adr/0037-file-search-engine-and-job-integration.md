# ADR 0037: Mesin Pencarian Berkas & Konten dengan Batasan Aman dan Integrasi Job System

## Status
Accepted

## Konteks
Pada Phase 5 (File Manager Custom), task **P5-T8** menuntut kapabilitas pencarian berkas dan direktori yang aman, cepat, dan komprehensif bagi pengguna panel:
1. Pencarian berdasarkan **nama berkas** (case-sensitive atau case-insensitive) dengan filter ekstensi opsional.
2. Pencarian berbasis **isi teks/kode** (grep-like) di dalam berkas dengan cuplikan cuplikan baris kode yang cocok (nomor baris dan teks baris hingga batas panjang baris).
3. Batas keamanan dan sumber daya (*guardrails*):
   - Timeout default (15 detik) untuk mencegah operasi pencarian memonopoli CPU server.
   - Batas maksimal jumlah hasil (default 100 entri, dapat dikonfigurasi).
   - Pengabaian berkas biner secara otomatis (deteksi null byte pada 512 byte pertama).
   - Batas ukuran berkas untuk pencarian isi (maksimal 5 MB) agar memori dan I/O tidak terbebani.
   - Pengabaian direktori internal sistem panel (`.panel-trash` dan `.panel-upload-tmp`).
   - Penolakan ketat terhadap *path traversal* (`../`, dll) di luar kurungan `panel-fs` (*ConfinedFs*).
4. Opsi eksekusi:
   - Mode **sinkron** untuk pencarian cepat di folder spesifik / subdirektori berukuran wajar.
   - Mode **asinkron / background job** (`as_job: true`) via `job_runner` (`files.search`) untuk pencarian di pohon direktori yang dalam atau berskala besar.

## Keputusan Arsitektur

### 1. Mesin Pencarian di Crate `panel-fs`
- Diimplementasikan dalam modul `crates/panel-fs/src/search.rs`:
  - Fungsi `search_confined(&ConfinedFs, start_path, &SearchOptions) -> Result<SearchReport, FsError>`.
  - Menggunakan struktur antrean iteratif (`VecDeque`) untuk traversal direktori sehingga aman dari bahaya stack overflow pada kedalaman folder yang sangat dalam.
  - Setiap iterasi memeriksa durasi `Instant::now()` terhadap batas waktu `timeout`. Jika terlampaui, proses traversal berhenti seketika dan memberi tanda `timed_out: true`.
  - Mengabaikan direktori internal `.panel-trash` dan `.panel-upload-tmp`.
  - Fungsi pendeteksi berkas biner `is_binary_file`: membaca 512 byte awal dan memeriksa keberadaan `\0` (null byte). Jika terdeteksi biner, pencarian isi teks dilewati.
  - Untuk berkas teks berukuran $\le$ 5 MB, baris dibaca secara streaming via `BufReader` dengan batasan maksimal cuplikan per berkas (`MAX_MATCHES_PER_FILE = 5`) dan pemangkasan panjang baris (`MAX_SNIPPET_LENGTH = 160`).
  - Laporan pencarian (`SearchReport`) merangkum `items`, `total_scanned`, `limit_reached`, `timed_out`, dan `duration_ms`.

### 2. Protokol IPC Agent & RPC
- Crate `panel-proto` menambahkan varian:
  - `RequestPayload::FsSearch(FsSearchRequest)`
  - `ResponsePayload::FsSearch(FsSearchResponse)`
  - DTO terstruktur `FsSearchItemDto` dan `ContentMatchDto`.
- Crate `panel-agent` menangani permintaan pencarian melalui worker confined filesystem dan mencatat peristiwa ke audit log dengan event `fs.search`.

### 3. API Endpoint & Integrasi Background Job
- Endpoint `POST /api/v1/files/search` di `panel-api`:
  - Menerima payload: `path`, `query`, `search_content`, `case_sensitive`, `extension`, `max_results`, `timeout_seconds`, dan `as_job: bool`.
  - Validasi: string `query` wajib tidak kosong, dan `path` divalidasi keamanannya terhadap path traversal (mengembalikan `403 Forbidden` jika melanggar kurungan).
  - Jika `as_job: false` (default): Menjalankan pencarian langsung secara sinkron melalui `agent_client.fs_search` dan mengembalikan respons `200 OK` dengan daftar hasil.
  - Jika `as_job: true`: Meng-enqueue job baru bertipe `"files.search"` ke dalam `JobRunner`, mengembalikan `202 Accepted` bersama `job_id`, dan mendaftarkan `FileSearchHandler` di registry sistem job.

### 4. Komponen Antarmuka (`FileSearchModal`)
- Komponen modal interaktif di `web/src/components/files/FileSearchModal.tsx`:
  - Input query pencarian yang responsif dengan tombol submit "Cari" dan keyboard shortcut.
  - Panel filter *collapsible*: opsi pencarian isi file (toggle switch), sensitivitas huruf besar/kecil (case sensitive), dan pembatasan ekstensi berkas.
  - Tampilan hasil bersih dengan ikon tipe berkas, path lengkap, ukuran berkas, tombol aksi "Buka di Editor" dan "Lihat Preview".
  - Untuk hasil pencarian konten, cuplikan baris kode yang cocok ditampilkan dengan nomor baris dalam blok font monospace.
  - Banner peringatan edukatif bila hasil pencarian terpotong oleh batasan maksimum (`limitReached`) atau batas waktu habis (`timedOut`).
  - Ringkasan footer yang menampilkan jumlah berkas ditemukan, total entri dipindai, dan durasi eksekusi dalam milidetik.

## Konsekuensi & Mitigasi
- **Keamanan & Isolasi**: Direktori akar dipaksa selalu terkungkung di dalam direktori kerja tenant. File sistem sensitif di luar home direktori tidak dapat diakses atau dipindai.
- **Konsumsi Sumber Daya Terkendali**: Batasan ganda (waktu default 15s dan kuota hasil default 100) serta filter ukuran file 5 MB dan deteksi biner mencegah beban CPU tinggi dan lonjakan memori I/O.
- **Fleksibilitas Pengguna**: Pengguna dapat memilih pencarian instan untuk berkas terdekat atau mendelegasikan pemindaian direktori besar ke background job tanpa membuat browser menunggu.
