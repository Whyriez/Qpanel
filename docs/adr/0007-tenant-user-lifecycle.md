# ADR-0007: Lifecycle dan Isolasi User Linux Tenant

- **Status**: Diterima (Accepted)
- **Tanggal**: 2026-10-03
- **Konteks**: PRD Bagian 4.3, 4.4, 5.2, Task P2-T1

## Konteks dan Masalah

Isolasi antar-tenant merupakan fondasi utama keamanan shared hosting pada Nusa Panel. Berdasarkan PRD Bagian 4.4:
> "Batas isolasi = **satu tenant (user panel)**. 1 tenant = 1 user Linux `p_<slug>` (UID 10000–59999), tanpa password login, shell sesuai kebijakan."
> "Home `0700` + **ACL** `u:<nginx-user>:--x` pada home; docroot `0750` + ACL baca untuk nginx. Tenant lain tidak bisa traverse."

Kebutuhan arsitektur dan batasan keamanan:
1. **Pemisahan Hak Akses Keras (Rule 4.3)**: `panel-api` tidak pernah berjalan sebagai root dan tidak menyentuh `/etc` atau `/home` secara langsung. Semua manipulasi sistem didelegasikan ke daemon berprivilese `panel-agent` melalui RPC bertipe.
2. **Tanpa Shell Interpolation (Rule 4.3(4) & Rule 4)**: Dilarang keras membangun perintah sistem melalui shell atau string interpolation. Binary eksternal harus dipanggil langsung via `Command::new(binary).args(...)` dengan allowlist ketat.
3. **Validasi Ganda**: Input divalidasi dengan newtype di API (`panel-core`) dan **divalidasi ulang** secara ketat di sisi agent sebelum dieksekusi.
4. **Siklus Hidup Lengkap**:
   - `create`: pembuatan user tanpa password, alokasi UID 10000–59999, grup primer `p_<slug>`, grup suplemen `panel-tenants`, layout direktori home (`0700`), subdirektori awal (`sites` mode `0750`, `tmp`, `.ssh`, `.panel-trash`), dan entri ACL untuk web server (`u:www-data:--x`).
   - `suspend`: penguncian akun (`usermod -L`), pengalihan shell ke `/usr/sbin/nologin`, dan terminasi seluruh proses tenant yang sedang berjalan (`pkill -KILL -u <uid>`).
   - `unsuspend`: pembukaan kunci akun (`usermod -U`) dan pemulihan shell sesuai kebijakan.
   - `delete`: terminasi proses, opsi retensi (arsip direktori home ke index trash) atau purge (penghapusan permanen).
5. **Kemampuan Uji Mandiri (Testability & Safety)**: Menghindari modifikasi sistem developer saat development lokal (Rule 2) dengan menyediakan abstraksi eksekutor (`SystemRunner`) yang dapat di-mock untuk pengujian tanpa root di lingkungan mana pun.

## Keputusan Arsitektur

### 1. Eksekutor Sistem Berbasis Trait (`SystemRunner`)
Untuk mematuhi Rule 2 dan memungkinkan pengujian komprehensif tanpa memerlukan akses root host:
```rust
pub trait SystemRunner: Send + Sync {
    fn run_command(&self, binary: &str, args: &[&str]) -> Result<std::process::Output, AgentError>;
    fn create_dir_all(&self, path: &Path, mode: u32, uid: u32, gid: u32) -> Result<(), AgentError>;
    fn set_permissions(&self, path: &Path, mode: u32) -> Result<(), AgentError>;
    fn set_ownership(&self, path: &Path, uid: u32, gid: u32) -> Result<(), AgentError>;
    fn set_facl(&self, path: &Path, user: &str, permissions: &str) -> Result<(), AgentError>;
    fn move_path(&self, from: &Path, to: &Path) -> Result<(), AgentError>;
    fn remove_dir_all(&self, path: &Path) -> Result<(), AgentError>;
    fn path_exists(&self, path: &Path) -> bool;
}
```
- `RealSystemRunner`: Menjalankan binary Linux yang telah di-allowlist (`useradd`, `usermod`, `userdel`, `groupadd`, `groupdel`, `setfacl`, `pkill`) dan syscall sistem berkas Unix (`PermissionsExt`, `chown`).
- `MockSystemRunner`: Merekam seluruh pemanggilan binary, argumen, hak akses, mode direktori, dan mutasi file ke dalam memori untuk pengujian unit dan integration test IPC yang aman dan deterministik.

### 2. Allowlist Binary dan Keamanan Perintah
Agent menolak mengeksekusi binary apa pun di luar daftar resmi:
```rust
pub const ALLOWED_BINARIES: &[&str] = &[
    "useradd", "usermod", "userdel", "groupadd", "groupdel", "setfacl", "pkill",
];
```
Argumen diteruskan sebagai array string terpisah tanpa melibatkan parser shell (`/bin/sh` atau `/bin/bash`), sehingga serangan command injection tidak dapat terjadi secara struktural.

### 3. Kontrak RPC Bertipe pada `panel-proto`
Perintah user lifecycle ditambahkan ke enum `RequestPayload` dan `ResponsePayload`:
- `RequestPayload::UserCreate(UserCreateRequest)` → `ResponsePayload::UserCreated(UserCreatedResponse)`
- `RequestPayload::UserSuspend(UserSuspendRequest)` → `ResponsePayload::UserSuspended(UserSuspendedResponse)`
- `RequestPayload::UserUnsuspend(UserUnsuspendRequest)` → `ResponsePayload::UserUnsuspended(UserUnsuspendedResponse)`
- `RequestPayload::UserDelete(UserDeleteRequest)` → `ResponsePayload::UserDeleted(UserDeletedResponse)`

### 4. Validasi Domain dan Pencegahan Tabrakan User Sistem
Di `panel-core`:
- `Username`: Wajib diawali prefiks `p_`, panjang 3–30 karakter, hanya karakter alfanumerik huruf kecil dan garis bawah `_`.
- `is_reserved_username`: Menolak nama user yang bertabrakan dengan akun sistem umum Linux (`root`, `daemon`, `nginx`, `www-data`, `panel`, `mysql`, `postgres`, `redis`, dll.).
- `TenantUid`: Membatasi UID secara ketat pada rentang 10000..=59999.
- `ShellPolicy`: Memetakan kebijakan `none` (`/usr/sbin/nologin`), `sftp` (`/usr/lib/openssh/sftp-server`), dan `full` (`/bin/bash`).

### 5. Audit Logging Otomatis
Setiap eksekusi lifecycle user oleh agent otomatis mencatat record terstruktur:
`AgentAuditRecord::new("panel-api", "user.<create|suspend|unsuspend|delete>", username, status, req_id)`.

## Konsekuensi

- **Positif**:
  - Privilege separation terpenuhi sempurna; API tidak memerlukan hak root.
  - Direktori home tenant dibuat terisolasi dengan mode 0700 dan ACL traversal khusus web server, mencegah pembacaan lintas tenant.
  - Seluruh alur lifecycle user dapat diuji secara otomatis dalam CI dan mesin lokal tanpa merusak konfigurasi sistem developer.
  - Penghapusan mendukung retensi aman (trash index) sebelum tindakan pemusnahan permanen.
- **Tantangan**:
  - Pengujian di mesin produksi/VM Ubuntu 24.04 aktual memerlukan utilitas paket `acl` (`setfacl`) terpasang. Skrip dev VM (`scripts/dev-vm.sh`) sudah menyertakannya.
