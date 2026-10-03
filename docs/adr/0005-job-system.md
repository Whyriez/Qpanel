# ADR-0005: Sistem Job Asinkron, State Machine, Worker, dan SSE Progres

- **Status**: Diterima (Accepted)
- **Tanggal**: 2026-10-03
- **Konteks**: PRD Bagian 4.7, 11.2, Task P1-T7

## Konteks dan Masalah

Operasi hosting control panel banyak melibatkan proses berdurasi panjang (long-running operations), seperti kompilasi aset, instalasi runtime, penerbitan sertifikat SSL, pembuatan backup, dan pengaturan service tenant. Sesuai PRD Bagian 4.7:
> "Job: operasi panjang (install runtime, build, backup, extract) dicatat di tabel `jobs` (`queued → running → succeeded|failed|cancelled`), punya progress dan log, dialirkan ke UI lewat SSE, retry dengan backoff, idempoten, dan bertahan saat restart (job `running` yang terputus dilanjutkan atau ditandai gagal secara eksplisit)."

Kebutuhan arsitektur:
1. **Atomic Claiming**: Worker harus mengambil job berikutnya secara atomik langsung dari SQLite tanpa race condition atau lock terdistribusi yang berat.
2. **State Machine & Retry Backoff**: Transisi status strictly defined (`queued`, `running`, `succeeded`, `failed`, `cancelled`). Kegagalan sementara harus di-retry dengan exponential backoff hingga mencapai `max_retries`.
3. **Idempotensi**: Mencegah eksekusi ganda jika klien mengirim ulang permintaan pembuatan job yang sama (misal jaringan terputus atau retry klien).
4. **SSE Progres Real-Time**: Progres eksekusi (0–100%) dan pesan log dialirkan ke antarmuka pengguna secara asinkron melalui Server-Sent Events (SSE).
5. **Pemulihan Setelah Restart (Crash Recovery)**: Jika daemon `panel-api` mati mendadak saat ada job yang sedang berstatus `running`, sistem saat booting harus memulihkan kondisi tersebut secara eksplisit.
6. **Isolasi RBAC Tenant**: Tenant biasa hanya diizinkan melihat, membatalkan, dan menerima progres streaming dari job miliknya sendiri. Akses terhadap job tenant lain harus ditolak (`403 Forbidden`).

## Keputusan Arsitektur

### 1. Atomic Job Claiming Menggunakan SQLite 3.35+ `RETURNING`
Pengambilan job berikutnya oleh worker menggunakan kueri SQL tunggal yang atomik:
```sql
UPDATE jobs
SET status = 'running', updated_at = ?
WHERE id = (
    SELECT id FROM jobs
    WHERE status = 'queued'
    ORDER BY created_at ASC, id ASC
    LIMIT 1
)
RETURNING id, user_id, name, status, progress, payload_json, result_json, error_message, retry_count, max_retries, created_at, updated_at
```
Hal ini menjamin integritas transaksi SQLite WAL tanpa memerlukan lock file atau antrean eksternal (seperti Redis).

### 2. State Machine & Handler Registry
- Modul `crates/panel-api/src/jobs/registry.rs` menyediakan trait `JobHandler` dengan `JobContext` yang aman.
- Handlers menerima `JobContext` yang memfasilitasi pelaporan progres `report_progress(progress, message)` dan pengecekan pembatalan `is_cancelled()`.
- Handler bawaan yang terdaftar:
  - `test.sleep`: mensimulasikan tugas bertahap dengan pelaporan progres berkala dan dukungan pembatalan.
  - `test.fail`: memicu kegagalan terukur untuk menguji mekanisme retry dan backoff.
  - `system.echo`: merefleksikan payload sebagai hasil eksekusi.
- Crate mendatang (Phase 2–4) dapat mendaftarkan handler baru ke `JobRegistry` tanpa mengubah kode inti runner.

### 3. Retry dengan Exponential Backoff
Ketika handler mengembalikan kegagalan:
- Cek batas `job.retry_count + 1 < job.max_retries`.
- Jika masih ada kuota percobaan:
  - Hitung durasi backoff: `100ms * 2^retry_count`.
  - Status diperbarui kembali ke `queued` dengan counter `retry_count` bertambah dan `error_message` tersimpan.
  - Event SSE berstatus `queued` dipancarkan menginformasikan jadwal retry.
  - Worker menunda pemrosesan selama durasi backoff.
