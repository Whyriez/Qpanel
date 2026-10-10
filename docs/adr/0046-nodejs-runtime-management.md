# ADR 0046: Node.js Multi-Version Runtime Management

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.1 dan Fase 7 (P7-T1), Nusa Panel harus mendukung multi-versi Node.js tanpa intervensi manual atau mengotori sistem global:
1. Administrator harus dapat melihat katalog versi resmi Node.js (via `https://nodejs.org/dist/index.json` dengan fallback ke katalog offline terkurasi bila mirror tidak dapat diakses).
2. Pemasangan versi Node.js harus memverifikasi integritas checksum SHA256 terhadap manifest resmi `SHASUMS256.txt` sebelum diekstrak.
3. Direktori target pemasangan terisolasi per versi di `/opt/panel/runtimes/node/<versi>` dengan mode permissions binary `0755` dan penanganan ekstraksi aman anti-path-traversal.
4. Paket manager modern `pnpm` dan `yarn` diaktifkan otomatis melalui wrapper Corepack (`corepack enable`) di direktori binary runtime.
5. Proteksi penghapusan: runtime Node.js tidak boleh dihapus (dihard-block dengan HTTP 409 Conflict) jika versi tersebut masih digunakan oleh satu atau lebih situs di basis data.

## Keputusan Arsitektur

### 1. Ekstraksi dan Validasi Tarball Terisolasi (`panel-runtime`)
- Format direktori: `/opt/panel/runtimes/node/<versi>` (versi dinormalisasi tanpa awalan `v`, misal `22.14.0`).
- Tarball resmi Node.js (`node-v<versi>-linux-<arch>.tar.gz`) memiliki direktori root `node-v<versi>-linux-<arch>/`. Ekstraksi menerapkan stripping komponen pertama (`strip-components=1`) secara aman langsung ke target folder.
- Setiap entri arsip diverifikasi dengan `canonicalize()` agar tidak lolos keluar target path (`anti-path-traversal` attack prevention).
- Mode biner (`node`, `npm`, `npx`, `corepack`) diatur ke `0755` di Unix/Linux.

### 2. Validasi Integritas SHA256 Otomatis
- Download manifest `SHASUMS256.txt` dan ekstrak baris yang sesuai dengan file tarball target.
- Verifikasi byte array tarball menggunakan SHA256 hashing. Jika hash tidak cocok, proses dibatalkan seketika dengan error `ChecksumMismatch` tanpa menulis binary ke filesystem.
- Mendukung mode offline tarball path dan offline checksum untuk VM testing dan isolated environment.

### 3. Corepack Wrapper Shims
- Fungsi `enable_corepack_shims()` memeriksa direktori `bin/` runtime:
  - Memanggil `corepack enable --install-directory <bin_dir>` bila binary `node` dan `corepack` tersedia.
  - Sebagai mekanisme fallback / lingkungan non-root terisolasi, shims `pnpm` dan `yarn` dibuat secara deterministik memanggil binary `corepack` di direktori yang sama (`#!/bin/sh\nexec "$(dirname "$0")/corepack" pnpm "$@"`).
  - Pada Windows runner, script `.cmd` disiapkan untuk kompatibilitas developer.

### 4. Pencegahan Penghapusan Runtime yang Masih Digunakan (409 Conflict)
- Endpoint `DELETE /api/v1/runtimes/node/{version}` memeriksa database `sites` terlebih dahulu:
  - Mencari situs dengan `(kind = 'nodejs' OR runtime = 'nodejs')` yang memiliki versi sama.
  - Jika ditemukan, request ditolak dengan kode status `409 Conflict` dan detail daftar nama domain (`used_by_domains`) yang mencegah penghapusan.
  - Peristiwa dicatat ke audit log dengan status `"denied"`.

### 5. Antarmuka Web
- `RuntimePage.tsx` diperbarui dengan navigasi tab ganda (`[PHP]` dan `[Node.js]`).
- Indikator badge visual untuk status LTS beserta codename (`Jod`, `Iron`, `Hydrogen`), status instalasi, dan status Corepack.
- Tombol hapus dinonaktifkan dengan visual warning dan tooltip jika runtime digunakan oleh situs aktif.
- Modal pemasangan `NodeInstallModal` menyediakan pemilihan versi katalog, opsi Corepack, dan pengaturan mirror lanjutan.

## Konsekuensi
- **Positif**:
  - Runtime Node.js sepenuhnya modular, tidak mengotori binary OS di `/usr/bin/node`.
  - Keamanan terjamin karena verifikasi checksum SHA256 otomatis mencegah tarball corrupt atau termanipulasi.
  - Pengembang tenant mendapatkan akses instan ke `node`, `npm`, `pnpm`, dan `yarn`.
  - Situs produksi terlindungi dari downtime akibat penghapusan runtime yang tidak disengaja.
- **Batasan**:
  - Pengunduhan rilis baru membutuhkan koneksi internet ke mirror atau ketersediaan berkas offline di host. Fallback katalog terkurasi memastikan UI dan penentuan versi tetap responsif saat offline.
