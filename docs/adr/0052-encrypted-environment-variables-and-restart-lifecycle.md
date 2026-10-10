# ADR 0052: Encrypted Environment Variables, Mask/Reveal UI, and Restart Lifecycle (P8-T1)

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.5, Bagian 6 (Keamanan), dan Bagian 8 (Fase 8 P8-T1 "Environment variables"):
1. **Keamanan Rahasia (Secrets)**:
   - Variabel lingkungan tenant harus dienkripsi dengan algoritma XChaCha20-Poly1305 (256-bit key, 192-bit nonce, 128-bit authentication tag).
   - Enkripsi dan dekripsi **hanya boleh dilakukan di agent daemon**. API server (`panel-api`) tidak boleh memegang master encryption key.
   - Variabel lingkungan **tidak pernah muncul dalam bentuk plaintext di database** (`panel.db`), log sistem, maupun audit log.
   - Berkas environment host (`/etc/panel/env/site-<id>.env`) wajib bertipe izin `0600 root` (systemd membacanya sebelum drop privilege ke user tenant).
2. **Kebutuhan Pengguna & UI**:
   - Tab `Environment` pada halaman detail situs (`SiteDetailPage.tsx`) berfungsi penuh.
   - Nilai default dimasking (`••••••••`) dengan tombol intip/toggle (reveal/hide per-variabel dan toggle global "Tampilkan Semua").
   - Kemampuan import berkas `.env` dengan penanganan komentar (`#`), spasi, kutip tunggal/ganda, karakter escape (`\n`, `\"`), awalan `export `, serta opsi gabung (*merge*) atau ganti (*replace*).
   - Penanda "Perlu restart" (*Restart Required banner*) muncul saat variabel lingkungan ditambah, diedit, diimpor, atau dihapus, dan otomatis hilang setelah proses aplikasi dimulai ulang (*restarted*).

## Keputusan Arsitektur

### 1. Modul Kriptografi Agent (`panel-agent::crypto`)
- Menggunakan crate resmi RustCrypto `chacha20poly1305 = "0.10"` dengan cipher AEAD `XChaCha20Poly1305`.
- Master key disimpan secara persisten pada host di `/etc/panel/secret.key` (32 bytes acak CSPRNG dari `OsRng`, mode `0600 root`).
- Format ciphertext terenkripsi: `enc:v1:<base64(24_byte_nonce || ciphertext_with_tag)>`.
- Agent mengekspos fungsi:
  - `encrypt_value` & `decrypt_value`: Memproses enkripsi/dekripsi dengan validasi nonce dan tag otentikasi.
  - `encrypt_env_vars` & `decrypt_env_vars`: Memproses map key-value secara atomik.
  - `apply_site_env_file`: Mendekripsi variabel rahasia di dalam memori agent dan menulis berkas `/etc/panel/env/site-<id>.env` dengan permission `0600 root`.

### 2. Protokol IPC Bertipe (`panel-proto`)
- Menambahkan request dan response bertipe ke dalam `RequestPayload` dan `ResponsePayload`:
  - `SiteEnvEncrypt(SiteEnvEncryptRequest)` -> `SiteEnvEncrypted(SiteEnvEncryptResponse)`
  - `SiteEnvDecrypt(SiteEnvDecryptRequest)` -> `SiteEnvDecrypted(SiteEnvDecryptResponse)`
  - `SiteEnvApply(SiteEnvApplyRequest)` -> `SiteEnvApplied(SiteEnvApplyResponse)`
- Agen memvalidasi nama key variabel agar mematuhi konvensi POSIX standard (`^[a-zA-Z_][a-zA-Z0-9_]*$`).

### 3. API Handlers & Zero-Plaintext Storage (`panel-api::env_vars`)
- Penyimpanan database (`sites.settings_json`):
  - Menyimpan map `"encrypted_env_vars": { "KEY": "enc:v1:..." }`.
  - Menyimpan flag `"env_restart_required": true / false`.
- Endpoints RESTful:
  - `GET /api/v1/sites/{id}/env`:
    - Query parameter `?reveal=bool` (default: `false`).
    - Jika `reveal=false`: Mengembalikan seluruh nilai masked (`••••••••`) dengan flag `is_masked: true`.
    - Jika `reveal=true`: Memanggil agent via `decrypt_site_env` dan mengembalikan nilai plaintext secara aman kepada klien terotorisasi.
  - `PUT /api/v1/sites/{id}/env`:
    - Memvalidasi nama key.
    - Mengirim payload ke agent untuk dienkripsi via `encrypt_site_env`.
    - Menyimpan hanya ciphertext terenkripsi ke database SQLite.
    - Menandai `env_restart_required = true`.
    - Memanggil agent untuk menulis berkas env host mode `0600`.
    - Mencatat audit log mutasi tanpa membocorkan nilai rahasia (hanya mencatat nama key yang diubah).
  - `POST /api/v1/sites/{id}/env/import`:
    - Mengurai (*parse*) konten format `.env` (mendukung komentar `#`, kutip tunggal/ganda, multi-line, escapes, dan export statement).
    - Mendukung opsi `merge: true` atau replace.
    - Mengenkripsi via agent, menyimpan ke DB, dan menandai `env_restart_required = true`.
  - `POST /api/v1/sites/{id}/env/restart`:
    - Melakukan restart service aplikasi yang bersangkutan (Node.js atau Python via agent).
    - Menghapus penanda `env_restart_required = false` pada database.
    - Aksi restart standar (`node/action` dan `python/action` dengan `action: "restart"`) juga otomatis membersihkan flag `env_restart_required`.

### 4. Komponen Antarmuka Web (`SiteEnvironmentTab.tsx`)
- Terintegrasi sebagai tab **"Environment"** pada halaman `SiteDetailPage.tsx`.
- Desain modern mengikuti token desain Nusa Panel (WCAG 2.2 AA, dark mode support):
  - Banner peringatan warna amber *"Perubahan Variabel Lingkungan Memerlukan Restart"* dengan tombol *"Restart Sekarang"* dan indikator loading.
  - Tabel variabel terenkripsi dengan format monospace, tombol salin key, dan tombol toggle mata (eye icon) untuk reveal/hide per-baris.
  - Tombol global *"Tampilkan Semua"* / *"Sembunyikan Semua"*.
  - Modal *"Tambah / Edit Variabel"* dengan validasi nama POSIX realtime.
  - Modal *"Import .env"* dengan textarea, pemilih file lokal, pratinjau deteksi jumlah variabel, dan pilihan merge/replace.
  - Modal konfirmasi hapus variabel dengan peringatan restart.
  - Dukungan lokalisasi dwibahasa penuh (ID / EN).

## Konsekuensi

### Positif
- **Kepatuhan Keamanan Penuh**: Plaintext rahasia tenant tidak pernah menyentuh database SQLite, log audit, maupun file log sistem. Enkripsi dan dekripsi hanya berada di agent daemon.
- **Isolasi Berkas Host**: Berkas `.env` host berizin `0600 root` sehingga pengguna lain maupun tenant proses di filesystem tidak dapat membaca berkas ini secara langsung; systemd membacanya sebelum melakukan *drop privilege*.
- **Pengalaman Pengguna yang Intuitif**: Pengembang dapat melihat nilai yang disembunyikan kapan saja melalui toggle reveal, mengimpor berkas konfigurasi `.env` dengan sekali klik, dan mengetahui status perlu restart aplikasi.

### Pertimbangan
- Dekripsi saat `?reveal=true` memerlukan satu putaran IPC ke agen daemon, yang dirancang cepat dan asinkron (sub-millisecond latency).
