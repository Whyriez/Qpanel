# ADR-0044: Kebijakan Shell Tenant (none | sftp | full) dan Isolasi Drop-in sshd

## Status
Diterima (Accepted)

## Konteks
Sesuai spesifikasi `PRD (1).md` baris 796 (P6-T6):
> "Kebijakan shell: `none | sftp | full` per plan/tenant; drop-in `sshd` untuk grup `panel-tenants` (SFTP-only via `internal-sftp` + chroot); validasi `sshd -t` sebelum reload; **uji bahwa akses SSH admin tidak pernah terkunci**."
> 
> Acceptance Criteria (baris 799):
> "terminal terbuka di folder aktif sebagai tenant yang benar; id, ls /home, dan cat /home/<tenant-lain> membuktikan isolasi; fork bomb di terminal tertahan oleh TasksMax; SFTP-only tidak bisa membuka shell."

Tantangan arsitektural utama yang diselesaikan dalam ADR ini:
1. **Pencegahan Admin Lockout:** Konfigurasi SSH Daemon yang salah atau terlalu agresif dapat mengunci akses SSH admin/developer ke mesin host VM.
2. **Pre-reload Syntax Verification (`sshd -t`):** Memastikan berkas konfigurasi drop-in tidak pernah langsung ditulis dan direload ke daemon sebelum sintaksnya terverifikasi 100% valid.
3. **Multi-level Shell Policy Enforcement:** Kebijakan tidak boleh hanya diterapkan di level SSH (misal `nologin` atau `internal-sftp`), tetapi juga harus ditegakkan secara serentak di level web terminal API (`/api/v1/terminal/sessions`), agar tenant dengan policy `sftp` atau `none` tidak dapat membypass batasan melalui web console.
4. **Fleksibilitas Inheritance vs Per-tenant Override:** Nilai default shell policy diwarisi dari `Plan` paket hosting, dengan kemampuan bagi admin untuk memberikan override per tenant.

## Keputusan Arsitektur

### 1. Struktur Drop-in OpenSSH (`/etc/ssh/sshd_config.d/50-panel.conf`)
Drop-in SSH daemon didesain khusus hanya menargetkan grup tenant dan SFTP khusus:
```sshd_config
# Nusa Panel - SSH Drop-in Isolation (Managed automatically)
Match Group panel-sftp
    ForceCommand internal-sftp
    ChrootDirectory /home
    PermitTTY no
    X11Forwarding no
    AllowTcpForwarding no
    AllowAgentForwarding no

Match Group panel-tenants
    X11Forwarding no
    AllowTcpForwarding no
    AllowAgentForwarding no
```
- **Jaminan Akses Admin Tidak Terkunci:** Pengguna sistem administratif (seperti `root`, `ubuntu`, `admin`) **TIDAK PERNAH** dimasukkan ke dalam grup `panel-tenants` maupun `panel-sftp`. Dengan demikian, direktif pembatasan, chroot, dan `PermitTTY no` tidak pernah memengaruhi sesi SSH admin.

### 2. Validasi Atomik Sintaks Sebelum Reload (`sshd -t`)
Pada `SshManager::apply_sshd_dropin`:
1. Konten drop-in ditulis terlebih dahulu ke berkas sementara di samping direktori tujuan (`50-panel.conf.tmp.<uuid>`).
2. `panel-agent` mengeksekusi `sshd -t -f <temp_path>` melalui `SystemRunner`.
3. Bila eksekusi menghasilkan non-zero exit code (syntax error), berkas sementara segera dihapus dan error `AgentError::ExecutionFailed` dikembalikan. Tidak ada perubahan konfigurasi aktif dan `systemctl reload ssh` **tidak dipanggil**.
4. Bila validasi berhasil: berkas dipindahkan secara atomik (`fs::rename`) ke lokasi aktif (`/etc/ssh/sshd_config.d/50-panel.conf`) dengan permission `0644`.
5. Systemctl reload dieksekusi secara aman (`systemctl reload ssh`).

### 3. Tiga Kebijakan Shell (`ShellPolicy`)
- **`none`:**
  - Login shell Linux diatur ke `/usr/sbin/nologin` (atau `/bin/false`).
  - Dikeluarkan dari grup `panel-sftp`.
  - Akses Web Terminal ditolak (`403 Forbidden`).
- **`sftp`:**
  - Login shell Linux diatur ke `/usr/lib/openssh/sftp-server` (atau fallback nologin).
  - Dimasukkan ke dalam sistem grup `panel-sftp`.
  - OpenSSH Match Group memaksakan `internal-sftp`, `ChrootDirectory /home`, dan `PermitTTY no`.
  - Akses Web Terminal ditolak (`403 Forbidden`) dengan pesan jelas dalam Bahasa Indonesia.
- **`full`:**
  - Login shell Linux diatur ke `/bin/bash`.
  - Dikeluarkan dari grup `panel-sftp`, dimasukkan ke grup `panel-tenants`.
  - Akses Web Terminal diizinkan dengan pengawasan isolasi TasksMax cgroup v2.

### 4. Sinkronisasi Model Data & API Layer
- **Database:** Kolom nullable `shell_policy` ditambahkan ke tabel `users` (`20261003000007_user_shell_policy.sql`). Bila bernilai `NULL`, tenant mewarisi kebijakan dari `plans.shell_policy`.
- **API Endpoints:**
  - `GET /api/v1/tenants/{id}/shell-policy`: Mengembalikan status kebijakan efektif, nilai override, dan apakah diwarisi dari plan.
  - `PUT /api/v1/tenants/{id}/shell-policy`: Admin dapat mengatur override (`none | sftp | full | null`). Nilai divalidasi dengan `panel_core::ShellPolicy::parse` dan disinkronkan ke agent melalui RPC `apply_ssh_policy`.
  - `POST /api/v1/ssh/dropin/apply`: Endpoint khusus admin untuk meregenerasi atau menguji drop-in SSH configuration.
- **Web UI:** Komponen kartu Kebijakan Shell diintegrasikan pada `TenantDetailPage.tsx` tab ringkasan, lengkap dengan indikator status warna StatusPill dan kontrol seleksi override untuk admin.

## Konsekuensi

### Positif
- **Zero Risk Admin Lockout:** Terbukti dengan unit test `test_admin_ssh_not_locked_out_when_tenant_dropin_applied` bahwa drop-in hanya menargetkan grup tenant dan tidak menyentuh user admin.
- **Isolasi SFTP Terjamin:** Tenant SFTP-only tidak dapat mengalihkan eksekusi ke shell interaktif atau web terminal.
- **Fail-safe Configuration:** Kesalahan penulisan berkas drop-in tidak pernah merusak layanan SSH sistem karena validasi `sshd -t` selalu dilakukan sebelum penerapan berkas.
- **Audit Terintegrasi:** Setiap modifikasi kebijakan shell dicatat dalam audit logging dengan detail aktor, user target, dan perubahan nilai.

### Netral / Pertimbangan
- Pada sistem Linux host/VM pengujian, OpenSSH daemon memerlukan direktori chroot `/home` berpemilik `root:root` dengan permission `0755` agar `internal-sftp` dapat berfungsi dengan aman tanpa error chroot permission.
