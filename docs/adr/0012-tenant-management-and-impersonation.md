# ADR-0012: Manajemen Tenant, Hosting Plans, dan Impersonasi Aman

- **Status**: Diterima (Accepted)
- **Tanggal**: 2026-10-03
- **Konteks**: PRD Bagian 4.4, 5.1, 5.2, 5.3, 8.4, Task P2-T6

## Konteks dan Masalah

Pada fase 2, Nusa Panel membutuhkan sistem manajemen tenant dan paket hosting lengkap (P2-T6):
1. **Daftar & Detail Tenant**: Menampilkan daftar tenant dengan filter status, pencarian slug/email, informasi user Linux dan UID, status isolasi cgroup v2, serta visualisasi pemakaian sumber daya langsung (CPU %, Memory RSS/Peak, Disk quota & inode, Task count).
2. **Wizard Tambah Tenant**: Alur pembuatan tenant terpandu dalam 3 langkah (Identitas & Slug -> Pemilihan Paket Hosting -> Konfirmasi). Pembuatan tenant memicu provisioning terpadu: user Linux sistem (`p_<slug>`, UID >= 10000), shell policy, direktori home berizin `0700`, slice cgroup v2 (`tenant-<slug>.slice`), kuota disk, dan akun panel awal.
3. **Siklus Hidup Tenant (Suspend/Unsuspend/Delete)**:
   - *Suspend*: Mengunci akun Linux (`usermod -L`), mematikan proses aktif pada slice (`systemctl kill --signal=SIGTERM tenant-<slug>.slice`), membekukan controller cgroup (`cgroup.freeze = 1`), dan menonaktifkan status di database.
   - *Unsuspend*: Membuka kunci akun Linux (`usermod -U`), mencairkan slice (`cgroup.freeze = 0`), dan mengaktifkan kembali di database.
   - *Delete*: Menghapus user Linux (`userdel`), menghapus slice cgroup, membersihkan kuota, dan opsi opsional menghapus direktori home (`rm -rf /home/<linux_user>`).
4. **Dialog Tindakan Berbahaya (`ConfirmDangerDialog`)**: Konfirmasi tindakan berisiko tinggi (hapus tenant, hapus paket hosting) yang mewajibkan pengetikan teks konfirmasi (misal nama slug) untuk mencegah kecelakaan operasional.
5. **Paket Hosting (Hosting Plans)**:
   - CRUD template paket hosting: CPU quota %, memory limit & high bytes, tasks max, IO weight, disk quota & inode, batasan jumlah situs, domain, database, dan shell policy.
   - Propagasi dinamis: Pembaruan limit pada paket hosting secara otomatis menyinkronkan konfigurasi slice cgroup dan kuota disk pada semua tenant aktif yang menggunakan paket tersebut via `panel-agent`.
6. **Impersonasi Aman (Admin Masuk Sebagai Tenant)**:
   - Administrator dapat melakukan impersonasi akun tenant tanpa perlu mengetahui password tenant.
   - Banner peringatan mencolok (`ImpersonationBanner`) ditampilkan di bagian atas seluruh halaman dengan tombol "Kembali ke Admin".
   - Seluruh aktivitas saat impersonasi mencatat aktor sebenarnya (`impersonated_by`) ke catatan audit sistem (`audit_logs`).

## Keputusan Arsitektur

### 1. Endpoint REST API (`crates/panel-api`)
- **Plans API** (`crates/panel-api/src/plans/`):
  - `GET /api/v1/plans` & `GET /api/v1/plans/:id`: Menampilkan daftar dan detail paket.
  - `POST /api/v1/plans`: Membuat paket baru.
  - `PUT /api/v1/plans/:id`: Memperbarui batas sumber daya paket. Mengambil seluruh tenant aktif yang terikat dengan paket ini dan mengirim permintaan `SliceApply` serta `QuotaSet` ke agent secara otomatis.
  - `DELETE /api/v1/plans/:id`: Menghapus paket dengan pencegahan (paket default atau paket yang masih digunakan oleh tenant tidak dapat dihapus).
- **Tenants API** (`crates/panel-api/src/tenants/`):
  - `GET /api/v1/tenants`: Daftar tenant dengan pencarian dan filter status.
  - `GET /api/v1/tenants/:id`: Detail tenant termasuk data akun Linux, slice cgroup, dan kuota.
  - `GET /api/v1/tenants/:id/resources`: Pemantauan langsung (live reader) metrics cgroup v2 (`memory.current`, `memory.peak`, `pids.current`, dll.) dan disk quota.
  - `POST /api/v1/tenants`: Wizard provisioning tenant end-to-end (membuat user Linux, home dir, slice cgroup, kuota, record DB, dan akun panel).
  - `POST /api/v1/tenants/:id/suspend`: Membekukan cgroup slice dan menonaktifkan akun.
  - `POST /api/v1/tenants/:id/unsuspend`: Mencairkan cgroup slice dan mengaktifkan akun.
  - `DELETE /api/v1/tenants/:id`: Menghapus tenant dengan opsi purge berkas home.
  - `POST /api/v1/tenants/:id/impersonate`: Menginisiasi sesi impersonasi.
  - `POST /api/v1/auth/stop-impersonating`: Mengakhiri impersonasi dan memulihkan sesi admin.
  - `GET /api/v1/system/hardening` & `POST /api/v1/system/hardening/apply`: Diagnostik dan hardening host.

### 2. Sesi & Audit Trail Impersonasi
- Metadata sesi di `panel_db::models::session` mencatat identitas administrator (`impersonated_by: <admin_username>`) dalam `user_agent` atau atribut sesi.
- `panel-api/src/auth/middleware.rs` mengekstrak `impersonated_by` ke `AuthenticatedUser` dan `UserInfo`.
- Handler audit mencatat `actor` asli dan menyertakan konteks `impersonated_by` dalam metadata JSON audit log.

### 3. Komponen Antarmuka Web (`web/src/`)
- `components/ui/ConfirmDangerDialog.tsx`: Dialog modal dapat digunakan kembali dengan input pengetikan konfirmasi teks persis dan opsi hapus berkas.
- `components/layout/ImpersonationBanner.tsx`: Banner peringatan aksen kuning di atas header dengan aksi keluar instan.
- `components/tenants/TenantWizardDialog.tsx`: Wizard 3 langkah dengan validasi form, kalkulasi otomatis nama user Linux (`p_<slug>`), dan ringkasan limit.
- `pages/TenantsPage.tsx`: Tampilan tabel tenant, ringkasan diagnostik hardening sistem, filter status, pencarian, dan pemicu aksi.
- `pages/TenantDetailPage.tsx`: Halaman detail tenant dengan gauge/progress bar live resource cgroup v2 dan kuota disk, ringkasan isolasi, dan zona bahaya.
- `pages/PlansPage.tsx`: Manajemen template paket hosting dalam format kartu, badge shell policy, dan form modal buat/edit limit kernel.

## Konsekuensi

- Multi-tenant hosting dapat dikelola secara penuh baik dari API maupun antarmuka grafis.
- Batasan kernel cgroup v2 dan kuota filesystem ext4/xfs tersinkronisasi otomatis saat administrator memperbarui paket.
- Impersonasi aman dan akuntabel sesuai standar kepatuhan audit keamanan.
