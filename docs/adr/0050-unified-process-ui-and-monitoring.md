# ADR 0050: Unified Process Management, Uptime Telemetry, and Application Live Logs

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.1, 5.5, dan Spesifikasi Fase 7 (P7-T5 "UI proses terpadu"), Nusa Panel memerlukan mekanisme terintegrasi untuk memantau dan mengelola seluruh proses aplikasi latar belakang (Node.js dan Python) yang berjalan di bawah unit sandboxed systemd (`site-<id>.service`):
1. **Pemantauan Terpadu Antar Runtime**: Administrator dan tenant membutuhkan visibilitas terhadap status proses (running, inactive, failed, reloading), PID, penggunaan memori (RAM cgroup), akumulasi waktu CPU, dan kesehatan health probe di satu tempat.
2. **Kalkulasi Uptime Akurat Tanpa Polling Drift**: Menampilkan durasi running secara real-time membutuhkan timestamp aktivasi yang presisi dari systemd dan sinkronisasi dengan monotonic clock kernel.
3. **Aliran Log Aplikasi Terpadu (Stdout / Stderr)**: Pengembang membutuhkan akses instan ke log stdout dan stderr aplikasi tanpa harus membuka terminal SSH atau journalctl manual, baik via REST polling maupun WebSocket live tailing streaming.
4. **Kontrol Siklus Hidup Proses Cepat**: Tombol kontrol mulai (start), henti (stop), mulai ulang (restart), dan deploy ulang (redeploy) harus tersedia langsung pada kartu proses situs maupun dashboard monitoring global.

## Keputusan Arsitektur

### 1. Endpoint REST Agregasi Proses Terpadu (`GET /api/v1/processes`)
- Modul baru `crates/panel-api/src/processes.rs` mengekspos endpoint `GET /api/v1/processes`.
- **Otorisasi Berbasis Tenant**:
  - Administrator menerima daftar proses dari seluruh situs aktif di node.
  - Tenant (regular user) hanya menerima daftar proses dari situs miliknya (`site.user_id == auth_user.id`).
- Mengembalikan DTO `ProcessListResponse` yang berisi:
  - `processes`: Array `ProcessItemDto` (`site_id`, `primary_domain`, `runtime`, `version`, `service_name`, `active`, `sub_state`, `pid`, `memory_bytes`, `cpu_usage_usec`, `uptime_secs`, `healthy`, `port_or_socket`, `probe_message`).
  - `total_active`: Jumlah total proses yang berstatus `active`.
  - `total_memory_bytes`: Akumulasi penggunaan memori cgroup dari seluruh proses yang aktif.

### 2. Kalkulasi Uptime Berbasis Systemd Monotonic Clock
- Systemd mengekspos properti `ActiveEnterTimestampMonotonic` (dalam mikrodetik sejak sistem boot) melalui `systemctl show`.
- Pada `panel-agent` (`node_site.rs` dan `python_site.rs`), nilai ini diekstraksi dan dibandingkan dengan `/proc/uptime`:
  ```rust
  let sys_uptime_secs = std::fs::read_to_string("/proc/uptime")
      .ok()
      .and_then(|content| content.split_whitespace().next()?.parse::<f64>().ok());
  if let Some(sys_uptime_secs) = sys_uptime_secs {
      let start_secs = mono_usec as f64 / 1_000_000.0;
      let diff = sys_uptime_secs - start_secs;
      if diff >= 0.0 {
          return diff as u64;
      }
  }
  mono_usec / 1_000_000
  ```
- Keuntungan: Tahan terhadap perubahan jam sistem (NTP adjustment / timezone skew) dan memberikan angka detik uptime yang konsisten.
- Di sisi frontend, interval timer 1 detik menginkrementasi counter uptime lokal secara langsung saat proses aktif, mencegah UI terasa statis.

### 3. Log Aplikasi Terpadu (`type=app`) dan Fallback Journalctl
- Menambahkan tipe log `app` pada `SiteLogsReadRequest` dan `crates/panel-api/src/logs.rs`.
- `crates/panel-agent/src/webserver.rs`:
  - Jalur berkas utama: `/var/log/panel/{username}/{domain}/app.log`.
  - Fallback aman: Jika berkas log aplikasi belum dibuat (misalnya aplikasi menulis ke stdout/stderr journald bawaan systemd), agent menjalankan `journalctl -u site-{clean_id}.service -n <lines> --no-pager -o cat` untuk mengekstrak baris log terakhir.
  - Menghubungkan log aplikasi langsung ke infrastruktur WebSocket live tailing (`/ws/logs/{site_id}?type=app`).

### 4. Komponen Frontend `UnifiedProcessCard` dan Tab `processes` pada `RuntimePage`
- **`UnifiedProcessCard.tsx`**:
  - Kartu monitoring proses terpadu untuk Node.js dan Python.
  - Menampilkan badge status (Aktif/Nonaktif, SubState systemd, Status Sehat/Belum Siap), PID, Uptime live counter, Penggunaan Memori, dan CPU kumulatif.
  - Kontrol siklus hidup (Mulai, Berhenti, Mulai Ulang, Deploy Ulang).
  - Terminal bawaan yang dapat diciutkan/dibuka (collapsible) untuk live streaming stdout/stderr aplikasi lengkap dengan filter pencarian dan selector baris.
  - Diintegrasikan di `SiteDetailPage.tsx` pada tab `node` dan `python`, serta tombol `Aplikasi (Stdout/Stderr)` pada tab `logs`.
- **Monitoring Global pada `RuntimePage.tsx`**:
  - Menambahkan tab `processes` ("Monitoring Proses Aplikasi").
  - Menampilkan kartu ringkasan (Total Proses Terkelola, Proses Aktif, Total Penggunaan RAM).
  - Dilengkapi filter real-time berdasarkan runtime (Node.js/Python), status aktivitas, dan pencarian teks (domain, PID, service).

## Konsekuensi
- **Positif**:
  - Administrator dan tenant memiliki satu dashboard seragam untuk melihat kesehatan seluruh proses latar belakang Node.js dan Python.
  - Membaca log stdout/stderr aplikasi tidak lagi membutuhkan akses SSH manual.
  - Metrik memori dan CPU diambil langsung dari cgroup kernel, mencegah drift informasi.
- **Batasan**:
  - Journalctl fallback dibatasi maksimal 1000 baris demi performa agent.
