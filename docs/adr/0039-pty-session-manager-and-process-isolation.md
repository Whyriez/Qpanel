# ADR 0039: PTY Session Manager di Agent dan Model Isolasi Eksekusi Shell Tenant

## Status
Accepted

## Konteks
Pada Phase 6 (Web Terminal dan SSH), task **P6-T1** menuntut tersedianya fondasi runtime terminal yang aman di `panel-agent`:
1. **Spawn Login Shell sebagai UID Tenant**: Proses login shell tidak boleh dijalankan sebagai root atau user sistem panel, melainkan harus dijalankan secara ketat sebagai UID dan GID Linux tenant (`p_<slug>`).
2. **Isolasi Slice Tenant (cgroup v2)**: Shell harus berada di dalam slice resource tenant (`panel-u<uid>.slice`) agar konsumsi RAM, CPU, dan batas proses (`TasksMax`) dapat tertahan dan tidak mengganggu tenant lain atau host OS.
3. **Validasi Direktori Kerja (`cwd`) Terkurung**: Direktori kerja yang diminta saat membuka sesi terminal harus divalidasi tidak boleh keluar dari home directory tenant (`/home/p_<slug>`). Percobaan directory traversal (`..`) atau symlink escape harus ditolak.
4. **Hardening Eksekusi**: Penerapan `PR_SET_NO_NEW_PRIVS` untuk mencegah eskalasi hak akses SUID/capabilities, serta rlimit untuk batas proses dan file descriptor.
5. **Manajemen Pseudoterminal (PTY)**: Mengalokasikan master/slave PTY (POSIX `openpty`), menangani I/O non-blocking, perubahan ukuran layar terminal (`TIOCSWINSZ` resize), dan penanganan terminasi (`SIGHUP`/`SIGTERM`/`SIGKILL`).
6. **Batas Sesi Konkuren & Idle Reaper**: Membatasi jumlah sesi terminal bersamaan per tenant (kuota paket/plan) dan membersihkan sesi yang ditinggalkan tidak aktif (*idle timeout*).

## Keputusan Arsitektur

### 1. Desain Abstraksi PTY (`PtyBackend` & `PtyHandle`)
- Modul [crates/panel-agent/src/pty/backend.rs](file:///e:/Latihan/Panel%20Hosting/Vpanel/crates/panel-agent/src/pty/backend.rs) memisahkan runtime OS PTY menjadi dua implementasi:
  1. `UnixPtyBackend`: Menggunakan syscalls POSIX via `libc`:
     - Alokasi master/slave pair dengan `openpty`.
     - Master fd diatur menjadi `O_NONBLOCK`.
     - `fork` child process:
       - Di child: `setsid()`, `ioctl(slave, TIOCSCTTY)`, redirect stdin/stdout/stderr ke slave fd.
       - Hardening: `prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)`, `setrlimit(RLIMIT_NPROC)`, `setrlimit(RLIMIT_NOFILE)`.
       - Pindah direktori ke `cwd` yang telah divalidasi aman.
       - Drop privileges: `initgroups`, `setresgid(gid, gid, gid)`, `setresuid(uid, uid, uid)`.
       - Eksekusi login shell dengan environment variables bersih (`HOME`, `USER`, `LOGNAME`, `SHELL`, `TERM`, `PATH`).
     - Di parent: Pasang PID child ke `/sys/fs/cgroup/panel-u<uid>.slice/cgroup.procs` bila cgroup v2 tersedia.
  2. `MockPtyBackend`: Engine terminal in-memory yang mengemulasi interaktivitas shell (banner, echo, exit, resize) untuk pengujian unit dan kompilasi lintas platform (Windows dev host).

### 2. Validasi Path Confinement (`resolve_confined_cwd`)
- Di [crates/panel-agent/src/pty/manager.rs](file:///e:/Latihan/Panel%20Hosting/Vpanel/crates/panel-agent/src/pty/manager.rs):
  - Home base default `/home/<username>` menjadi jangkar confinement.
  - Jika path relatif diberikan, komponen `..` ditolak segera dengan `AgentError::SecurityViolation`.
  - Jika path absolut diberikan, harus berupa prefix `/home/<username>`.
  - Canonical path dari direktori target diverifikasi tetap diawali canonical path home tenant guna menolak symlink escape.

### 3. Kuota Sesi Konkuren dan Idle Timeout Reaper
- Sesi aktif dilacak per-tenant di dalam `tenant_sessions: Arc<RwLock<HashMap<String, HashSet<String>>>>`.
- Sebelum alokasi PTY baru, jumlah sesi aktif diperiksa terhadap batas `max_sessions` (default 5 atau sesuai plan tenant). Jika kuota tercapai, request ditolak dengan error informatif.
- Setiap sesi melacak timestamp aktivitas presisi milidetik (`last_active_at`).
- Background worker `start_idle_reaper(interval)` berjalan secara berkala:
  - Memeriksa sesi yang melampaui `idle_timeout` (default 15 menit / 900 detik) atau yang child process-nya telah exit.
  - Mengirim sinyal terminasi, menutup file descriptor, dan membebaskan entri dari memory map.

### 4. Protokol IPC (`panel-proto`) & Klien API (`panel-api`)
- Protokol bertipe ditambahkan ke [crates/panel-proto/src/lib.rs](file:///e:/Latihan/Panel%20Hosting/Vpanel/crates/panel-proto/src/lib.rs):
  - `PtyOpen(PtyOpenRequest)` → `PtyOpen(PtyOpenResponse)`
  - `PtyInput(PtyInputRequest)` → `PtyInput(PtyInputResponse)`
  - `PtyReadOutput(PtyReadOutputRequest)` → `PtyReadOutput(PtyReadOutputResponse)`
  - `PtyResize(PtyResizeRequest)` → `PtyResize(PtyResizeResponse)`
  - `PtyClose(PtyCloseRequest)` → `PtyClose(PtyCloseResponse)`
  - `PtyList(PtyListRequest)` → `PtyList(PtyListResponse)`
- Klien IPC di [crates/panel-api/src/agent_client.rs](file:///e:/Latihan/Panel%20Hosting/Vpanel/crates/panel-api/src/agent_client.rs) mengimplementasikan method-method bertipe yang siap dikonsumsi oleh WebSocket handler pada task P6-T2.

## Konsekuensi
- **Positif**:
  - Privilege separation terpenuhi penuh: API tidak pernah root, agent mengeksekusi shell dengan drop privilege ke tenant UID/GID.
  - Sesi terminal terisolasi di dalam slice cgroup v2 tenant, menjaga stabilitas server dari fork bomb dan kehabisan memori.
  - Path traversal dan symlink escape tertolak di muka sebelum shell dibuka.
  - Sesi tidak aktif dibersihkan otomatis, menjamin footprint RAM tetap hemat (0 RAM leaked).
  - Teruji dengan unit tests di `panel-agent` dan integration tests di `panel-api`.
