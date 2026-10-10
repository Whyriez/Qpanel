# ADR-0032: API File Manager dan Siklus Hidup Trash Terisolasi

## Status
Diterima (Accepted)

## Konteks
PRD Bab 5.3 dan Task P5-T3 mensyaratkan:
> **P5-T3 API operasi**: list (cursor, sort, filter), stat, mkdir, rename, move, copy (job + progres), hapus → **trash** (restore, retensi, hitung quota), kosongkan trash.

Kebutuhan utama:
1. Endpoint RESTful yang efisien untuk navigasi dan manipulasi berkas di web panel.
2. Mekanisme penghapusan aman yang tidak langsung melenyapkan file secara permanen, melainkan memindahkannya ke tempat sampah (trash) dengan metadata lengkap agar dapat di-restore kapan saja.
3. Berkas di dalam trash harus tetap dihitung dalam kuota disk tenant (karena masih memakai storage server).
4. Penegakan otorisasi RBAC yang ketat: tenant biasa dilarang mengakses atau mengintip berkas milik tenant lain.
5. Pencatatan audit trail untuk setiap aksi modifikasi dan penghapusan.

## Keputusan Arsitektur

1. **Rute API RESTful (`/api/v1/files/*`)**:
   - `GET /api/v1/files/list`: Navigasi folder dengan paginasi (`limit`, `offset`), filter pencarian nama (`search`), ekstensi (`extension`), dan pengurutan (`sort_by`, `sort_order`).
   - `GET /api/v1/files/stat`: Mengambil metadata lengkap berkas atau direktori (ukuran, permission, MIME type, timestamps).
   - `POST /api/v1/files/mkdir`: Membuat direktori baru (opsional rekursif).
   - `POST /api/v1/files/rename` & `/move`: Mengubah nama atau memindahkan berkas/folder di dalam jail.
   - `POST /api/v1/files/copy`: Menyalin berkas atau pohon direktori secara rekursif.
   - `POST /api/v1/files/trash`: Memindahkan berkas ke `.panel-trash`.
   - `GET /api/v1/files/trash`: Menampilkan daftar berkas di dalam tempat sampah.
   - `POST /api/v1/files/trash/restore`: Mengembalikan berkas dari tempat sampah ke lokasi asalnya.
   - `DELETE /api/v1/files/trash/empty`: Mengosongkan tempat sampah secara permanen.
   - `DELETE /api/v1/files/permanent`: Menghapus berkas secara langsung tanpa masuk ke trash.

2. **Struktur Penyimpanan Trash (`.panel-trash`)**:
   - Setiap tenant memiliki direktori `.panel-trash` di root home mereka (`/home/p_<slug>/.panel-trash`).
   - Berkas yang di-trash dipindahkan ke `.panel-trash/files/<trash_id>`.
   - Metadata dicatat dalam `.panel-trash/metadata.json` (`trash_id`, `original_path`, `name`, `size_bytes`, `trashed_at`).
   - Karena berada di bawah home tenant, penggunaan disk oleh trash secara otomatis tercakup dalam kuota disk tenant (`quota.scan` dan `repquota`).

3. **Otorisasi RBAC dan Isolasi Tenant**:
   - Endpoint menerima parameter opsional `tenant` khusus untuk peran `admin`.
   - Tenant dengan peran `user` hanya diizinkan mengelola berkas di bawah username mereka sendiri. Jika mencoba melewatkan `tenant` lain, API mengembalikan respons `403 Forbidden`.

4. **Pencatatan Audit Trail**:
   - Setiap operasi yang mengubah state filesystem (`mkdir`, `rename`, `copy`, `trash`, `restore`, `empty`, `delete`) memicu penulisan rekaman audit ke database panel melalui `AuditWriter`.

## Konsekuensi
- Memudahkan frontend file manager (P5-T9) melakukan operasi CRUD secara aman.
- Mencegah kehilangan data fatal akibat ketidaksengajaan klik hapus melalui fitur restore trash.