- Jika kuota retry habis:
  - Status diperbarui ke `failed`.
  - Event SSE berstatus `failed` dipancarkan dengan pesan kesalahan permanen.

### 4. Idempotensi Pengajuan Job
Pada endpoint `POST /api/v1/jobs`, klien dapat menyediakan `id` kustom (UUID).
- Jika `id` sudah ada di database, sistem mengembalikan data job yang sudah ada dengan status `200 OK` (tanpa membuat duplikasi ataupun mereset eksekusi).
- Jika `id` tidak diberikan atau belum ada, sistem mengalokasikan UUID v4 baru dan mengembalikan `201 Created`.

### 5. Pembatalan Job (Cancellation)
- Job berstatus `queued` atau `running` dapat dibatalkan melalui `POST /api/v1/jobs/{id}/cancel`.
- Untuk job yang sedang aktif dieksekusi, `JobRunner` memicu sinyal `tokio_util::sync::CancellationToken` yang terpaut dengan job tersebut.
- Handler mendeteksi sinyal ini dan menghentikan proses, kemudian status job disetel ke `cancelled`.

### 6. Streaming Progres Real-Time via SSE
Endpoint `GET /api/v1/jobs/{id}/events` menggunakan `axum::response::sse::{Event, KeepAlive, Sse}`:
- Saat klien pertama kali terhubung, sistem mengirimkan snapshot status terkini job (`progress` event).
- Jika job sudah dalam status terminal (`succeeded`, `failed`, `cancelled`), stream langsung ditutup secara elegan.
- Jika job masih aktif (`queued` atau `running`), stream menerima event progres dari `tokio::sync::broadcast::Sender` yang difilter khusus untuk ID job tersebut.
- Ketika job mencapai status terminal, event terakhir dipancarkan dan stream ditutup otomatis.
- Heartbeat `KeepAlive` otomatis diaktifkan untuk mencegah pemutusan koneksi idle oleh proxy reverse.

### 7. Pemulihan Saat Startup (Crash Recovery)
Saat `panel-api` melakukan boot up (`main.rs`):
- Memanggil `job_repo.recover_interrupted_jobs()`.
- Kueri SQL mencari job dengan status `running` yang tertinggal akibat pemadaman/crash sebelumnya, lalu menandainya sebagai `failed` secara eksplisit dengan keterangan `"Interrupted by server restart"`.
- Setelah pemulihan selesai, background worker loop dijalankan dalam task Tokio tersendiri dengan penanganan graceful shutdown terpadu.

### 8. Penegakan Otorisasi RBAC Multi-Tenant
Sesuai `crates/panel-core/src/rbac.rs` (`Action::ManageJob { owner_id }`):
- Administrator memiliki akses global untuk menginspeksi, membatalkan, dan mendengarkan event semua job.
- Tenant biasa (`Role::User`) hanya diizinkan menginspeksi, membatalkan, dan mendengarkan stream event dari job yang memiliki `user_id == Some(auth_user.id)`. Percobaan akses ke job tenant lain akan ditolak dengan `403 Forbidden`.
- Daftar job pada `GET /api/v1/jobs` otomatis diisolasi untuk hanya menampilkan job milik tenant yang bersangkutan.

## Konsekuensi

- **Positif**:
  - Seluruh operasi asinkron panel terlacak secara terpusat, aman dari crash, dan transparan bagi pengguna.
  - Implementasi SSE ringan tanpa kebutuhan dependensi broker pesan eksternal.
  - Pengujian integrasi komprehensif (`job_test.rs`) memvalidasi seluruh alur state machine, retry backoff, pembatalan, pemulihan restart, dan isolasi tenant.
- **Negatif**:
  - Broadcast channel in-memory saat ini terbatas pada single-node daemon. Jika kelak di-scale ke multi-node (di luar cakupan v1), broadcast sender dapat diintegrasikan dengan SQLite notifications atau Redis pub/sub.
