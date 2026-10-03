# ADR-0003: Role-Based Access Control (RBAC) & Kebijakan Otorisasi Terpusat

- **Status**: Diterima (Accepted)
- **Tanggal**: 2026-10-03
- **Konteks**: PRD Bagian 1.3, 4.2, 5.2, Task P1-T5

## Konteks dan Masalah

Panel hosting memiliki dua peran utama pengguna pada v1 (dengan rencana penambahan peran `reseller` di masa depan):
1. **Server Admin** (`role = "admin"`): pemilik server yang mengelola seluruh penyewa (tenant), paket hosting (plan), runtime, setelan server global, dan audit log menyeluruh.
2. **Tenant User** (`role = "user"`): pengguna penyewa hosting yang hanya boleh mengakses sumber daya miliknya sendiri (situs, domain, database, log, file, dan terminal) tanpa dapat mengakses atau melihat sumber daya penyewa lain ataupun setelan global server.

Diperlukan:
- Model peran yang kuat dan siap diperluas.
- Mesin kebijakan (policy engine) terpusat untuk memisahkan logika otorisasi dari handler HTTP.
- Middleware dan extractor Axum bertipe aman (`RequireAdmin`, `RequireAdminCsrf`, `RequireAuth`, `RequireCsrf`) agar handler tidak perlu menulis pemeriksaan berulang.
- Validasi isolasi lintas-penyewa pada tingkat API.

## Keputusan Arsitektur

### 1. Model Domain Role & Action (`panel-core`)
- Enum `Role`:
  - `Admin`: Administrator dengan hak akses penuh tanpa batasan.
  - `User`: Pengguna penyewa terisolasi.
  - `Reseller`: Disediakan secara forward-compatible untuk masa depan.
- Enum `Action`: Memetakan operasi domain seperti `ViewSystemInfo`, `ManageSettings`, `ListUsers`, `CreateUser`, `ViewUser { target_user_id }`, `UpdateUser { target_user_id }`, `SuspendUser`, `DeleteUser`, `ImpersonateUser`, `ViewAuditLogs`, `ManageSite { owner_id }`, `ManageDatabase { owner_id }`, `AccessFileManager { owner_id }`, dll.

### 2. Kebijakan Otorisasi Terpusat (`authorize` & `check_permission`)
- Fungsi `authorize(role: Role, subject_id: i64, action: &Action) -> bool`:
  - Jika peran adalah `Admin`: selalu bernilai `true`.
  - Jika peran adalah `User`:
    - Operasi global administratif (`ManageSettings`, `ListUsers`, `CreateUser`, `ManagePlan`, `ImpersonateUser`, `SuspendUser`, `DeleteUser`) ditolak (`false`).
    - Operasi audit log global ditolak; hanya diizinkan jika `target_user_id == Some(subject_id)`.
    - Operasi sumber daya tenant (`ManageSite`, `ManageDatabase`, `AccessFileManager`, `AccessTerminal`, dll.) hanya diizinkan bila `owner_id == subject_id`.
    - Akses data pengguna hanya diizinkan untuk ID pengguna sendiri (`target_user_id == subject_id`).

### 3. Middleware & Extractor Axum (`panel-api`)
- `RequireAuth(pub AuthenticatedUser)`: Memvalidasi cookie sesi, kedaluwarsa sesi, keberadaan user di database, dan status akun harus `active` (bila `suspended`, ditolak dengan HTTP 403 Forbidden).
- `RequireCsrf(pub AuthenticatedUser)`: Memvalidasi autentikasi dan header `X-CSRF-Token`.
- `RequireAdmin(pub AuthenticatedUser)`: Memvalidasi autentikasi dan memeriksa `user.is_admin()`. Jika bukan admin, mengembalikan HTTP 403 Forbidden (`{"error": "Forbidden: Administrator privileges required"}`).
- `RequireAdminCsrf(pub AuthenticatedUser)`: Memvalidasi autentikasi, hak admin, dan header `X-CSRF-Token` untuk rute mutasi state administratif.
- Metode pembantu pada `AuthenticatedUser`:
  - `can_access_user(target_user_id: i64) -> bool`
  - `authorize(action: &Action) -> bool`
  - `require_permission(action: &Action) -> Result<(), (StatusCode, &'static str)>`

### 4. Rute Administratif dan Pengujian Otorisasi
- Rute administratif:
  - `GET /api/v1/admin/users`: Daftar semua pengguna (khusus admin).
  - `GET /api/v1/admin/users/{id}`: Detail pengguna (khusus admin).
  - `POST /api/v1/admin/users/{id}/status`: Mengubah status akun pengguna (khusus admin + CSRF).
  - `GET /api/v1/admin/settings`: Membaca setelan sistem (khusus admin).
  - `POST /api/v1/admin/settings`: Mengubah setelan sistem (khusus admin + CSRF).
- Rute pembuktian isolasi tenant:
  - `GET /api/v1/user/{id}/profile`: Tenant hanya bisa melihat profil miliknya sendiri; mencoba melihat profil tenant lain ditolak dengan HTTP 403 Forbidden. Admin dapat melihat profil semua tenant.
