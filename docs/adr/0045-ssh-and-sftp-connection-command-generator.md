# ADR-0045: Generator Perintah Koneksi SSH, SFTP, dan Dialog Integrasi UI

## Status
Diterima (Accepted)

## Konteks
Sesuai spesifikasi `PRD (1).md` baris 797 (P6-T7):
> "Salin perintah SSH: tombol/modal untuk menyalin perintah SSH interaktif ke folder aktif (`ssh -t user@host "cd '<folder>' && exec $SHELL -l"`), dengan informasi SFTP."
>
> Desain dan Acceptance Criteria:
> 1. Shell escaping yang aman untuk argumen folder dengan tanda kutip tunggal (`'`).
> 2. Penanganan port kustom non-standar (flag `-p <port>` untuk SSH, `-P <port>` untuk SFTP).
> 3. Integrasi kebijakan shell (`ShellPolicy`): jika tenant dibatasi pada kebijakan `sftp`, perintah shell interaktif dinonaktifkan dengan banner peringatan yang jelas dan mengarahkan ke perintah SFTP / client SFTP (FileZilla, Cyberduck, VS Code SFTP). Jika `none`, seluruh akses remote dinonaktifkan.
> 4. Menyediakan informasi koneksi SFTP lengkap (GUI client params, format URL `sftp://`, dan `~/.ssh/config` snippet).
> 5. Integrasi ke UI File Manager (toolbar & context menu) serta Site Detail (header & card Overview).

Tantangan arsitektural utama yang diselesaikan dalam ADR ini:
1. **Keamanan Eksekusi Shell (POSIX Single-Quote Escaping):** Bila path berisi spasi, simbol aneh, atau tanda kutip tunggal (`'`), penulisan perintah tanpa sanitasi dapat menimbulkan kerentanan command injection pada mesin lokal pengguna atau error syntax bash (`syntax error near unexpected token`).
2. **Penundaan Ekspansi Variabel Lingkungan Lokal (`$SHELL`):** Jika perintah ditulis `"cd '...' && exec $SHELL -l"`, shell lokal pengguna (zsh/bash di laptop developer) dapat mengekspansi `$SHELL` lokal sebelum dikirim ke remote. Perintah harus menggunakan `exec \$SHELL -l` agar diekspansi oleh remote login shell tenant.
3. **Penyelarasan Host dan Hak Akses RBAC:** API endpoint `GET /api/v1/ssh/connection-info` harus memverifikasi bahwa tenant hanya dapat meminta info koneksi untuk dirinya sendiri, sedangkan admin dapat menyalin info untuk tenant mana pun (dengan impersonasi aman). Hostname harus dipetakan secara cerdas: domain situs (bila ada), request host header, atau fallback loopback.

## Keputusan Arsitektur

### 1. Fungsi Sanitasi dan Generator Perintah di `panel-core`
Generator perintah diisolasi sebagai modul murni dan teruji di `crates/panel-core/src/ssh.rs`:
- **`escape_shell_single_quote(s: &str) -> String`**:
  Menggantikan setiap karakter `'` dengan `'\''` sesuai kaidah standar POSIX single-quoted literal escaping:
  ```rust
  pub fn escape_shell_single_quote(s: &str) -> String {
      s.replace('\'', r#"'\''"#)
  }
  ```
- **`generate_ssh_command(params: &SshConnectionParams) -> String`**:
  Menghasilkan:
  `ssh [-p <port>] -t <user>@<host> "cd '<escaped_folder>' && exec \$SHELL -l"`
- **`generate_sftp_command(params: &SshConnectionParams) -> String`**:
  Menghasilkan:
  `sftp [-P <port>] <user>@<host>:'<escaped_folder>'`
- **`generate_sftp_url(params: &SshConnectionParams) -> String`**:
  Menghasilkan deep-link URL berstandar URI:
  `sftp://<user>@<host>[:<port>]/<path>`
- **`generate_ssh_config_snippet(params: &SshConnectionParams, alias: Option<&str>) -> String`**:
  Menghasilkan konfigurasi siap pakai untuk dimasukkan ke `~/.ssh/config`.

