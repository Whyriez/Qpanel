# ADR 0038: Arsitektur UI File Manager Custom dan Pengalaman Pengguna Terpadu

## Status
Accepted

## Konteks
Pada Phase 5 (File Manager Custom), task **P5-T9** menuntut implementasi antarmuka pengguna (UI) pengelola berkas yang modern, responsif, berkinerja tinggi, dan ramah pengguna sesuai Wireframe 8.4 dan aturan PRD:
1. **Navigasi Presisi**: Breadcrumbs navigasi path yang responsif, folder tree collapsible di panel kiri, dan integrasi mulus dengan layout shell panel.
2. **Fleksibilitas Tampilan**: Mode tampilan tabel terperinci (List) dan kartu kisi (Grid) dengan pengurutan kolom multi-dimensi (Nama, Ukuran, Terakhir Diubah, Ekstensi).
3. **Interaktivitas Kelas Desktop**:
   - Pilihan berganda (Multi-select via checkbox, `Ctrl`/`Cmd`+klik, dan `Shift`+klik untuk range selection).
   - *Rubber band marquee selection* (drag kotak seleksi transparan pada canvas area berkas).
   - Drag-and-drop berkas baik secara internal (pemindahan antar folder) maupun drop langsung file desktop dari OS ke browser untuk memicu unggahan otomatis.
4. **Alur Kerja Unggah Tangguh (Upload Tray)**:
   - Panel floating di pojok kanan bawah yang menampilkan progres unggah chunked resumable per file dengan indikator persentase, status bar animasi, dan kontrol jeda/lanjut/batal.
5. **Manajemen Tempat Sampah (.panel-trash)**:
   - Drawer geser samping untuk meninjau berkas yang terhapus ke `.panel-trash`, memulihkan berkas ke lokasi asal, atau mengosongkan sampah secara permanen.
6. **Integrasi Modal Lengkap**:
   - Menghubungkan secara terpadu modal editor teks CodeMirror 6 (P5-T7), modal preview media gambar/PDF/video/audio (P5-T7), modal permission & chmod POSIX (P5-T6), dan modal pencarian berkas & konten (P5-T8).
7. **Kesiapan Fase Berikutnya**:
   - Tombol "Terminal di sini" di baris atas ditampilkan dalam status dinonaktifkan (*disabled*) dengan tooltip edukatif "Tersedia di Phase 6 — Web terminal".

## Keputusan Arsitektur

### 1. Komponen Utama `FileManager` dan Halaman `FileManagerPage`
- Komponen inti [web/src/components/files/FileManager.tsx](file:///e:/Latihan/Panel%20Hosting/Vpanel/web/src/components/files/FileManager.tsx):
  - Dirancang modular dan menerima props `initialPath`, `baseRootPath`, `title`, dan `quotaInfo`.
  - Dapat dirender di dua tempat:
    1. Sebagai halaman mandiri penuh [web/src/pages/FileManagerPage.tsx](file:///e:/Latihan/Panel%20Hosting/Vpanel/web/src/pages/FileManagerPage.tsx) yang terhubung ke sidebar navigasi utama `Shell.tsx` (rute `files` dengan ikon `FolderTree`).
    2. Sebagai tab tertanam `"Berkas"` di dalam [web/src/pages/SiteDetailPage.tsx](file:///e:/Latihan/Panel%20Hosting/Vpanel/web/src/pages/SiteDetailPage.tsx), sehingga developer situs dapat langsung mengelola file docroot situs tanpa berpindah halaman.

### 2. Selection & Rubber Band Engine
- Marquee drag selection diimplementasikan murni di level React/DOM:
  - Container canvas melacak `onMouseDown`, `onMouseMove`, dan `onMouseUp`.
  - Koordinat bounding rectangle kotak seleksi dihitung secara dinamis.
  - Setiap elemen berkas bertanda `data-file-item="true"` dicek tumpang tindihnya (*intersection calculation*) dengan `getBoundingClientRect()`, memperbarui state `selectedPaths`.

### 3. Desktop & Internal Drag-and-Drop
- Menggunakan HTML5 Drag and Drop API:
  - Berkas internal di-drag dengan data payload `application/panel-files` bertipe JSON list path.
  - Berkas eksternal desktop dideteksi melalui `e.dataTransfer.files`, yang langsung memicu antrean unggah chunked resumable ke direktori aktif.

### 4. Floating Action Bar & Context Menu
- Floating Action Bar melayang di tengah bawah saat ada 1 atau lebih berkas terpilih:
  - Menampilkan jumlah berkas terpilih, tombol aksi massal (Arsipkan, Ubah Izin, Salin, Potong, Hapus ke Sampah), dan tombol batalkan seleksi.
- Context Menu kustom muncul saat klik kanan pada item berkas atau area kanvas kosong, menyediakan akses cepat untuk membuka di editor, pratinjau, unduh, ganti nama (F2), ekstrak arsip, dan ubah izin chmod.

### 5. Pengawasan Kuota Disk
- Baris atas menampilkan visual progress meter kuota disk (penggunaan vs batas kuota plan).
- Warnanya berubah adaptif: hijau ($\le 75\%$), kuning ($75\% - 90\%$), dan merah ($> 90\%$).

## Konsekuensi & Mitigasi
- **Kenyamanan Pengguna**: Menghadirkan pengalaman interaksi desktop-grade di dalam peramban web modern tanpa ketergantungan library pihak ketiga yang membengkak.
- **Aksesibilitas & Standar Desain**: Menggunakan design tokens Nusa Panel (`var(--surface)`, `var(--line)`, `var(--primary)`, dll.) dengan dukungan kontras warna tema terang dan gelap serta navigasi keyboard penuh (`Ctrl+A`, `Delete`, `F2`, `Ctrl+C`, `Ctrl+V`, `Ctrl+F`, `Esc`).
- **Konsistensi Keamanan**: Seluruh aksi mutasi dan pembacaan berkas tetap terkungkung di dalam confined filesystem `panel-fs` dan dieksekusi sebagai UID/GID tenant yang terisolasi.
