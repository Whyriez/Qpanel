# ADR 0041: Arsitektur UI Terminal Web Multi-Tab Berbasis xterm.js & Integrasi Pengelola Berkas

## Status

Accepted

## Konteks

Setelah fondasi PTY agent (P6-T1) dan streaming WebSocket dua arah (P6-T2) selesai dibangun, Nusa Panel membutuhkan antarmuka pengguna web (front-end) modern untuk terminal sesuai PRD bagian 8:
1. **Emulasi Terminal Standar Industri**: Memerlukan parser escape code ANSI/VT100 lengkap yang andal, berperforma tinggi, mendukung kursor, warna 256/TrueColor, teks Unicode/UTF-8, dan hyperlink klik.
2. **Pengalaman Multi-Tab & Multi-Sesi**: Pengguna dapat membuka beberapa tab terminal konkuren secara bersamaan, berpindah tab secara instan, dan melihat indikator status koneksi (connected, connecting/reconnecting, disconnected) di setiap tab.
3. **Penyelarasan Desain Nusa Panel (Design Tokens & Tema)**: Terminal harus selaras dengan estetika token Nusa Panel (dark/light mode), tipografi monospace modern, kontras WCAG 2.2 AA, dan micro-interaction yang halus.
4. **Resposif & Auto-Resize (Fit Addon)**: Ukuran grid baris dan kolom terminal harus otomatis menyesuaikan lebar/tinggi wadah kontainer, serta mengirim pesan kontrol resize `{ type: "resize", cols, rows }` ke server API untuk disinkronkan ke agent PTY melalui `ioctl(TIOCSWINSZ)`.
5. **Dukungan Pencarian Teks & Quick Actions**: Menyediakan dialog pencarian teks melayang di atas terminal, pembersihan layar (*clear buffer*), dan tombol sambung ulang (*reconnect*).
6. **Integrasi Alur Kerja Pengelola Berkas (File Manager)**: Pengguna di antarmuka File Manager dapat langsung membuka terminal di folder yang sedang aktif via floating drawer tanpa harus meninggalkan konteks navigasi berkas.

## Keputusan Arsitektur

### 1. Komponen Inti Terminal (`TerminalInstance.tsx`)

- **Teknologi**: Menggunakan `@xterm/xterm` (v6.0) dengan addon resmi:
  - `@xterm/addon-fit`: Menghitung ukuran karakter font dan menyesuaikan dimensi kolom dan baris ke container div.
  - `@xterm/addon-web-links`: Mendeteksi URL HTTP/HTTPS pada baris keluaran terminal dan menjadikannya tautan yang dapat diklik.
  - `@xterm/addon-search`: Memungkinkan pencarian teks incremental dan navigasi kecocokan (*match previous/next*).
- **Tema Desain Nusa Panel**:
  - Palet tema gelap: background `#09191F` (deep marine), foreground `#E4EEF0`, kursor aksen `#3FC1C9` (primary cyan), kontras ANSI ANSI colors (red `#EF6B6B`, green `#4CC58A`, yellow `#F0B455`, blue `#6FA3EE`).
  - Palet tema terang: background `#F8FAFC`, foreground `#0F172A`, kursor `#0284C7`.
- **Streaming Biner Ringan**:
  - Mengonfigurasi WebSocket klien dengan `ws.binaryType = 'arraybuffer'`.
  - Input keyboard dikirimkan melalui `term.onData` sebagai `Uint8Array` biner langsung ke WebSocket tanpa overhead serialisasi JSON.
  - Output dari WebSocket diterima sebagai `ArrayBuffer` dan langsung dioper ke `term.write(new Uint8Array(event.data))`.
- **Sinkronisasi Resize Dua Arah**:
  - Menggunakan `ResizeObserver` pada wadah DOM container untuk memanggil `fitAddon.fit()`.
  - Bila dimensi berubah, komponen mengirimkan frame JSON `{ type: "resize", cols, rows }` ke WebSocket dan API REST.

### 2. Komponen Multi-Tab (`TerminalTabs.tsx`)

- Menampilkan daftar tab terminal dengan status badge dot interaktif:
  - Hijau (`bg-emerald-500`): Terhubung normal (*connected*).
  - Amber berkedip (`bg-amber-500 animate-pulse`): Sedang menghubungkan (*connecting* / *reconnecting*).
  - Merah (`bg-rose-500`): Terputus (*disconnected*).
- Menyediakan toolbar cepat di sisi kanan:
  - Tombol **Tab Baru** (`Plus`).
  - Tombol **Cari Teks** (`Search` / `Ctrl+F`) untuk membuka overlay pencarian.
  - Tombol **Sambung Ulang** (`RotateCw`) untuk rekoneksi manual jika koneksi terputus.
  - Tombol **Bersihkan Layar** (`Trash2` / `Ctrl+L`).
  - Tombol **Maksimalkan / Kembalikan Ukuran** (`Maximize2` / `Minimize2`).
  - Tombol **Tutup Sesi** (`X`).

### 3. Komponen Drawer Mengambang (`TerminalDrawer.tsx`) & Integrasi FileManager

- Dirancang sebagai bottom drawer melayang (`fixed bottom-0 z-50`) dengan tinggi adaptif (420px default, atau 85vh saat dimaksimalkan).
- Terintegrasi langsung dengan tombol **"Terminal di sini"** pada header pengelola berkas (`FileManager.tsx`):
  - Saat pengguna menavigasi subfolder (misal `sites/example.com/public`), mengklik "Terminal di sini" akan otomatis membuka drawer terminal dengan initial working directory `cwd` diset ke folder tersebut.
  - Memberikan transisi kerja yang mulus bagi pengembang/administrator situs tanpa bolak-balik menu.

### 4. Halaman Mandiri (`TerminalPage.tsx`) & Navigasi Aplikasi

- Disediakan halaman mandiri `/terminal` lengkap dengan container penuh, terintegrasi pada navigasi utama `Shell.tsx` dan `CommandPalette.tsx`.
- Tersedia i18n dwibahasa (Bahasa Indonesia & Bahasa Inggris) untuk semua label, keterangan status, dan tooltip.

## Konsekuensi

### Positif
- Latensi terminal mendekati nol berkat transmisi stream biner mentah murni antara browser xterm dan PTY backend.
- Pengguna mendapatkan pengalaman shell yang kaya (tab ganda, search, scrollback instan saat reconnect, drawer terintegrasi di file manager).
- Mematuhi seluruh standar UI Nusa Panel, WCAG 2.2 AA, dan zero type-checking error (`tsc -b` lulus 100%).

### Negatif / Mitigasi
- Addon xterm menambah ukuran bundle JS sekitar 300-400 KB. Mitigasi: xterm dimuat dengan tree-shaking efisien dan siap untuk code-splitting dynamic `import()` jika diperlukan pada build optimasi mendatang.