### 2. Endpoint REST API `GET /api/v1/ssh/connection-info`
Diimplementasikan pada `crates/panel-api/src/ssh/handlers.rs`:
- **Parameter Query (`SshConnectionInfoQuery`):**
  - `path`: target direktori relatif atau absolut (dikanonikalisasi ke folder home tenant).
  - `site_id`: ID situs opsional untuk menyelesaikan hostname domain primer dan docroot.
  - `tenant`: username tenant target (hanya diizinkan bila pemanggil memiliki role `Admin`).
- **Autentikasi & RBAC:**
  - Tenant non-admin otomatis dibatasi pada data username tenant miliknya.
  - Memeriksa `ShellPolicy` tenant (`full`, `sftp`, atau `none`).
  - Menghitung `can_shell` dan `can_sftp` berdasarkan kebijakan yang berlaku.
  - Memeriksa ketersediaan public key SSH tenant (`has_ssh_keys` dan `ssh_keys_count`).
- **Output DTO (`SshConnectionInfoResponse`):**
  Mengembalikan parameter terstruktur lengkap (`host`, `port`, `username`, `active_path`, `shell_policy`, `ssh_command`, `sftp_command`, `sftp_url`, `ssh_config_snippet`).

### 3. Komponen Dialog UI `SshConnectModal`
Dibuat di `web/src/components/ssh/SshConnectModal.tsx`:
- Mengikuti Nusa Panel Design Tokens (`var(--surface)`, `var(--raised)`, `var(--ink)`, `var(--primary)`, `var(--radius-*)`).
- **Tab Navigasi:**
  1. **SSH Terminal:** Perintah interaktif `ssh -t ...`, tombol salin cepat, opsi buka web terminal bawaan, dan peringatan jika belum ada SSH key yang terdaftar.
  2. **SFTP Transfer:** Perintah CLI `sftp ...`, deep-link `sftp://`, serta tabel parameter terstruktur untuk konfigurasi aplikasi FTP (Host, Port, Protocol, Username, Initial Path).
  3. **Konfigurasi `~/.ssh/config`:** Blok konfigurasi SSH client dengan instruksi append ke berkas lokal.
- **Pengaturan Tingkat Lanjut (Host & Port Override):**
  Pengguna dapat menyesuaikan hostname publik atau port SSH alternatif tanpa perlu melakukan request ulang ke server; perintah dikalkulasi ulang secara reaktif di sisi klien.
- **Penanganan Kebijakan Shell:**
  - Bila `shell_policy === 'sftp'`, tab SSH otomatis mendisplay banner peringatan oranye dan menonaktifkan perintah shell, memprioritaskan tab SFTP.
  - Bila `shell_policy === 'none'`, kedua fitur shell dan SFTP dinonaktifkan dengan banner merah yang jelas.

### 4. Integrasi Frontend
- **File Manager (`FileManager.tsx`):**
  - Ditambahkan tombol "Salin SSH" di header action bar dengan icon Terminal.
  - Ditambahkan item "Buka Perintah SSH / SFTP..." pada context menu klik kanan di direktori maupun area kosong canvas.
- **Detail Situs (`SiteDetailPage.tsx`):**
  - Ditambahkan tombol "SSH / SFTP" di quick action header.
  - Ditambahkan link cepat "Salin Akses SSH / SFTP" pada kartu informasi Docroot direktori di tab Overview.

## Konsekuensi & Dampak
- **Positif:**
  - Mengeliminasi friksi developer yang ingin langsung masuk ke direktori proyek melalui terminal laptop atau SFTP client.
  - Mencegah insiden command injection akibat path direktori yang mengandung spasi atau karakter kutip.
  - Mengurangi tiket bantuan pengguna terkait konfigurasi SSH client dan FileZilla.
  - Menjaga kepatuhan terhadap batas isolasi `ShellPolicy` tanpa membingungkan pengguna paket SFTP-only.
- **Keterbatasan:**
  - Jika server berada di balik NAT atau port forwarding khusus yang tidak terdeteksi dari header HTTP, pengguna perlu mengisi override Host / Port pada panel pengaturan lanjutan modal.
