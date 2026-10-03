# ADR-0008: Model Hosting Plan dan Batas Resource Cgroup v2 Melalui Systemd Slice

- **Status**: Diterima (Accepted)
- **Tanggal**: 2026-10-03
- **Konteks**: PRD Bagian 4.4, 5.2, Task P2-T2

## Konteks dan Masalah

Untuk menjaga stabilitas lingkungan multi-tenant dan mencegah satu tenant menghabiskan sumber daya sistem secara berlebihan ("noisy neighbor" atau DoS), setiap tenant dialokasikan hosting plan dengan kuota resource yang terukur.

Berdasarkan PRD Bagian 4.4 dan 5.2:
> "Resource: Slice per tenant: `MemoryMax/High`, `CPUQuota`, `TasksMax`, `IOWeight`/bandwidth, dari plan."
> "Plan (paket): batas disk, inode, RAM, CPU, jumlah proses, jumlah site/domain/DB/cron, jumlah sesi terminal, batas upload, izin shell (`none` / `sftp` / `full`), izin upstream eksternal."

Kebutuhan arsitektur:
1. **Model Data Plan**: Penyimpanan paket hosting terstruktur di database (`panel-db`) mencakup batas CPU (`cpu_quota_percent`), RAM (`memory_limit_bytes`, `memory_high_bytes`), proses/task (`tasks_max`), I/O weight (`io_weight`), kuota disk/inode, kuota resource aplikasi (site/domain/db/cron/terminal), serta kebijakan shell dan network.
2. **Penerapan Cgroup v2 via Systemd Drop-in**: Menggunakan hierarki cgroup v2 bawaan systemd:
   - Nama slice: `panel-u<uid>.slice`
   - Direktori drop-in: `/etc/systemd/system/panel-u<uid>.slice.d/`
   - Berkas konfigurasi: `50-limits.conf`
   - Properti yang dikonfigurasi: `MemoryMax`, `MemoryHigh`, `CPUQuota`, `TasksMax`, `IOWeight`.
   - Reload konfigurasi: `systemctl daemon-reload`.
3. **Pemisahan Hak Akses**: `panel-api` tidak memegang hak root; penerapan limit dikirim melalui IPC bertipe (`SliceApplyRequest` dan `SliceRemoveRequest`) ke `panel-agent`.
4. **Pencegahan Injeksi & Testability**:
   - Nilai konfigurasi divalidasi ketat (karakter alfanumerik untuk memory, digit/% untuk CPU quota, range 1–10000 untuk IOWeight).
   - Pengoperasian file dan eksekusi `systemctl daemon-reload` diisolasi di dalam `SystemRunner` trait, memungkinkan pengujian komprehensif di lingkungan non-root tanpa modifikasi host.

## Keputusan Arsitektur

### 1. Model Data Plan (`panel-db`)
Tabel `plans` dibuat dengan skema SQL di `20261003000003_plans.sql`:
- Default paket `Starter` (100% CPU, 512 MB RAM, 100 tasks, 10 GB disk) dan `Pro` (200% CPU, 2048 MB RAM, 300 tasks, 50 GB disk).
- Relasi integritas referensial: plan yang sedang terhubung ke user tidak dapat dihapus (`FOREIGN KEY` check).

### 2. Protokol Bertipe (`panel-proto`)
Penambahan perintah ke payload RPC:
- `RequestPayload::SliceApply(SliceApplyRequest)` → `ResponsePayload::SliceApplied(SliceAppliedResponse)`
- `RequestPayload::SliceRemove(SliceRemoveRequest)` → `ResponsePayload::SliceRemoved(SliceRemovedResponse)`

```rust
pub struct SliceApplyRequest {
    pub uid: u32,
    pub memory_max: Option<String>,
    pub memory_high: Option<String>,
    pub cpu_quota: Option<String>,
    pub tasks_max: Option<u64>,
    pub io_weight: Option<u32>,
}
```

### 3. Modul `SliceManager` pada `panel-agent`
- Bertanggung jawab membuat drop-in `/etc/systemd/system/panel-u<uid>.slice.d/50-limits.conf` dengan hak akses `0644`.
- Menjalankan `systemctl daemon-reload` menggunakan allowlisted binary `"systemctl"`.
- Setiap operasi menulis audit record di log agent (`action: "slice.apply"`, `target: "panel-u<uid>.slice"`).

### 4. Client Integration pada `panel-api`
- Fungsi utilitas `plan_to_slice_apply_request(uid, plan)` mengonversi representasi numerik plan di DB ke string format unit systemd (mis. `2048M`, `150%`).
- Pengujian integration test `plan_slice_ipc_test` memverifikasi seluruh siklus hidup: CRUD plan DB -> konversi ke slice request -> pengiriman IPC -> pembuatan berkas konfigurasi drop-in dan eksekusi `daemon-reload` -> penghapusan slice.

## Konsekuensi

- **Positif**:
  - Seluruh service tenant (`panel-u<uid>.slice`) yang dijalankan oleh unit systemd (seperti master php-fpm dan Node/Python daemon) secara otomatis berada di bawah batas cgroup yang didefinisikan plan.
  - Perubahan batas resource dilakukan secara deklaratif dan idempoten.
  - Bebas risiko injeksi shell karena pembuatan berkas dilakukan lewat syscall tulis file langsung dan pemanggilan `systemctl daemon-reload` tanpa parser shell.
- **Batasan**:
  - Pada lingkungan non-systemd (seperti container Docker sederhana tanpa init), cgroup v2 slice tidak aktif. Pengujian memanfaatkan `MockSystemRunner` untuk validasi deterministik di lingkungan CI dan developer lokal.
