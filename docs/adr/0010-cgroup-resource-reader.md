# ADR-0010: Pembaca Metrik Resource Cgroup v2 per Slice Melalui Agent

- **Status**: Diterima (Accepted)
- **Tanggal**: 2026-10-03
- **Konteks**: PRD Bagian 4.4, 5.2, 5.6, Task P2-T4

## Konteks dan Masalah

Untuk menyajikan informasi penggunaan sumber daya secara real-time pada dashboard (CPU usage, RAM saat ini dan peak, jumlah thread/proses aktif, dan throughput disk I/O) bagi setiap tenant tanpa membebani sistem dengan polling berat atau memberikan hak istimewa berlebih ke `panel-api`, pembacaan metrik harus bersumber langsung dari berkas kontrol cgroup v2 Linux di kernel.

Berdasarkan PRD:
- **P2-T4**: "Pembaca resource: baca cgroup v2 per slice (`cpu.stat`, `memory.current`, `io.stat`, `pids.current`) lewat agent."
- **Keamanan**: `panel-api` tidak memiliki hak akses langsung ke sistem berkas `/sys/fs/cgroup` yang berpotensi sensitif, melainkan meminta data metrik bertipe via IPC ke `panel-agent`.

Kebutuhan arsitektur:
1. Pembacaan berkas kontrol cgroup v2 per tenant slice:
   - `cpu.stat`: waktu eksekusi total (`usage_usec`), waktu user (`user_usec`), waktu sistem kernel (`system_usec`), serta metrik throttling (`nr_periods`, `nr_throttled`, `throttled_usec`).
   - `memory.current` & `memory.peak`: pemakaian memori riil tenant dan puncak memori tertinggi dalam byte.
   - `pids.current` & `pids.peak`: jumlah proses/thread aktif saat ini dan puncak proses.
   - `io.stat`: agregasi akumulatif byte baca/tulis (`rbytes`, `wbytes`) dan operasi IO (`rios`, `wios`) dari seluruh perangkat block storage terkait.
2. Penanganan kondisi slice:
   - Apabila slice belum di-instansiasi oleh systemd atau sedang tidak memiliki proses aktif, sistem tidak boleh menghasilkan error/kegagalan, melainkan mengembalikan status `exists: false` dengan metrik bernilai 0.
   - Fleksibilitas path lokasi slice systemd: mendeteksi lokasi root `/sys/fs/cgroup/panel-u<uid>.slice` maupun sub-hirarki `system.slice/panel-u<uid>.slice`.
3. Keamanan & Ketahanan (Safety & Resilience):
   - Validasi ketat terhadap rentang UID tenant (`10000..=59999`) menggunakan `TenantUid::parse`.
   - Parsing berkas kontrol yang tahan terhadap berkas rusak, kosong, atau format tak terduga tanpa pernah memicu panic atau `unwrap()`.

## Keputusan Arsitektur

### 1. Protokol Bertipe (`panel-proto`)
Penambahan tipe request, response, dan struktur data metrik di `panel-proto`:
- `ResourceReadRequest { uid: u32 }`
- `SliceResourceMetrics`:
  - `uid`, `slice_name`, `exists`
  - `cpu_usage_usec`, `cpu_user_usec`, `cpu_system_usec`, `cpu_nr_periods`, `cpu_nr_throttled`, `cpu_throttled_usec`
  - `memory_current_bytes`, `memory_peak_bytes`
  - `pids_current`, `pids_peak`
  - `io_read_bytes`, `io_write_bytes`, `io_read_ops`, `io_write_ops`
- `ResourceReadResponse { metrics: SliceResourceMetrics }`

### 2. Modul `CgroupReader` pada `panel-agent`
Dibuat di `crates/panel-agent/src/resource.rs`:
- Menerima konfigurasi path root cgroup v2 (default `/sys/fs/cgroup`, dapat dikustomisasi untuk pengujian).
- Helper `read_u64_file` membaca dan mem-parse nilai numerik tunggal secara aman tanpa panic.
- `parse_cpu_stat` memecah baris pasangan key-value standard cgroup v2.
- `parse_io_stat` mengurai baris multi-perangkat block storage dan menjumlahkan total throughput IO.

### 3. Integrasi `AgentClient` pada `panel-api`
- Fungsi `read_slice_resources(uid)` memanggil agent melalui Unix socket IPC dan mengembalikan `SliceResourceMetrics`.
- Pengujian integration test `tests/resource_ipc_test.rs` memverifikasi pembacaan cgroup aktif, fallback slice tidak aktif, dan penolakan UID di luar batas yang sah.

## Konsekuensi

- API dan UI Phase 2/Phase 9 dapat menampilkan metrik akurat langsung dari kernel cgroup v2.
- Kode aman dari crash dan mematuhi aturan strict zero unwrap di non-test code.
