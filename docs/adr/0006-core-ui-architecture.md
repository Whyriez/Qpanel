# ADR-0006: Arsitektur UI Inti (Auth, 2FA, JobTray, Audit, CommandPalette)

- **Status**: Diterima (Accepted)
- **Tanggal**: 2026-10-03
- **Konteks**: PRD Bagian 8.2–8.6, 11.2, Task P1-T8

## Konteks dan Masalah

Phase 1 menitikberatkan pada keandalan dan keamanan fondasi panel kontrol hosting (`panel-api`, `panel-agent`, dan `panel-db`). Pada task P1-T8:
> "UI inti: login, 2FA, wizard first-run, halaman profil/keamanan (sesi aktif, 2FA, API token), halaman Audit, JobTray, command palette (dasar)."

Kebutuhan arsitektur antarmuka pengguna:
1. **Design Tokens & Estetika**: Mengadopsi sistem desain dua tema sesuai PRD Bagian 8.2: tema terang *Kabut* dan tema gelap *Laut Dalam*. Tipografi menggunakan *Plus Jakarta Sans* dan *JetBrains Mono*, serta mematuhi WCAG 2.2 AA.
2. **Autentikasi & CSRF State Management**: `AuthContext` terpusat yang secara otomatis memeriksa sesi aktif via `/api/v1/auth/me`, memuat CSRF token via `/api/v1/auth/csrf`, dan menyertakan header `X-CSRF-Token` serta `credentials: 'include'` pada setiap mutasi REST.
3. **2FA Challenge & Recovery Codes**: Alur login mendeteksi flag `requires_2fa` dari server, membuka input 6-digit TOTP secara seamless dengan opsi beralih ke kode pemulihan (recovery codes 8 karakter). Halaman profil memungkinkan aktivasi 2FA dengan QR code base64, verifikasi awal, dan modal penyimpanan 8 kode pemulihan dengan tombol salin & unduh teks.
4. **First-Run Setup Wizard**: Memandu inisialisasi awal server bila token setup aktif, memvalidasi token, dan membimbing pembuatan akun administrator.
5. **Real-time JobTray Floating Dock**: Sesuai PRD 8.3 & 8.5, tray mengapung di kanan bawah menampilkan counter tugas aktif dengan animasi pulse. Membuka tray memberikan daftar tugas, progress bar 0–100%, pesan kesalahan, tombol pembatalan, dan live SSE updates otomatis.
6. **Command Palette (`cmdk`)**: Pintasan `Ctrl+K` atau `Cmd+K` untuk navigasi kilat ke seluruh halaman utama, penggantian tema instan, dan logout.
7. **Pencatatan Audit Terpaginasi**: Halaman `/audit` dengan filter aksi, status, dan modal penampil payload JSON terformat.

## Keputusan Arsitektur

### 1. Struktur Modul Frontend (`web/src/`)
- `context/AuthContext.tsx`: Mengelola status `user`, `csrfToken`, status `loading`, login, dan logout.
- `api/client.ts`: Abstraksi API terpadu yang menyematkan header `X-CSRF-Token` pada metode mutasi dan menangani koneksi Server-Sent Events (`EventSource`).
- `pages/`:
  - `LoginPage.tsx`: Formulir login dengan dukungan multi-langkah (Password $\to$ 2FA challenge $\to$ Recovery Code).
  - `FirstRunWizardPage.tsx`: Stepper setup awal 3 langkah (Token $\to$ Akun Admin $\to$ Selesai).
  - `SecurityProfilePage.tsx`: Manajemen profil, pembaruan kata sandi, dan lifecycle 2FA.
  - `AuditLogPage.tsx`: Tabel riwayat audit terpaginasi dengan modal inspeksi JSON.
  - `JobsPage.tsx`: Tabel antrean tugas dengan modal live streaming SSE.
- `components/layout/`:
  - `Shell.tsx`: Shell navigasi responsif dengan dukungan tema terang/gelap/sistem dan pengalihan bahasa (ID/EN).
  - `JobTray.tsx`: Floating tray dock di pojok kanan bawah.
- `components/ui/`:
  - `CommandPalette.tsx`: Komponen command palette berbasis library `cmdk`.
  - `StatusPill.tsx`, `PulseStrip.tsx`, `EmptyState.tsx`, `Skeleton.tsx`, `Toast.tsx`.

### 2. Standar Bahasa dan Aksesibilitas
- Bahasa bawaan: Bahasa Indonesia (sentence case, aksi jelas seperti "Simpan perubahan", "Verifikasi & Aktifkan"). Tersedia opsi beralih ke bahasa Inggris via `i18next`.
- Seluruh input interaktif memiliki label semantik, fokus terlihat, dan kontras warna sesuai rasio WCAG 2.2 AA.

## Konsekuensi

- **Positif**:
  - Pengguna mendapatkan pengalaman UI yang cepat, modern, dan informatif dengan umpan balik visual real-time (SSE).
  - Bundle JavaScript hasil build sangat ringan (~134 KB gzip), jauh di bawah target maksimal PRD (250 KB gzip).
  - Unit test Vitest dan TypeScript `tsc --noEmit` lolos 100%.
- **Negatif**:
  - Halaman manajemen spesifik Phase berikutnya (situs, domain, plan) masih berupa placeholder undangan beraksi sampai fase terkait diimplementasikan.
