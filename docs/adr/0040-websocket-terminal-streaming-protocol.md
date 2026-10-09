# ADR 0040: Protokol Streaming WebSocket Terminal & Manajemen Sesi Reconnect

## Status

Accepted

## Konteks

Pada task P6-T1, fondasi session manager PTY tingkat sistem di `panel-agent` telah dibangun dengan alokasi pseudoterminal (master/slave PTY), isolasi tenant Linux, attachment slice cgroup, dan confinement direktori kerja `cwd` aman anti-traversal.

Pada task P6-T2, Nusa Panel membutuhkan lapisan API dan streaming real-time dua arah antara browser web dan proses shell PTY di host agent:
1. **Streaming Output Berkepadatan Tinggi**: Mengalirkan data keluaran stdout/stderr terminal tanpa beban decoding/encoding berlebih ke browser client.
2. **Backpressure & Manajemen Buffer**: Mencegah penumpukan memori tak terkendali di server saat koneksi klien lambat atau browser dijeda.
3. **Reconnect Tahan Putus & Preservasi Scrollback**: Ketika koneksi jaringan terputus atau pengguna merefresh browser, sesi shell tidak boleh terhenti seketika, dan riwayat keluaran terminal terbaru harus dapat dipulihkan secara instan tanpa layar kosong (blank screen).
4. **Keamanan CSWSH & Otorisasi Ketat**: Mencegah eksploitasi Cross-Site WebSocket Hijacking (CSWSH) dan memastikan tenant A tidak dapat membaca atau mengirim input ke sesi terminal milik tenant B.
5. **Penegakan Kuota Paket Langganan (Plan Limits)**: Membatasi jumlah sesi terminal konkuren aktif yang dapat dibuka oleh setiap tenant berdasarkan konfigurasi paket (`max_terminal_sessions`).

## Keputusan Arsitektur

### 1. Desain Protokol WebSocket Bidirectional

Endpoint streaming ditempatkan pada rute `/ws/terminal/{session_id}` dengan protokol framing ganda:

- **Client -> Server (Inbound)**:
  - *Frame Biner (`Message::Binary`)*: Diperlakukan sebagai byte mentah stdin keyboard/keystroke yang langsung diteruskan ke PTY master tanpa overhead parsing JSON. Ini menghasilkan latensi terendah untuk interaksi xterm.js.
  - *Frame Teks JSON (`Message::Text`)*: Digunakan untuk pesan kontrol terstruktur:
    - `{ "type": "resize", "cols": u16, "rows": u16 }`: Mengubah ukuran jendela terminal via `pty_resize`.
    - `{ "type": "ping" }`: Pemeriksaan liveness dari klien.
    - `{ "type": "input", "data": "..." }`: Masukan teks/keystroke opsional via JSON.
  - *Frame Teks Mentah*: Diteruskan sebagai byte masukan fallback.
- **Server -> Client (Outbound)**:
  - *Frame Teks JSON*: Mengirim status inisialisasi dan event lifecycle:
    - `{ "type": "connected", "session_id": "...", "cols": ..., "rows": ..., "cwd": "...", "username": "..." }`
    - `{ "type": "pong" }`
    - `{ "type": "closed", "exit_code": ... }`
    - `{ "type": "error", "message": "..." }`
  - *Frame Biner (`Message::Binary`)*: Mengalirkan potongan data keluaran PTY (termasuk ANSI escape codes VT100) langsung ke socket klien.

### 2. Scrollback Ring Buffer Berbatas untuk Reconnect

Untuk memenuhi persyaratan reconnect tanpa kehilangan data tampilan:
- Sesi terminal mengalokasikan ring buffer scrollback berkapasitas tetap (default **64 KB** / 65.536 byte).
- Setiap chunk keluaran yang ditarik dari PTY agent disimpan ke dalam ring buffer ini dengan kebijakan sliding window: byte terlama di awal dibuang secara efisien jika total data melampaui kapasitas maksimum.
- Saat koneksi WebSocket baru terhubung (baik koneksi awal maupun reconnect setelah disconnect), seluruh isi ring buffer yang tersimpan langsung dikirimkan sebagai payload biner awal. xterm.js di sisi klien dapat seketika merender kembali riwayat sesi sebelumnya secara mulus.

### 3. Arsitektur Single Reader Task & Multi-subscriber Broadcast

Untuk menghindari race condition di mana beberapa klien/tab berebut membaca byte stream PTY dari agent:
- Hanya ada **satu background reader task** per sesi yang bertugas menarik data non-blocking dari agent (`agent_client.pty_read_output`).
- Data yang terbaca didistribusikan ke subscriber WebSocket aktif menggunakan channel `tokio::sync::broadcast`.
- Dengan pendekatan ini, pengguna dapat membuka multi-window/multi-tab pada sesi yang sama dengan output yang tersinkronisasi sempurna.
- Task reader menerapkan adaptive backoff: 5 ms saat terdapat aliran output deras, bertransisi ke 20 ms dan 50 ms saat idle untuk meminimalkan beban CPU.

### 4. Backpressure Handling via Bounded Channels

- Jalur pengiriman WebSocket dipisahkan ke dalam task tersendiri dengan bounded queue `tokio::sync::mpsc::channel(128)`.
- Jika klien WebSocket lambat mengonsumsi frame, pemanggilan `.send().await` menahan task listener/reader secara asinkron, sehingga mencegah konsumsi memori tak terbatas pada server API.

### 5. Keamanan CSWSH, RBAC, dan Validasi Kuota

- **Validasi Origin (CSWSH)**: Header `Origin` divalidasi via `validate_ws_origin`. Origin dari domain asing yang tidak sah ditolak seketika dengan `403 Forbidden`.
- **Autentikasi & RBAC**: Menggunakan cookie sesi panel (`panel_session`) atau token otentikasi. Akses lintas-tenant ditolak dengan `403 Forbidden`. Admin memiliki hak akses penuh untuk menginspeksi atau mengelola semua sesi.
- **Validasi Kuota Plan**: Saat `POST /api/v1/terminal/sessions`, jumlah sesi aktif yang belum ditutup diperiksa terhadap `plan.max_terminal_sessions`. Jika batas tercapai, permintaan ditolak dengan `429 Too Many Requests`.
- **Audit Logging**: Setiap pembukaan dan penutupan sesi dicatat ke audit log sistem (`terminal.session_create`, `terminal.session_close`).

## Konsekuensi

### Positif

- Protokol sangat efisien dengan latensi rendah untuk keystroke dan streaming biner.
- Pengalaman pengguna tahan gangguan jaringan: refresh browser tidak merusak proses di terminal dan riwayat terminal langsung muncul kembali berkat scrollback buffer.
- Arsitektur terisolasi aman dari serangan CSWSH dan eksploitasi multi-tenant.
- Memory footprint terkontrol ketat (kapasitas ring buffer terikat 64 KB, bounded channel 128 pesan).

### Mitigasi

- Sesi PTY yang ditinggalkan klien akan tetap aktif di agent sampai batas `idle_timeout` (15 menit) atau ditutup secara eksplisit via REST API `DELETE /api/v1/terminal/sessions/{id}`. Ini disengaja agar proses panjang seperti kompilasi kode atau instalasi paket tidak terputus saat browser ditutup.
