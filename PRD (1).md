# PRD — Nusa Panel (working title)

> Web hosting control panel yang ringan, cepat, dan terisolasi per-tenant, ditulis dengan Rust.
> Versi dokumen: 0.1 (draft) · 3 Oktober 2026 · Bahasa: Indonesia (istilah teknis tetap English)

"Nusa" hanya working title. Nama crate/binary di dokumen ini memakai prefix generik `panel-*` supaya gampang di-rename.

---

## Progress Tracker

Agent **wajib** mencentang checklist ini dan menyalinnya ke `docs/PROGRESS.md` setiap selesai satu task.

- [x] Phase 0 — Fondasi & tooling
- [x] Phase 1 — Core panel (auth, DB, job, audit)
- [ ] Phase 2 — Tenant user & isolasi
- [ ] Phase 3 — Site engine: static + Nginx + domain + SSL
- [ ] Phase 4 — PHP multi-versi terisolasi
- [ ] Phase 5 — File manager custom
- [ ] Phase 6 — Web terminal & SSH
- [ ] Phase 7 — Node.js, Python, Reverse proxy
- [ ] Phase 8 — Kelengkapan detail project
- [ ] Phase 9 — Analytics mendalam
- [ ] Phase 10 — Performance, hardening, installer, rilis

---

## 0. Cara Memakai Dokumen Ini (untuk AI Agent)

Dokumen ini dirancang supaya dikerjakan **bertahap**, bukan sekaligus.

**Aturan kerja**

1. Kerjakan **satu task** (ID `P<fase>-T<nomor>`, contoh `P3-T4`) per sesi atau per PR. Jangan loncat fase dan jangan mengerjakan task fase berikutnya "sekalian".
2. Sebelum memulai fase baru, baca bagian 3 (Stack), 4 (Arsitektur), 6 (Keamanan), dan bagian fase tersebut.
3. **Definition of Done** setiap task:
   - Kode compile, `cargo fmt --check` dan `cargo clippy -- -D warnings` bersih.
   - Ada test (unit dan/atau integration) yang membuktikan acceptance criteria task.
   - Untuk perubahan UI: lolos `tsc`, lint, dan ada minimal satu test komponen atau e2e.
   - `docs/PROGRESS.md` diperbarui, dan jika ada keputusan teknis baru, tulis ADR di `docs/adr/`.
4. **Wajib develop dan test di VM** (Multipass / Lima / Vagrant, Ubuntu 24.04). `panel-agent` mengubah sistem (user, nginx, systemd, ACL). Jangan pernah menjalankannya di mesin host/laptop developer.
5. Jangan menjalankan `panel-api` sebagai root, walau hanya untuk "mempercepat dev". Pemisahan privilege adalah aturan keras (lihat 4.3).
6. Dilarang: membangun command shell dengan string interpolation, memakai `unwrap()`/`expect()` di kode non-test, menerima path/command mentah dari API di agent, dan menambah dependency besar tanpa ADR.
7. Jika ada hal yang ambigu: pilih default aman yang tertulis di PRD, catat di ADR, lanjutkan. Jangan menambah fitur di luar scope task.
8. Commit kecil dengan conventional commits (`feat(agent): ...`, `fix(web): ...`). Satu task boleh beberapa commit.
9. Jika sebuah task terlalu besar untuk satu sesi, pecah menjadi sub-task (`P4-T3a`, `P4-T3b`) di `docs/PROGRESS.md` sebelum mengerjakannya.

---

## 1. Ringkasan & Tujuan

### 1.1 Masalah

Panel hosting populer (cPanel, CyberPanel, CloudPanel) menjadi referensi fitur. Produk ini ingin menggabungkan tiga hal yang jarang hadir bersamaan:

- **Footprint RAM kecil** supaya VPS murah tetap lega untuk website pelanggan.
- **Isolasi tenant yang ketat** (user, file, proses, resource, versi PHP/runtime).
- **UI modern dengan analytics yang dalam**, bukan sekadar grafik CPU/RAM.

### 1.2 Tujuan v1

| # | Tujuan | Ukuran keberhasilan |
|---|--------|---------------------|
| G1 | Hosting multi-runtime: static, PHP, Node.js, Python, reverse proxy | Semua tipe bisa dibuat, di-deploy, dan HTTPS aktif dari UI |
| G2 | PHP multi-versi, terisolasi per tenant | Dua tenant memakai versi PHP berbeda bersamaan, tidak bisa saling baca |
| G3 | File manager custom | Upload resumable, permission, editor, terminal di folder aktif |
| G4 | Manajemen user terisolasi | Test isolasi lintas-tenant lulus 100% |
| G5 | Detail project lengkap | Domain, SSL, env, cron, log, redirect, deploy, DB, backup |
| G6 | Cepat dan stabil | Target di bagian 9 tercapai dan terdokumentasi lewat benchmark |
| G7 | Analytics mendalam | Traffic, latency percentile, error, cache, bot, geo, insight otomatis |

### 1.3 Persona

- **Server Admin**: pemilik server, mengelola semua tenant, plan, runtime, dan pengaturan sistem.
- **Tenant / Developer**: mengelola situs miliknya saja (file, domain, deploy, log, analytics).
- **Reseller**: ditunda ke backlog. Model data jangan menutup kemungkinan ini.

---

## 2. Scope

### 2.1 In scope (v1)

Hosting runtime, tenant management, plan/quota, file manager, web terminal + SSH, SSL (Let's Encrypt), log, cron, env vars, redirect/rule, Git deploy, database (MariaDB/PostgreSQL provisioning), backup lokal, analytics, tuning/optimizer, installer.

### 2.2 Out of scope (v1, lihat backlog di bagian 12)

Email server, DNS server, WAF penuh, Docker apps, multi-server, reseller/billing, installer 1-click (WordPress dll.), FTP biasa (pakai SFTP).

### 2.3 Platform target

- Ubuntu 22.04 / 24.04 LTS dan Debian 12, arsitektur x86_64 dan aarch64.
- systemd >= 249, cgroup v2, kernel >= 5.6 (butuh `openat2`).
- Spesifikasi server: minimum 1 vCPU / 1 GB RAM, rekomendasi 2 vCPU / 2 GB.

---

## 3. Keputusan Stack

Semua baris di bawah adalah **default yang direkomendasikan**. Yang ditandai ⚠ masih perlu konfirmasi pemilik produk (lihat bagian 13).

| Layer | Pilihan | Alasan | Alternatif (ditolak/ditunda) |
|-------|---------|--------|------------------------------|
| Bahasa backend | **Rust** (stable, edition 2024) | RAM kecil, tanpa GC, aman memori | — |
| HTTP | **Axum + Tokio + tower-http** | Standar de-facto ekosistem Rust, WebSocket/SSE bawaan | Actix (lebih rumit untuk tower middleware) |
| DB panel | **SQLite (WAL) via `sqlx`** | Tanpa daemon tambahan, hemat RAM, backup = salin file | PostgreSQL (daemon + RAM ekstra). Pakai layer repository agar bisa pindah |
| DB analytics | **SQLite terpisah** (`analytics.db`), rollup berlapis | Satu writer, batch insert, retensi terkontrol | ClickHouse/Timescale (terlalu berat). DuckDB: evaluasi nanti |
| Web server ⚠ | **Nginx** (paket resmi nginx.org, stable ≥ 1.26) | Matang, hemat RAM, HTTP/2 + HTTP/3, reload graceful | Caddy (auto-TLS enak, tapi RAM dan model config beda), OpenLiteSpeed, proxy custom Rust/Pingora (risiko tinggi, ditunda) |
| PHP | **PHP-FPM multi-versi** dari repo Sury (Debian) / `ppa:ondrej/php` (Ubuntu) | Versi paralel siap pakai | Build dari source (maintenance berat) |
| Node.js | **Binary resmi** nodejs.org di `/opt/panel/runtimes/node/<ver>`, `corepack` untuk pnpm/yarn | Versi paralel, verifikasi checksum, tanpa nvm | nvm/fnm (state per-user, susah dikontrol) |
| Python | **`uv`** untuk install versi + venv per site; gunicorn/uvicorn | Cepat, versi paralel, reproducible | pyenv, deadsnakes |
| Supervisor | **systemd** (via D-Bus `zbus`, fallback `systemctl` di balik trait `ServiceManager`) | Cgroup, sandbox, restart policy gratis | PM2/supervisord (RAM lebih besar, tanpa cgroup) |
| Isolasi | **User Linux per tenant + systemd slice (cgroup v2) + sandbox properties + ACL** | Terbukti, efisien | Container per tenant (lebih berat, ditunda) |
| TLS | **ACME HTTP-01** via crate `instant-acme`, DNS-01 menyusul | Tanpa dependency certbot | certbot/acme.sh (proses eksternal) |
| Template config | **`minijinja`** | Ringan, auto-escape bisa dikontrol | Tera/Askama |
| Frontend ⚠ | **React 19 + TypeScript + Vite** | Ekosistem terbesar untuk komponen file manager, tabel, terminal | Vue 3 (juga oke), SvelteKit, Leptos (ekosistem komponen belum cukup) |
| UI kit | **Tailwind CSS v4 + shadcn/ui (Radix)**, `lucide-react`, `cmdk`, `sonner` | Aksesibel, bisa di-theme total, tanpa lock-in | MUI/Ant (berat, susah dibuat unik) |
| Data FE | **TanStack Router + Query + Table + Virtual**, React Hook Form + Zod, Zustand (secukupnya) | Standar modern | Redux |
| Editor / Terminal | **CodeMirror 6**, **xterm.js** | CM6 jauh lebih ringan dari Monaco | Monaco (berat) |
| Chart | **uPlot** (time-series), **Apache ECharts** (heatmap/geo/treemap, lazy-load) | uPlot sangat kecil dan cepat | Recharts (cukup, tapi lebih berat untuk data padat) |
| Kontrak API | **OpenAPI 3.1** dari `utoipa`, TS client digenerate (`orval` / `openapi-typescript`) | Type-safe end-to-end | Tulis tangan |
| Realtime | **WebSocket** (terminal, live log, live metrics), **SSE** (progress job) | Sederhana | gRPC-web |
| Distribusi | `panel-api` meng-embed `web/dist` (`rust-embed`), satu binary + `panel-agent`; **tanpa Node.js di server produksi untuk panel itu sendiri** | Deploy mudah, RAM kecil | Container |
| Auth | Session cookie `HttpOnly+Secure+SameSite=Strict` + CSRF, **Argon2id**, **TOTP**, API token (disimpan hash) | Standar kuat | JWT stateless (susah revoke) |
| Secrets | **XChaCha20-Poly1305**; kunci master hanya terbaca agent | Env var tenant terenkripsi at rest | Plaintext |
| Observability | `tracing` + JSON log, `/healthz`, endpoint metrics opsional | | |

**Crate Rust yang diharapkan** (boleh diganti dengan ADR): `axum`, `tokio`, `tower-http`, `sqlx`, `serde`, `utoipa`, `tracing`, `thiserror`, `anyhow` (hanya di tepi/main), `argon2`, `totp-rs`, `rustix`, `cap-std`, `zbus`, `minijinja`, `instant-acme`, `notify`, `maxminddb`, `idna`, `chacha20poly1305`, `tokio-util`, `tar`/`flate2`/`zip` (extract aman).

**Build profile rilis**: `lto = "fat"`, `codegen-units = 1`, `strip = true`, evaluasi `panic = "abort"` dan allocator `mimalloc`/`jemalloc` di Phase 10.

---

## 4. Arsitektur

### 4.1 Gambaran umum

```
 Browser ──HTTPS──► Nginx  (:80/:443 situs tenant, :8443 panel)
                      │ proxy_pass 127.0.0.1:7700
                      ▼
                 panel-api   (user `panel`, NON-root) ──► SQLite (panel.db)
                      │ Unix socket /run/panel/agent.sock
                      │ (SO_PEERCRED, RPC bertipe, tanpa command mentah)
                      ▼
                 panel-agent (root, permukaan serangan minimal)
                      │
   ┌─────────────┬────┴────────────┬──────────────┬───────────────┐
   ▼             ▼                 ▼              ▼               ▼
 nginx conf   systemd units     useradd/ACL/    fs-worker       log-tailer +
 (atomic      (php-fpm/node/    quota/cgroup    (jalan sebagai  metrics collector
  apply)       python/cron)     slice           UID tenant)     (→ analytics.db)
```

### 4.2 Komponen (Cargo workspace)

```
panel/
├─ Cargo.toml                 # workspace
├─ crates/
│  ├─ panel-api/              # HTTP API, auth, RBAC, job runner (non-root)
│  ├─ panel-agent/            # daemon root: eksekusi operasi sistem
│  ├─ panel-proto/            # tipe RPC API↔agent (serde), versi protokol
│  ├─ panel-core/             # domain types, newtype validator, error
│  ├─ panel-db/               # migrasi + repository
│  ├─ panel-webserver/        # renderer config nginx + apply atomic
│  ├─ panel-runtime/          # manajer PHP / Node / Python / proxy
│  ├─ panel-fs/               # operasi file aman (path-confined)
│  ├─ panel-analytics/        # tailer, parser, aggregator, query
│  └─ panel-cli/              # `panelctl` (admin, doctor, backup, upgrade)
├─ web/                       # React + Vite + TS
├─ templates/                 # nginx / php-fpm / systemd (minijinja)
├─ installer/                 # install.sh, uninstall, upgrade
├─ tests/                     # integration + isolation + e2e (jalan di VM)
└─ docs/                      # PROGRESS.md, adr/, security-model.md
```

| Komponen | Hak akses | Tanggung jawab |
|----------|-----------|----------------|
| `panel-api` | user `panel` | REST/WS, auth, RBAC, validasi input, simpan state, kirim command ke agent |
| `panel-agent` | root | Satu-satunya yang menyentuh sistem. Validasi ulang semua input |
| fs-worker | UID tenant (drop privilege) | Semua operasi file tenant dan PTY, tidak pernah sebagai root |
| log-tailer | root (baca log) → tulis agregat | Mem-parse log nginx per site, kirim agregat ke `analytics.db` |

### 4.3 Aturan keras privilege separation

1. API **tidak pernah** root dan tidak punya akses tulis ke `/etc`, `/home` tenant, atau systemd.
2. Agent **hanya** menerima enum command bertipe (`Request::UserCreate{..}`, `Request::SiteApply{..}`). Tidak ada `Exec(String)`, tidak ada path bebas.
3. Setiap field divalidasi dengan **newtype** (`Username`, `Domain`, `SiteId`, `SafeRelPath`, `Port`, `PhpVersion`) di `panel-core`, dipakai oleh API **dan** agent (agent tetap memvalidasi ulang).
4. Menjalankan proses eksternal: selalu `Command::new(..).arg(..)`, tidak pernah lewat shell. Daftar binary yang boleh dipanggil di-allowlist.
5. Peer socket diverifikasi lewat `SO_PEERCRED` (hanya UID `panel`). Socket `0660 root:panel`.
6. Operasi pada data tenant (file, shell, build) berjalan sebagai **UID tenant** dengan `no_new_privs`, bukan root.
7. Unit systemd agent di-harden semaksimal mungkin tanpa mematahkan fungsi (target `systemd-analyze security` ≤ 5.0, lihat Phase 10).
8. Setiap command agent menulis **audit record** (siapa, apa, hasil).
9. Transport RPC didesain agnostik (Unix socket sekarang, bisa mTLS di masa depan untuk multi-server).

**Protokol RPC**: frame length-delimited (`tokio-util`), payload JSON bertipe (debuggable), field `proto_version`, `request_id`, timeout per command. Operasi streaming (output job, PTY, tail log) memakai stream channel terpisah per sesi yang dibuka lewat `OpenStream`.

### 4.4 Model isolasi tenant

Batas isolasi = **satu tenant (user panel)**. Semua situs milik tenant yang sama berbagi UID (seperti shared hosting klasik). Isolasi antar-situs dalam satu tenant memakai `open_basedir` dan struktur direktori, bukan batas keamanan keras.

| Lapisan | Mekanisme |
|---------|-----------|
| Identitas | 1 tenant = 1 user Linux `p_<slug>` (UID 10000–59999), tanpa password login, shell sesuai kebijakan |
| File | Home `0700` + **ACL** `u:<nginx-user>:--x` pada home; docroot `0750` + ACL baca untuk nginx. Tenant lain tidak bisa traverse |
| Proses | Semua service tenant `User=p_<slug>`, `Slice=panel-u<uid>.slice`. `/proc` di-mount `hidepid=2` (opsi installer) |
| Resource | Slice per tenant: `MemoryMax/High`, `CPUQuota`, `TasksMax`, `IOWeight`/bandwidth, dari plan |
| Sandbox service | `NoNewPrivileges`, `ProtectSystem=strict`, `PrivateTmp`, `PrivateDevices`, `ProtectKernel*`, `ProtectControlGroups`, `RestrictAddressFamilies`, `TemporaryFileSystem=/home:ro` + `BindPaths=/home/p_<slug>` (home tenant lain tidak terlihat sama sekali) |
| Disk | Quota filesystem (ext4 `usrquota` / XFS), fallback scan terjadwal bila FS tidak mendukung |
| Network | Loopback bersama. Default Python pakai Unix socket; Node pakai `127.0.0.1:<port>`; filter nftables per-UID di Phase 10 |
| PHP | Satu **php-fpm master per (tenant, versi PHP)** yang berjalan sebagai user tenant, satu pool per site; `open_basedir`, `disable_functions`, `tmp/session/upload` per site |

**Kenapa php-fpm master per (tenant, versi)** dan bukan satu master global per versi: master berjalan sebagai user tenant sehingga seluruh workernya otomatis ikut sandbox + cgroup tenant, dan RAM tetap hemat karena `pm = ondemand` (tanpa worker saat idle). Satu tenant dengan satu versi PHP = satu master.

### 4.5 Layout direktori di server

```
/opt/panel/                      # binary, runtimes (root-owned, read-only bagi tenant)
├─ bin/panel-api  bin/panel-agent  bin/panelctl
└─ runtimes/{node,python}/<versi>/
/etc/panel/
├─ panel.toml                    # config panel
├─ secret.key                    # 0600 root, hanya dibaca agent
├─ nginx/{sites,snippets}/       # config hasil render (root-owned)
├─ php/<uid>-<ver>/              # php-fpm.conf + pool.d/<site>.conf
└─ env/site-<id>.env             # 0600 root, dipakai EnvironmentFile=
/var/lib/panel/{panel.db,analytics.db,certs/,acme-webroot/,trash-index/}
/var/log/panel/                  # log panel + log nginx per site (root-owned, ACL baca tenant)
/run/panel/{agent.sock,php/}
/home/p_<slug>/
├─ sites/<domain>/{public|app}/  # docroot / root aplikasi
├─ tmp/  .ssh/  .panel-trash/
```

Config yang di-generate **tidak pernah** berada di direktori yang bisa ditulis tenant.

### 4.6 Model data (ringkasan)

`users`(role, linux_username, linux_uid, plan_id, status) · `plans`(limit CPU/RAM/disk/inode/proses/jumlah site/DB/…) · `sites`(user_id, kind, docroot, runtime, version, status, settings JSON) · `domains`(site_id, name, is_primary, redirect_mode) · `certificates` · `site_env`(terenkripsi) · `cron_jobs` · `rules`(redirect/header/basic-auth/IP) · `deployments` · `databases` · `backups` · `ssh_keys` · `jobs` · `audit_logs` · `sessions` · `api_tokens` · `notifications` · `settings`.
Analytics di `analytics.db`: `sys_metrics_{10s,1m,1h,1d}`, `site_stats_{1m,1h,1d}`, dimensi top-K, `events` (deploy marker, insight).

### 4.7 Job system, apply atomic, reconcile

- **Job**: operasi panjang (install runtime, build, backup, extract) dicatat di tabel `jobs` (`queued → running → succeeded|failed|cancelled`), punya progress dan log, dialirkan ke UI lewat SSE, retry dengan backoff, **idempoten**, dan bertahan saat restart (job `running` yang terputus dilanjutkan atau ditandai gagal secara eksplisit).
- **Apply atomic** untuk config nginx/php/systemd: render ke staging → validasi (`nginx -t`, `php-fpm -t`, `systemd-analyze verify`) → swap → reload graceful → **rollback otomatis** jika gagal. Reload di-debounce (gabungkan perubahan dalam ±500 ms).
- **Reconcile**: agent menyediakan operasi `apply(resource)` yang idempoten. `panelctl reconcile` membandingkan desired state (DB) dengan kondisi aktual dan memperbaiki drift (Phase 10).

---

## 5. Spesifikasi Fitur

### 5.1 Hosting multi-runtime

Setiap **site** punya `kind`: `static`, `php`, `nodejs`, `python`, `proxy`.

| Kind | Cara jalan | Konfigurasi utama |
|------|------------|-------------------|
| `static` | Nginx langsung | docroot, index, cache header, SPA fallback (`try_files … /index.html`) |
| `php` | Nginx → FastCGI socket → pool php-fpm milik tenant | versi PHP, extension, ini, preset (generic / WordPress / Laravel / custom), `pm` |
| `nodejs` | systemd service `User=tenant` → Nginx `proxy_pass` | versi Node, start/build/install command, port atau socket, env, WebSocket |
| `python` | systemd service (gunicorn/uvicorn) via Unix socket → Nginx | versi Python, venv, WSGI/ASGI entrypoint, jumlah worker, static/media mapping |
| `proxy` | Nginx reverse proxy murni | upstream (http/https/unix), preserve host, WebSocket, timeout, buffering, header |

**PHP (inti fitur)**

- Versi paralel dari repo Sury (7.4 sampai versi stabil terbaru yang tersedia saat implementasi). Service bawaan `phpX.Y-fpm.service` di-**mask**; panel memakai binary `php-fpmX.Y` dengan config sendiri.
- Ganti versi per site tanpa downtime berarti: buat pool di master versi tujuan → ubah upstream nginx → reload → hapus pool lama; **rollback otomatis** bila health check gagal.
- Default aman: `pm = ondemand`, `pm.process_idle_timeout = 10s`, `pm.max_children` dari plan, `open_basedir` = docroot + tmp site, `disable_functions` hardened (editable admin), `session.save_path` dan `upload_tmp_dir` per site, `slowlog` aktif.
- Extension dikelola dari daftar kurasi (mbstring, curl, gd, intl, zip, mysqli/pdo_mysql, pgsql, redis, imagick, opcache, xml, bcmath, soap, sqlite3, …).
- Pengaturan yang bisa diubah tenant: `memory_limit`, `upload_max_filesize`, `post_max_size`, `max_execution_time`, `max_input_vars`, `display_errors`, `date.timezone`, opsi OPcache. Batas atasnya ditentukan plan. Edit raw `php.ini` hanya untuk admin.

**Node.js dan Python**

- Runtime di `/opt/panel/runtimes/...`, download diverifikasi (SHA256, plus signature bila tersedia). Versi yang sedang dipakai tidak bisa dihapus.
- Perintah install/build dijalankan sebagai job **sebagai UID tenant**, di dalam slice tenant, dengan timeout dan output streaming.
- Service: `Restart=on-failure`, `RestartSec`, `LimitNOFILE`, env dari `EnvironmentFile`, log ke journald dan file site.

**Contoh unit service hasil generate (Node.js)**

```ini
[Unit]
Description=Site 42 (nodejs)
After=network.target

[Service]
User=p_alice
Group=p_alice
Slice=panel-u10001.slice
WorkingDirectory=/home/p_alice/sites/example.com/app
EnvironmentFile=-/etc/panel/env/site-42.env
ExecStart=/opt/panel/runtimes/node/22.11.0/bin/node server.js
Restart=on-failure
RestartSec=2
LimitNOFILE=65535
NoNewPrivileges=true
ProtectSystem=strict
ReadWritePaths=/home/p_alice
TemporaryFileSystem=/home:ro
BindPaths=/home/p_alice
PrivateTmp=true
PrivateDevices=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
```

**Contoh server block (PHP) yang diharapkan**

```nginx
server {
    listen 80;
    listen [::]:80;
    server_name example.com www.example.com;
    include /etc/panel/nginx/snippets/acme.conf;   # /.well-known/acme-challenge/
    location / { return 301 https://example.com$request_uri; }
}
server {
    listen 443 ssl;
    listen [::]:443 ssl;
    listen 443 quic;                # `reuseport` hanya boleh sekali per ip:port
    http2 on;
    server_name example.com;
    root /home/p_alice/sites/example.com/public;
    access_log /var/log/panel/p_alice/example.com/access.log panel_json;
    error_log  /var/log/panel/p_alice/example.com/error.log warn;
    include /etc/panel/nginx/snippets/security-headers.conf;
    include /etc/panel/nginx/snippets/deny-sensitive.conf;   # .env, .git, dll.
    location / { try_files $uri $uri/ /index.php?$args; }
    location ~ \.php$ {
        fastcgi_pass unix:/run/panel/php/p_alice-8.3.sock;
        include /etc/panel/nginx/snippets/fastcgi-params.conf;
    }
}
```

**Reverse proxy: guard wajib**

- Default hanya boleh upstream loopback (`127.0.0.1`, `::1`, Unix socket).
- Upstream eksternal butuh flag admin per plan/site. Selalu blok `169.254.0.0/16` (metadata cloud) dan port internal panel.
- Header `X-Forwarded-*`, `Host`, WebSocket upgrade, timeout, dan buffering bisa dikonfigurasi dan divalidasi.

### 5.2 Manajemen user terisolasi

- **Role**: `admin` (operasi sebagai root lewat agent), `user` (tenant). Dasar RBAC siap menambah `reseller`.
- **Siklus hidup**: `active` → `suspended` → `deleting`. Suspend = kunci login, stop semua service tenant, nginx menampilkan halaman 503 khusus. Hapus = konfirmasi mengetik nama user, retensi opsional (default 7 hari) sebelum purge.
- **Plan** (paket): batas disk, inode, RAM, CPU, jumlah proses, jumlah site/domain/DB/cron, jumlah sesi terminal, batas upload, izin shell (`none` / `sftp` / `full`), izin upstream eksternal, versi PHP/runtime yang boleh dipakai.
- **Impersonate** ("login as user") untuk admin: wajib audit log dan banner jelas di UI.
- **Keamanan akun**: Argon2id, TOTP + recovery codes, daftar sesi aktif dengan revoke, API token (scope + expiry), rate limit + lockout login.
- Username Linux di-generate (`p_<slug>`), tidak bisa dipilih bebas, divalidasi regex ketat, tidak boleh bentrok dengan user sistem.

### 5.3 File manager (custom)

Dibangun dari nol, bukan embed tool pihak ketiga.

**Fungsional**

- Navigasi: breadcrumb, pohon folder, tampilan list dan grid, sort, filter, pencarian nama (dan isi file, dibatasi), virtualized list (10k+ entri lancar).
- Operasi: buat, rename, pindah, salin (dengan progress), hapus ke **trash** (restore, retensi, hitung quota), multi-select (shift/ctrl, rubber band), drag-drop pindah dan drag-drop upload, context menu, shortcut keyboard.
- **Upload**: resumable dan chunked (kompatibel protokol tus atau varian sendiri dengan `Upload-Offset`), paralel, pause/resume, cek quota di muka, finalize atomic (tulis ke tmp lalu `rename`), batas ukuran dari plan. Tray upload dengan progress.
- **Download**: Range, ETag, streaming. Download folder sebagai zip streaming.
- **Arsip**: compress (zip, tar.gz) dan extract dengan proteksi zip-slip, symlink keluar root, zip-bomb (batas rasio/jumlah/ukuran). Jalan sebagai job.
- **Permission**: tampil owner/mode; chmod lewat grid centang + input oktal; rekursif dengan mode file dan folder terpisah; preset aman (755/644); peringatan untuk 777. chown dibatasi (tenant tidak bisa mengubah pemilik).
- **Editor**: CodeMirror 6, syntax by extension, batas ukuran (default 5 MB, lebih besar = read-only), deteksi konflik (mtime/ETag) saat simpan. Preview gambar/PDF/video/teks.
- **Quota meter** dan tombol **"Buka terminal di sini"** pada folder aktif.

**Keamanan (kritikal)**

- Semua operasi lewat library `panel-fs` yang **path-confined**: resolusi dengan `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)` atau `cap-std`; tidak pernah membangun path dengan string concat.
- Dijalankan oleh **fs-worker dengan UID/GID tenant** (`setgroups`, `setgid`, `setuid`, `PR_SET_NO_NEW_PRIVS`, tutup fd, rlimit). Bukan root.
- Test adversarial wajib: `..`, symlink keluar root, race (TOCTOU), null byte, normalisasi unicode, path sangat panjang, nama file aneh.

### 5.4 Terminal dan SSH

- **Web terminal**: xterm.js + WebSocket ke PTY yang di-spawn agent **sebagai UID tenant**, di dalam slice tenant, `cwd` = folder aktif di file manager. Multi-tab, resize, heartbeat, scrollback terbatas saat reconnect, idle timeout, batas sesi per plan, cek `Origin`, audit awal/akhir sesi (isi keystroke tidak direkam secara default).
- **SSH nyata** (opsional per plan): manajemen SSH key per tenant (file `authorized_keys` dikelola panel di lokasi root-owned), kebijakan shell `none | sftp | full` (SFTP-only memakai `internal-sftp` + chroot), tombol **"Salin perintah SSH"** untuk folder aktif, mis. `ssh -t p_alice@host "cd '/home/p_alice/sites/example.com/public' && exec \$SHELL -l"`.
- Perubahan `sshd` hanya lewat drop-in `/etc/ssh/sshd_config.d/50-panel.conf` yang membatasi `Match Group panel-tenants`, divalidasi `sshd -t` sebelum reload. **Tidak boleh mengunci akses SSH admin.**
- Sesi SSH tenant masuk slice tenant (limit resource sama).
- Backlog: shell jail ala CageFS (bubblewrap/namespace).

### 5.5 Detail project (halaman site)

| Tab | Isi | Fase |
|-----|-----|------|
| Overview | Status, aksi cepat (start/stop/restart), resource, SSL expiry, versi runtime, disk, peringatan | 8 |
| Domains | Primary, alias, subdomain, www-redirect, force HTTPS | 3 |
| Runtime | Versi, ekstensi, ini PHP, command Node/Python, worker | 4, 7 |
| Files | Tautan ke file manager di folder site | 5 |
| Environment | Env var terenkripsi, mask, import `.env` | 8 |
| SSL | Let's Encrypt, upload custom, status, auto-renew | 3 |
| Logs | Access/error/app, live tail, filter, unduh | 3 |
| Cron | Jadwal (systemd timer), run-now, histori, timeout | 8 |
| Rules | Redirect, header, CORS, hotlink, basic auth, IP allow/deny, maintenance mode, custom error page, snippet nginx (admin-only, divalidasi) | 8 |
| Deploy | Git (deploy key, webhook, release + rollback), histori | 8 |
| Databases | MariaDB/PostgreSQL per tenant | 8 |
| Backups | Snapshot file + DB, jadwal, retensi, restore | 8 |
| Analytics | Traffic, performa, error, bot, geo, insight | 9 |
| Settings | Rename, pindah owner, hapus (danger zone) | 3 |

### 5.6 Analytics mendalam

**Sumber data**

1. **Log nginx format JSON** (satu baris per request):

```nginx
log_format panel_json escape=json
  '{"t":"$time_iso8601","h":"$host","m":"$request_method","u":"$request_uri",'
  '"s":$status,"b":$body_bytes_sent,"rt":$request_time,"urt":"$upstream_response_time",'
  '"cs":"$upstream_cache_status","ip":"$remote_addr","ref":"$http_referer",'
  '"ua":"$http_user_agent","proto":"$server_protocol","tls":"$ssl_protocol"}';
```

2. Metrics sistem dari `/proc` dan cgroup v2 (CPU, RAM, swap, disk, inode, net, load, IO) per server, per tenant, per service.
3. Status aplikasi: php-fpm status (active/idle/queue/slow), slowlog, proses Node/Python.

**Pipeline**: tailer (inotify, checkpoint offset, tahan rotasi, aman saat crash) → parser → aggregator in-memory per menit → batch write ke `analytics.db` → rollup 1m → 1h → 1d.

**Metrik per site** (per menit): jumlah request, bytes, kelas status (2xx/3xx/4xx/5xx), cache status, histogram latensi log-bucket (p50/p95/p99), upstream time, estimasi pengunjung unik, top path (dinormalisasi `/post/123` → `/post/:id`, top-K dengan Space-Saving), referrer, negara (GeoIP), browser/OS/device, klasifikasi bot, versi HTTP dan TLS, 4xx/5xx teratas, URL terlambat.

**Retensi default**: 10s → 2 jam, 1m → 14 hari, 1h → 400 hari, 1d → tanpa batas. Bisa diatur admin.

**Privasi**: pengunjung unik dihitung dari hash (IP + UA + salt harian). IP mentah hanya tersimpan di view keamanan dengan retensi pendek.

**Insight otomatis (berbasis aturan)**: lonjakan 5xx, regresi latensi setelah deploy (marker deploy di grafik), lonjakan bot, lonjakan traffic, endpoint paling lambat, cache hit rendah, `pm.max_children` tercapai, disk/quota hampir penuh. Setiap insight punya penjelasan dan saran aksi.

### 5.7 Performa dan stabilitas

**Situs tenant**

- Nginx: `sendfile`, `tcp_nopush`, `open_file_cache`, `worker_processes auto`, HTTP/2 + HTTP/3, TLS 1.2/1.3 dengan session resumption dan OCSP stapling, gzip default (brotli/zstd opsional bila modul tersedia, evaluasi Phase 10), header cache untuk aset statis, `fastcgi_cache` opsional per site, limit koneksi/request yang masuk akal.
- PHP: OPcache aktif, `pm = ondemand`, `realpath_cache` ditala, profil per plan.
- Node/Python: jumlah worker dari plan, `LimitNOFILE`, graceful restart.
- Profil tuning otomatis dari spesifikasi server (sysctl, worker nginx, default FPM) dengan halaman "Optimizer" yang bisa apply dan rollback (Phase 10).

**Panel sendiri**

- Target RAM (lihat bagian 9), startup cepat, tanpa polling boros (event-driven), query SQLite ber-index, pagination di semua list.

**Stabilitas**

- Apply atomic + rollback, job idempoten, graceful shutdown, health check semua service, watchdog, `panelctl doctor`, backup config + DB panel, uji chaos (Phase 10).

---

## 6. Keamanan

| Area | Kontrol |
|------|---------|
| Injeksi config | Domain/nama divalidasi (punycode via `idna`), tolak karakter `; { } $ \n \r \0`. Template di-escape. Test golden + fuzz untuk renderer |
| Path/File | `panel-fs` confined, `openat2`, UID drop, test adversarial |
| Auth | Argon2id, TOTP, lockout, session rotation, CSRF token, cookie `HttpOnly+Secure+SameSite=Strict` |
| Transport | HTTPS wajib untuk panel (self-signed saat awal → Let's Encrypt). HSTS setelah sertifikat valid |
| Header | CSP ketat (tanpa inline script), `X-Content-Type-Options`, `Referrer-Policy`, `frame-ancestors 'none'` |
| Rate limit | Per-IP dan per-akun untuk login, upload, API sensitif |
| Secrets | Env var tenant dienkripsi XChaCha20-Poly1305; enkripsi/dekripsi hanya di agent. `EnvironmentFile` `0600 root` (systemd membacanya sebelum drop privilege) |
| Audit | Semua aksi mutasi (API dan agent) tercatat: aktor, target, hasil, IP |
| Supply chain | Checksum runtime, `cargo-deny`, `cargo audit`, lockfile di-commit, `npm audit` untuk web |
| Ekstraksi arsip | Anti zip-slip, symlink, zip-bomb |
| Cross-tenant | Suite test isolasi otomatis (Phase 2) dijalankan di CI di setiap rilis |
| Kode `unsafe` | `#![forbid(unsafe_code)]` di semua crate kecuali agent/fs yang butuh syscall, dan itu wajib dikomentari dengan alasan |

---

## 7. API dan Realtime

- Prefix `/api/v1`. JSON. Error mengikuti **RFC 9457 Problem Details** (`type`, `title`, `status`, `detail`, `errors[]`), pesan error menjelaskan apa yang salah dan cara memperbaikinya.
- Pagination cursor-based untuk list besar. Filter dan sort lewat query param terdokumentasi.
- Operasi panjang mengembalikan `202 Accepted` + `job_id`. Progres lewat `GET /api/v1/jobs/{id}/events` (SSE).
- Header `Idempotency-Key` pada operasi pembuatan resource.
- WebSocket: `/ws/terminal/{session}`, `/ws/logs/{site}`, `/ws/metrics`. Autentikasi memakai cookie sesi + cek `Origin`.
- OpenAPI dipublikasikan di `/api/v1/openapi.json` dan menjadi sumber kebenaran client TypeScript.

Contoh rute awal: `POST /auth/login`, `GET /me`, `GET|POST /users`, `GET|POST /plans`, `GET|POST /sites`, `POST /sites/{id}/domains`, `POST /sites/{id}/ssl/issue`, `GET /sites/{id}/logs`, `GET|POST|PATCH|DELETE /sites/{id}/fs/...`, `POST /sites/{id}/terminal`, `GET /sites/{id}/analytics/...`, `GET /system/metrics`, `GET /jobs/{id}`.

---

## 8. Desain UI/UX

### 8.1 Konsep: "Papan Navigasi"

Panel ini terasa seperti **ruang kendali kapal**: tenang, presisi, mudah dibaca dari jauh. Warna dipakai untuk **status**, bukan dekorasi. Tidak ada gradien dekoratif, tidak ada kartu seragam untuk segalanya.

Satu elemen yang dibuat memorable adalah **Pulse strip**: sparkline tipis berdenyut (60 menit terakhir: request dan error) yang muncul di daftar situs, header halaman site, dan dashboard. Semua elemen lain sengaja dibuat tenang.

Prinsip:

1. **Tenang secara default, keras hanya saat ada anomali.** Merah/amber hanya untuk masalah nyata.
2. **Struktur lewat spasi dan garis tipis**, bukan kotak di dalam kotak. Kartu dipakai hanya untuk objek yang memang berdiri sendiri (situs, tenant, metrik). Halaman pengaturan memakai section datar dengan pembatas.
3. **Radius berjenjang** sesuai hierarki, bukan satu radius untuk semuanya.
4. **Dua tema setara**: terang dan gelap (plus ikut sistem), keduanya didesain, bukan satu di-invert.
5. **Keyboard-first**: command palette `⌘K`/`Ctrl+K`, shortcut file manager, fokus selalu terlihat.
6. **Teks adalah bagian dari desain**: kalimat jelas, kata kerja aktif, sentence case.

### 8.2 Design tokens (titik awal, boleh disetel setelah mockup)

Terapkan sebagai CSS variables di `web/src/styles/tokens.css` dan dipetakan ke Tailwind v4 `@theme`.

**Warna**

| Token | Terang | Gelap | Peran |
|-------|--------|-------|-------|
| `--bg` Kabut / Laut Dalam | `#EDF1F2` | `#09191F` | Latar halaman |
| `--surface` Kertas | `#FAFCFC` | `#0F252D` | Panel, tabel, dialog |
| `--raised` | `#FFFFFF` | `#143039` | Popover, menu, hover baris |
| `--line` | `#D5DEE1` | `#1F3A45` | Garis pemisah 1px |
| `--ink` Tinta | `#0C1E26` | `#E4EEF0` | Teks utama |
| `--ink-muted` | `#51656E` | `#8CA5AD` | Teks sekunder |
| `--primary` Laut | `#0B7A85` | `#3FC1C9` | Aksi utama, fokus, tautan |
| `--accent` Pala | `#D9962B` | `#F0B455` | Penanda, highlight grafik, peringatan |
| `--success` Daun | `#2F9E62` | `#4CC58A` | Sehat, aktif |
| `--danger` Cengkeh | `#C23B3B` | `#EF6B6B` | Error, aksi destruktif |
| `--info` | `#3B76C9` | `#6FA3EE` | Informasi netral |

Aturan kontras: `--primary` terang lolos AA di atas `--surface`. **`--accent` terang tidak boleh dipakai sebagai warna teks**, hanya sebagai isian/penanda dengan teks `--ink` di atasnya. Status tidak boleh hanya mengandalkan warna (selalu titik/ikon + label).

**Tipografi**

- UI: **Plus Jakarta Sans** (variable, self-host, bebas lisensi; dirancang di Jakarta). Bobot 400/500/600/700.
- Mono: **JetBrains Mono**, **hanya** untuk konten teknis (path, port, perintah, terminal, kode). Bukan untuk label kecil dekoratif.
- Skala: 12 / 13 / **14 (dasar)** / 16 / 20 / 24 / 32. Line-height 1.5 untuk isi, 1.25 untuk judul. Judul tracking -0.01em. Angka di tabel/metrik `font-variant-numeric: tabular-nums`.
- Panjang baris isi ≤ 80 karakter. Tidak memakai ALL CAPS untuk label. Font di-host sendiri (tanpa CDN).

**Bentuk dan gerak**

- Spasi basis 4px. Radius: kontrol `6px`, panel/kartu `10px`, dialog/sheet `16px`, pill penuh.
- Elevasi: garis `--line` untuk struktur; bayangan hanya untuk elemen melayang (menu, dialog): `0 8px 24px -8px rgba(9,25,31,.35)`.
- Gerak: hover/press `120ms`, buka panel/dialog `200ms` dengan `cubic-bezier(.2,.8,.2,1)`. **Satu momen terorkestrasi**: saat dashboard pertama dimuat dalam sesi, pulse strip "tergambar" kiri ke kanan (±600 ms). Selebihnya gerak hanya sebagai respons aksi pengguna. Hormati `prefers-reduced-motion`.

### 8.3 Layout shell

```
┌──────────┬────────────────────────────────────────────────────────────┐
│ Nusa     │ vps-01 ▾             Cari atau jalankan perintah  Ctrl+K    │
│          │                                          Notifikasi  RA ▾   │
│ Beranda  ├────────────────────────────────────────────────────────────┤
│ Situs    │  Konten (rata kiri, lebar maks 1280px, bukan di tengah)     │
│ Pengguna │                                                            │
│ Analitik │                                                            │
│ Runtime  │                                                            │
│ Tugas    │                                                            │
│ Audit    │                                                  [Tugas 2] │
│ Pengatur │  ← job tray kanan bawah, bisa dibuka untuk lihat progres    │
└──────────┴────────────────────────────────────────────────────────────┘
```

- Sidebar bisa diciutkan menjadi rail ikon. Tenant hanya melihat menu yang relevan.
- Breakpoint: ≥1280 penuh, 768–1279 rail ikon, <768 navigasi bawah + tampilan kartu (file manager mobile = list saja).
- Kepadatan: toggle **Nyaman / Padat** (tinggi baris tabel 48px / 36px).

### 8.4 Wireframe halaman kunci

**Dashboard (admin)**

```
Semua layanan normal. 3 situs perlu perhatian.

┌ CPU ─────────┐ ┌ Memori ──────┐ ┌ Disk ────────┐ ┌ Jaringan ─────┐
│ 18%   ▁▂▃▂▂  │ │ 1,2 / 4 GB   │ │ 41 / 80 GB   │ │ naik 2,1 Mb/s │
└──────────────┘ └──────────────┘ └──────────────┘ └───────────────┘

Situs                                        Cari   Filter   [Situs baru]
────────────────────────────────────────────────────────────────────────
● example.com    PHP 8.3   ▁▂▂▃▅▃▂▂▁▂▃▃▂   1,2k req/mnt   p95 180 ms
● api.acme.dev   Node 22   ▁▁▂▂▂▃▂▂▂▁▁▂▂   340 req/mnt    p95 42 ms
▲ blog.rafi.id   PHP 8.1   ▂▂▂▇█▆▃▂▂▂▂▂▂   5xx naik ke 4,2%   Lihat penyebab
```

**Detail situs**

```
example.com   [● Berjalan]  PHP 8.3          ▁▂▂▃▅▃▂▂▁▂▃▃▂  (pulse strip)
Dibuat oleh p_alice. SSL aktif sampai 12 Des 2026.   [Restart] [Buka situs]
─────────────────────────────────────────────────────────────────────────
Ringkasan  Domain  Runtime  Environment  SSL  Log  Cron  Aturan  Deploy
Database  Backup  Analitik  Pengaturan
─────────────────────────────────────────────────────────────────────────
Penggunaan sumber daya          Peringatan
RAM 210 MB dari 512 MB          Kuota disk 91%. Bersihkan file atau naikkan plan.
CPU 6%                          Deploy terakhir 2 jam lalu (berhasil).
Disk 4,5 GB dari 5 GB
```

**File manager**

```
/ sites / example.com / public                 Kuota 4,5 / 5 GB  [Terminal di sini]
┌ Pohon ────────┬───────────────────────────────────────────────────────────┐
│ ▾ sites       │ [Unggah] [Folder baru] [Arsipkan]        Cari…   List|Grid │
│   ▾ example   │ Nama                 Ukuran   Diubah        Izin   Pemilik │
│     ▸ public  │ ▸ wp-content            —     kemarin       755    p_alice │
│     ▸ logs    │   index.php          405 B    3 hari lalu   644    p_alice │
│ ▸ tmp         │   .htaccess          1,2 KB   3 hari lalu   644    p_alice │
└───────────────┴───────────────────────────────────────────────────────────┘
Tray unggah: backup.zip  ████████░░ 82%  [Jeda]
```

**Analitik situs**

```
Rentang [24 jam ▾]  Bandingkan dengan periode lalu [aktif]   Realtime [ ]
┌ Request ─────────────────────────┐ ┌ Latensi p50 / p95 / p99 ──────────┐
│ (area chart + marker deploy)     │ │ (3 garis, tooltip bersama)        │
└──────────────────────────────────┘ └───────────────────────────────────┘
┌ Status 4xx/5xx ─┐ ┌ Cache hit ─┐ ┌ Bot vs manusia ┐ ┌ Negara (peta) ───┐
Tabel: URL terlambat · URL error · Referrer · Browser/OS · Top IP (keamanan)
Insight: "p95 naik 2,3x setelah deploy 14:02. Endpoint /checkout paling lambat."
```

### 8.5 Komponen inti (bangun di Phase 0–1, pakai ulang di seluruh panel)

`PulseStrip`, `StatusPill` (titik + label, tidak hanya warna), `ResourceMeter`, `DataTable` (virtual, padat/nyaman, kolom bisa diatur), `CommandPalette`, `ConfirmDangerDialog` (ketik nama resource untuk konfirmasi), `JobTray`, `EmptyState` (selalu berisi aksi berikutnya), `PathChip`/`CodeChip` dengan tombol salin, `FormSection` datar dengan label di kiri, `Toast` (sonner), `Skeleton`.

### 8.6 Aturan UX dan copy

- **Setiap halaman punya tiga keadaan yang dirancang**: loading (skeleton), kosong (undangan beraksi), error (apa yang salah + cara memperbaiki).
- Bahasa default **Indonesia**, tersedia **English** (i18next). Sentence case, kata kerja aktif. Tombol menyebut hasil persis: "Simpan perubahan", "Terbitkan sertifikat", bukan "Submit". Nama aksi konsisten sampai toast ("Situs dibuat").
- Pesan error tidak minta maaf dan tidak samar: "Domain sudah dipakai situs lain. Pilih domain lain atau hapus situs tersebut."
- Aksi destruktif selalu lewat `ConfirmDangerDialog`; aksi yang bisa dibatalkan memakai toast dengan "Urungkan".
- Tanpa tanda panah dekoratif di tombol/tautan, tanpa label ALL CAPS, tanpa eyebrow di atas setiap judul.
- Aksesibilitas: target WCAG 2.2 AA, fokus terlihat, navigasi keyboard penuh, label ARIA, tabel semantik, `prefers-reduced-motion` dan `prefers-color-scheme` dihormati.
- Optimistic UI hanya untuk aksi yang aman dibatalkan; operasi sistem menampilkan status job nyata.

---

## 9. Persyaratan Non-Fungsional (target terukur)

Semua angka adalah **target yang harus divalidasi lewat benchmark** (Phase 10), bukan klaim.

| Area | Target |
|------|--------|
| RAM idle `panel-api` | ≤ 40 MB RSS |
| RAM idle `panel-agent` | ≤ 20 MB RSS |
| RAM panel total saat beban sedang (100 situs, 10 sesi aktif) | ≤ 150 MB |
| Start cold `panel-api` | < 1 detik |
| Latensi API baca (tanpa operasi sistem) | p95 < 50 ms |
| Buat site static / PHP (tanpa penerbitan SSL) | < 5 s / < 10 s |
| Reload nginx | 0 koneksi aktif terputus |
| Gagal apply config | 100% rollback ke config terakhir yang valid |
| List folder 10.000 entri | respons < 300 ms, scroll tetap mulus |
| Upload | file 10 GB resumable, memori konstan (streaming) |
| Web UI | JS awal ≤ 250 KB gzip (rute dashboard); editor, terminal, chart berat di-lazy-load; LCP < 1,5 s |
| Ingest log | ≥ 5.000 baris/detik pada 1 vCPU, lag < 5 detik |
| Skala | 500 situs / 100 tenant per server tanpa degradasi panel |
| Ketahanan | Restart/crash panel **tidak** mengganggu situs tenant (data plane terpisah dari control plane) |

---

## 10. Strategi Testing

| Level | Isi | Alat |
|-------|-----|------|
| Unit | Validator, renderer template, parser log, aggregator, `panel-fs` | `cargo test`, `insta` (snapshot/golden) |
| Integration (VM) | Agent menjalankan operasi nyata: user, nginx, systemd, ACL, quota | Test harness di VM, jalan lewat `just test-vm` |
| **Isolasi lintas-tenant** | A tidak bisa baca/tulis/list data B, tidak melihat proses B, tidak lolos limit memori/proses (OOM terkurung, fork bomb tertahan) | `tests/isolation/` (wajib lulus 100% di CI sebelum rilis) |
| Keamanan | Path traversal, symlink, TOCTOU, zip-slip/bomb, injeksi config, CSRF, authz | Test khusus + `cargo-fuzz` untuk path normalizer, validator domain, renderer nginx, parser log, extractor arsip |
| E2E UI | Alur utama: login, buat site, deploy, file manager, terminal | Playwright |
| Beban | Static, PHP, WordPress, Node, proxy; ingest log | `oha`/`wrk`/`k6` |
| Chaos | Kill agent saat apply, kill nginx, disk penuh, config rusak, reboot paksa | Skrip di VM (Phase 10) |
| Matriks CI | Ubuntu 22.04, Ubuntu 24.04, Debian 12 | GitHub Actions + VM runner |

---

## 11. Roadmap Bertahap

Urutan fase mengikuti **dependensi teknis** (isolasi tenant harus ada sebelum hosting; hosting sebelum file manager yang butuh identitas tenant), bukan urutan penyebutan fitur. Setiap fase menghasilkan sesuatu yang bisa dijalankan dan diuji sendiri.

Format tiap fase: **Tujuan → Prasyarat → Task → Acceptance criteria → Jangan dikerjakan dulu**.

---

### Phase 0 — Fondasi dan tooling

**Tujuan**: kerangka repo, lingkungan dev aman, CI, dan jalur komunikasi API↔agent terbukti bekerja.
**Prasyarat**: tidak ada.

- [x] **P0-T1 Repo dan workspace**: Cargo workspace dengan crate kosong sesuai 4.2, `web/` (Vite + React + TS), `justfile` (`dev`, `test`, `lint`, `test-vm`), `rustfmt.toml`, `clippy` lint config, `.editorconfig`, `deny.toml`.
- [x] **P0-T2 VM dev**: skrip `scripts/dev-vm.sh` (Multipass/Lima) membuat Ubuntu 24.04 dengan systemd, nginx, build tools, repo ter-mount. Dokumentasi cara pakai di `docs/dev.md`.
- [x] **P0-T3 CI**: GitHub Actions untuk `fmt`, `clippy -D warnings`, `test`, `cargo-deny`, serta `tsc`/lint/test untuk web. Cache cargo.
- [x] **P0-T4 IPC skeleton**: `panel-proto` (envelope, `proto_version`, `Ping`, `Version`), `panel-agent` mendengarkan Unix socket + cek `SO_PEERCRED`, `panel-api` memanggilnya. Integration test.
- [x] **P0-T5 API skeleton**: Axum, loader config TOML (`/etc/panel/panel.toml`), `tracing` JSON, `/healthz`, `/api/v1/system/info`, OpenAPI via `utoipa`, graceful shutdown.
- [x] **P0-T6 Frontend shell**: token desain (8.2), font self-host, tema terang/gelap/sistem, layout shell (8.3), routing, client API digenerate dari OpenAPI, scaffolding i18n (id/en), komponen dasar (`StatusPill`, `EmptyState`, `Skeleton`, `Toast`). Embed `web/dist` ke `panel-api` via `rust-embed`; Vite dev proxy untuk development.
- [x] **P0-T7 Dokumen proyek**: `AGENTS.md` (ringkasan aturan bagian 0), `docs/PROGRESS.md`, `docs/adr/0001-stack.md`.

**Acceptance**: `just dev` menjalankan semuanya; `just test` hijau; di VM, UI menampilkan versi agent yang didapat dari `panel-api` lewat socket; CI hijau.
**Jangan dulu**: auth, database, fitur hosting.

---

### Phase 1 — Core panel (auth, DB, job, audit)

**Tujuan**: fondasi aplikasi yang aman untuk semua fitur berikutnya.
**Prasyarat**: Phase 0.

- [x] **P1-T1 Database**: SQLite WAL (`busy_timeout`, `foreign_keys`), migrasi `sqlx`, repository untuk `users`, `sessions`, `audit_logs`, `jobs`, `settings`, `api_tokens`.
- [x] **P1-T2 Bootstrap admin**: `panelctl admin create` dan token setup sekali pakai untuk first-run.
- [x] **P1-T3 Auth**: login/logout, Argon2id, sesi cookie, rotasi sesi, CSRF, rate limit + lockout, ganti password.
- [x] **P1-T4 2FA**: TOTP + recovery codes, enrol dan verifikasi.
- [x] **P1-T5 RBAC**: middleware `admin`/`user`, fungsi policy terpusat, test authz untuk setiap rute.
- [x] **P1-T6 Audit log**: penulis audit (API dan agent), API list + filter.
- [x] **P1-T7 Job system**: tabel `jobs`, worker, state machine, retry + backoff, idempotensi, SSE progres, pemulihan setelah restart.
- [x] **P1-T8 UI inti**: login, 2FA, wizard first-run, halaman profil/keamanan (sesi aktif, 2FA, API token), halaman Audit, `JobTray`, command palette (dasar).

**Acceptance**: tidak ada rute mutasi tanpa auth + CSRF; test authz lulus; login brute-force terkunci; job bertahan saat `panel-api` di-restart; UI lolos cek aksesibilitas dasar (keyboard, kontras).
**Jangan dulu**: user tenant, plan, hosting.

---

### Phase 2 — Tenant user dan isolasi

**Tujuan**: tenant terisolasi sebagai fondasi seluruh hosting.
**Prasyarat**: Phase 1.

- [x] **P2-T1 Agent: lifecycle user**: `user.create` (validasi, alokasi UID, home, mode + ACL, umask, tanpa password), `user.suspend/unsuspend`, `user.delete` (retensi + purge).
- [x] **P2-T2 Plan**: CRUD plan; penerapan batas ke slice `panel-u<uid>.slice` (drop-in `MemoryMax`, `CPUQuota`, `TasksMax`, `IOWeight`).
- [x] **P2-T3 Quota disk**: deteksi dukungan FS, set/get quota, job laporan pemakaian, fallback scan terjadwal.
- [x] **P2-T4 Pembaca resource**: baca cgroup v2 per slice (`cpu.stat`, `memory.current`, `io.stat`, `pids.current`) lewat agent.
- [x] **P2-T5 Hardening host**: opsi `hidepid=2` untuk `/proc`, pembuatan grup `panel-tenants`, pengecekan prasyarat (cgroup v2, ACL, quota) dengan pesan jelas bila tidak terpenuhi.
- [x] **P2-T6 API + UI**: daftar tenant, wizard buat tenant, halaman detail (ringkasan, resource, plan), suspend/unsuspend, hapus dengan `ConfirmDangerDialog`, halaman Plan, impersonate dengan banner + audit.
- [x] **P2-T7 Suite isolasi**: `tests/isolation/` (cross-tenant read/write/list, `/proc`, OOM terkurung, fork bomb tertahan, symlink ke home tenant lain).

**Acceptance**: suite isolasi 100% hijau di VM; limit plan terbukti menahan proses (uji beban); suspend menghentikan semua service tenant.
**Jangan dulu**: site/domain, nginx.

---

### Phase 3 — Site engine: static + Nginx + domain + SSL

**Tujuan**: situs static pertama bisa online dengan HTTPS, dan pipeline apply config aman.
**Prasyarat**: Phase 2.

- [x] **P3-T1 Model site dan domain**: validasi (IDN/punycode lewat `idna`), keunikan domain, status.
- [x] **P3-T2 Baseline nginx**: installer menulis `nginx.conf` ter-tuning dan direktori include; server default menolak host tak dikenal (444); vhost panel (`:8443`); snippet (acme, security-headers, deny-sensitive, fastcgi-params); log format `panel_json`.
- [x] **P3-T3 Renderer + apply atomic**: render `minijinja` → staging → `nginx -t` → swap → reload debounce → **rollback otomatis**. Test golden, test injeksi (domain jahat), test rollback dengan config sengaja rusak.
- [x] **P3-T4 Site static**: buat/aktif/nonaktif/hapus, halaman placeholder, docroot, SPA fallback, header cache.
- [ ] **P3-T5 Alias dan redirect domain**: alias, subdomain, www ↔ non-www, force HTTPS.
- [ ] **P3-T6 ACME**: akun, HTTP-01, terbit, simpan sertifikat (`0600`), scheduler renew (30 hari sebelum expiry), OCSP stapling, upload sertifikat custom, fallback self-signed, backoff + dukungan staging LE.
- [ ] **P3-T7 Log**: log per site di `/var/log/panel/<user>/<site>/`, logrotate, API tail + pencarian (dibatasi), live tail via WebSocket.
- [ ] **P3-T8 UI**: daftar situs (dengan `PulseStrip` placeholder), wizard buat site (pilih tipe → domain → tenant → tinjau), detail situs: Ringkasan, Domain, SSL, Log, Pengaturan.

**Acceptance**: situs static online dengan HTTPS (staging LE atau domain uji); config rusak tidak pernah membuat nginx mati (rollback terbukti di test); domain berisi karakter terlarang ditolak di API dan agent.
**Jangan dulu**: PHP, runtime lain, analytics.

---

### Phase 4 — PHP multi-versi terisolasi

**Tujuan**: PHP hosting yang terisolasi dan bisa ganti versi per site.
**Prasyarat**: Phase 3.

- [ ] **P4-T1 Manajer runtime PHP**: tambah repo Sury/PPA, install/hapus versi sebagai job dengan progres, daftar versi tersedia dan terpasang, **mask** service bawaan `phpX.Y-fpm`.
- [ ] **P4-T2 Ekstensi**: daftar kurasi, aktif/nonaktif per versi (admin) dan per site (dalam batas plan).
- [ ] **P4-T3 Master per (tenant, versi)**: generator unit systemd (jalan sebagai user tenant, sandbox 4.4, slice tenant), pool per site, socket `/run/panel/php/<user>-<ver>.sock`, default hardened (5.1), `pm = ondemand` dari plan, slowlog, status path internal.
- [ ] **P4-T4 Template Nginx PHP**: preset generic, WordPress, Laravel (`public/`), custom front-controller; blok file sensitif; batas upload; `fastcgi_cache` opsional.
- [ ] **P4-T5 Ganti versi**: alur pindah pool + reload + health check + **rollback otomatis**.
- [ ] **P4-T6 Pengaturan PHP**: UI untuk pengaturan `php.ini` kurasi (5.1) dengan validasi dan batas plan; raw `php.ini` hanya admin.
- [ ] **P4-T7 Log PHP**: viewer error log dan slow log; kumpulkan status FPM (active/idle/queue) untuk Phase 9.
- [ ] **P4-T8 Uji kompatibilitas**: WordPress, Laravel, PHP polos; dua tenant dengan versi berbeda berjalan bersamaan; skrip PHP tenant A gagal membaca file tenant B.

**Acceptance**: WordPress dan Laravel berjalan; ganti 8.1 → 8.3 tanpa error pelanggan; `phpinfo` menunjukkan `open_basedir` dan versi yang benar; RAM idle tanpa traffic = nol worker.
**Jangan dulu**: Node/Python, backup, DB.

---

### Phase 5 — File manager custom

**Tujuan**: file manager yang aman dan terasa premium, dirancang sendiri.
**Prasyarat**: Phase 2 (UID tenant), Phase 3 (struktur site).

- [ ] **P5-T1 `panel-fs`**: library resolusi path confined (`openat2` / `cap-std`) dan operasi dasar; test adversarial lengkap (5.3).
- [ ] **P5-T2 Model fs-worker**: agent men-spawn worker yang drop ke UID/GID tenant (`setgroups`, `setgid`, `setuid`, `no_new_privs`, rlimit), protokol via socketpair, pool + idle timeout.
- [ ] **P5-T3 API operasi**: list (cursor, sort, filter), stat, mkdir, rename, move, copy (job + progres), hapus → **trash** (restore, retensi, hitung quota), kosongkan trash.
- [ ] **P5-T4 Upload/download**: upload resumable chunked + cek quota di muka + finalize atomic; download Range/ETag; zip folder streaming.
- [ ] **P5-T5 Arsip**: compress zip/tar.gz dan extract aman (anti zip-slip, symlink, zip-bomb) sebagai job.
- [ ] **P5-T6 Permission**: lihat/ubah mode (rekursif dengan mode file dan folder terpisah), peringatan 777, batasan chown.
- [ ] **P5-T7 Editor dan preview**: CodeMirror 6, batas ukuran, deteksi konflik (mtime/ETag), preview gambar/PDF/video/teks.
- [ ] **P5-T8 Pencarian**: nama dan isi, batas waktu dan batas hasil, sebagai job.
- [ ] **P5-T9 UI file manager**: sesuai 8.4: breadcrumb, pohon, list/grid virtual, multi-select, rubber band, drag-drop, context menu, shortcut keyboard, tray unggah (jeda/lanjut), meter quota, empty/loading/error state, tombol "Terminal di sini" (disabled sampai Phase 6).

**Acceptance**: seluruh test adversarial `panel-fs` lulus; unggah 10 GB bisa dilanjutkan setelah koneksi putus dengan memori konstan; 10.000 entri tetap mulus; tenant tidak bisa keluar dari home-nya lewat jalur apa pun.
**Jangan dulu**: terminal, SSH.

---

### Phase 6 — Web terminal dan SSH

**Tujuan**: shell langsung di folder aktif, baik lewat browser maupun SSH.
**Prasyarat**: Phase 5.

- [ ] **P6-T1 Session manager PTY di agent**: spawn login shell **sebagai UID tenant**, di dalam slice tenant (transient scope), `cwd` = folder yang diminta (divalidasi lewat `panel-fs`), `no_new_privs`, rlimit; batas sesi per plan; idle timeout.
- [ ] **P6-T2 Protokol WebSocket**: frame biner, resize, heartbeat, cek `Origin`, autentikasi cookie, backpressure; reconnect dengan scrollback terbatas.
- [ ] **P6-T3 UI terminal**: xterm.js (fit, search), multi-tab, tema mengikuti token, tombol "Terminal di sini" dari file manager, indikator koneksi.
- [ ] **P6-T4 Audit**: catat awal/akhir sesi (tenant, folder, durasi, IP). Perekaman keystroke tidak aktif secara default.
- [ ] **P6-T5 Manajemen SSH key**: tambah/hapus public key per tenant (file dikelola panel di lokasi root-owned), validasi format, fingerprint di UI.
- [ ] **P6-T6 Kebijakan shell**: `none | sftp | full` per plan/tenant; drop-in `sshd` untuk grup `panel-tenants` (SFTP-only via `internal-sftp` + chroot); validasi `sshd -t` sebelum reload; **uji bahwa akses SSH admin tidak pernah terkunci**.
- [ ] **P6-T7 Salin perintah SSH**: UI membuat perintah `ssh -t user@host "cd '<folder>' && exec $SHELL -l"` dan info SFTP.

**Acceptance**: terminal terbuka di folder aktif sebagai tenant yang benar; `id`, `ls /home`, dan `cat /home/<tenant-lain>` membuktikan isolasi; fork bomb di terminal tertahan oleh `TasksMax`; SFTP-only tidak bisa membuka shell.
**Jangan dulu**: shell jail ala CageFS (backlog).

---

### Phase 7 — Node.js, Python, Reverse proxy

**Tujuan**: semua tipe hosting v1 tersedia.
**Prasyarat**: Phase 3 dan 5.

- [ ] **P7-T1 Runtime Node**: ambil daftar versi dari nodejs.org (`index.json`), download + verifikasi SHA256, install ke `/opt/panel/runtimes/node/<ver>`, `corepack enable`, hapus (diblokir bila dipakai), opsi mirror/offline.
- [ ] **P7-T2 Site Node**: root app, command install/build/start (job sebagai UID tenant, timeout, output streaming), alokasi port (rentang 20000–29999, unik di DB, bind `127.0.0.1`) atau Unix socket, generator unit systemd (contoh 5.1), template proxy dengan WebSocket upgrade, health check, aksi start/stop/restart/redeploy, status.
- [ ] **P7-T3 Runtime + site Python**: install versi via `uv`, venv per site, install dependency (`requirements.txt`/`pyproject`) sebagai job, entrypoint WSGI/ASGI, gunicorn/uvicorn lewat Unix socket, jumlah worker dari plan, preset Django/Flask/FastAPI (static/media mapping).
- [ ] **P7-T4 Reverse proxy**: upstream http/https/unix, preserve Host, WebSocket, timeout, buffering, header kustom, health upstream, **guard SSRF** (5.1).
- [ ] **P7-T5 UI proses terpadu**: status, uptime, CPU/RAM (cgroup), restart, live log untuk Node dan Python.
- [ ] **P7-T6 Uji aplikasi contoh**: Express, Next.js (standalone), Django, FastAPI, Flask. Verifikasi isolasi (service tenant A tidak melihat home tenant B).

**Acceptance**: kelima aplikasi contoh berjalan lewat HTTPS; crash app otomatis restart; upstream eksternal ditolak untuk tenant tanpa izin; versi runtime yang dipakai tidak bisa dihapus.
**Jangan dulu**: Docker, nftables per-UID (Phase 10).

---

### Phase 8 — Kelengkapan detail project

**Tujuan**: halaman site selengkap hosting panel pada umumnya.
**Prasyarat**: Phase 3–7.

- [ ] **P8-T1 Environment variables**: terenkripsi (enkripsi hanya di agent), mask/reveal, import `.env`, penanda "perlu restart".
- [ ] **P8-T2 Cron**: systemd timer + service per job (jalan sebagai tenant, di slice tenant), builder ekspresi cron + terjemahan bahasa manusia, run-now, histori + log, timeout, batas concurrent.
- [ ] **P8-T3 Aturan situs**: redirect terstruktur (301/302, path/regex, divalidasi), header kustom, CORS preset, hotlink protection, basic auth (bcrypt), IP allow/deny, mode maintenance, custom error page, snippet nginx (admin-only, allowlist direktif, `nginx -t`).
- [ ] **P8-T4 Git deploy**: deploy key ed25519, clone/pull, branch, perintah build, hook, struktur release dengan **swap symlink atomic dan rollback**, endpoint webhook dengan secret (GitHub/GitLab), histori + log deploy, marker deploy untuk analytics.
- [ ] **P8-T5 Database**: install-on-demand MariaDB/PostgreSQL (admin), buat DB + user per tenant dengan prefix dan batas dari plan, rotasi password, remote access mati secara default, import/export lewat job.
- [ ] **P8-T6 Backup**: snapshot lokal (file `tar.zst` + dump DB), jadwal, retensi, restore (ke lokasi sama atau baru), hitung ukuran terhadap quota. S3-compatible = "Should".
- [ ] **P8-T7 Notifikasi**: pusat notifikasi + SMTP opsional untuk SSL hampir expired, quota > 90%, job gagal, deploy gagal.
- [ ] **P8-T8 Halaman Ringkasan final**: status, aksi cepat, resource, SSL expiry, versi runtime, disk, peringatan kontekstual, deploy terakhir.

**Acceptance**: seluruh tab di 5.5 berfungsi; rollback deploy bekerja; restore backup mengembalikan situs yang identik; env var tidak pernah muncul plaintext di DB, log, atau audit.

---

### Phase 9 — Analytics mendalam

**Tujuan**: analisis yang benar-benar menjawab "kenapa situs saya lambat/error?".
**Prasyarat**: Phase 3 (log JSON), 4 (status FPM), 8 (marker deploy).

- [ ] **P9-T1 Collector metrics sistem**: `/proc` + cgroup per server, tenant, service; interval 10 s → rollup 1m/1h/1d; retensi 5.6.
- [ ] **P9-T2 Log tailer**: inotify, checkpoint offset, tahan rotasi, aman saat crash, atribusi site dari path, backpressure.
- [ ] **P9-T3 Aggregator**: metrik per menit (5.6): kelas status, bytes, histogram latensi (p50/p95/p99), cache, unik (hash harian), top-K path ternormalisasi, referrer, GeoIP (`maxminddb`; DB-IP Lite/GeoLite2 sesuai lisensi, dokumentasikan), UA parser, klasifikasi bot, versi HTTP/TLS.
- [ ] **P9-T4 Metrik aplikasi**: polling status php-fpm, parse slowlog, metrik proses Node/Python dari cgroup.
- [ ] **P9-T5 Query API**: rentang waktu, interval otomatis, perbandingan periode, filter, top-N, ekspor CSV, batas biaya query.
- [ ] **P9-T6 UI analitik**: halaman per site, per tenant, dan server-wide; rentang waktu + preset, auto-refresh, tampilan realtime (WS), grafik (area/line/bar/heatmap/peta/treemap), tabel dengan drill-down, marker deploy, **`PulseStrip` sungguhan** di daftar situs dan header situs.
- [ ] **P9-T7 Insight engine**: aturan di 5.6 menghasilkan insight + saran aksi, kirim ke pusat notifikasi.
- [ ] **P9-T8 Analitik keamanan** *(Should)*: IP teratas, probing path (`wp-login`, `xmlrpc`, `.env`), indikasi brute force, blokir IP dengan TTL.
- [ ] **P9-T9 Siklus data**: job retensi, `VACUUM`/`wal_checkpoint`, batas ukuran DB, pemulihan bila `analytics.db` korup.

**Acceptance**: ingest ≥ 5.000 baris/detik pada 1 vCPU dengan lag < 5 s; grafik 30 hari termuat < 500 ms; tidak ada kebocoran IP mentah di luar view keamanan; insight "regresi latensi setelah deploy" terbukti di skenario uji.

---

### Phase 10 — Performance, hardening, installer, rilis

**Tujuan**: dari "jalan" menjadi "stabil dan siap produksi".
**Prasyarat**: Phase 0–9.

- [ ] **P10-T1 Benchmark harness**: `oha`/`wrk`/`k6` untuk static, PHP hello, WordPress, Node hello, proxy; laporan baseline di `docs/benchmarks/`; bandingkan tuning (`fastcgi_cache`, HTTP/3, OPcache, brotli/zstd bila ada modul).
- [ ] **P10-T2 Optimizer**: profil tuning otomatis dari spesifikasi server (worker nginx, sysctl, default FPM, hint worker Node/Python), apply + rollback dari UI.
- [ ] **P10-T3 Optimasi panel**: profil memori, evaluasi `mimalloc`/`jemalloc`, `panic = "abort"`, ukuran binary, waktu start; **validasi target RAM bagian 9**.
- [ ] **P10-T4 Hardening**: audit ke OWASP ASVS L2 (subset relevan), fuzz target lengkap, `cargo audit`/`deny`, skor `systemd-analyze security` unit panel, filter nftables per-UID untuk loopback (tenant A tidak bisa konek ke port app tenant B), CSP ketat, rotasi secret.
- [ ] **P10-T5 Keandalan**: uji chaos (kill agent saat apply, kill nginx, disk penuh, config rusak, reboot paksa), `panelctl doctor`, `panelctl reconcile`, `panelctl backup` untuk DB + config panel, pemulihan job.
- [ ] **P10-T6 Installer dan upgrader**: `install.sh` idempoten (cek OS/kernel/cgroup, paket, user, unit, baseline nginx, petunjuk firewall, token first-run), `panelctl upgrade` (migrasi DB + rollback), uninstall, pipeline rilis (x86_64 + aarch64, target `gnu`).
- [ ] **P10-T7 Dokumentasi**: panduan admin, panduan tenant, referensi API (OpenAPI), dokumen arsitektur, `docs/security-model.md`, panduan kontribusi.
- [ ] **P10-T8 Beta**: dogfood di VPS nyata, soak test 7 hari, bug bash, checklist rilis v1.0.

**Acceptance**: seluruh target bagian 9 tercapai dan terdokumentasi; suite isolasi + chaos + fuzz hijau; instalasi dari nol di VPS bersih < 10 menit; upgrade dari versi sebelumnya tanpa kehilangan data.

---

## 12. Backlog (setelah v1)

Email (Postfix/Dovecot), DNS server (PowerDNS), WAF (Coraza/ModSecurity), aplikasi Docker, multi-server (agent remote lewat mTLS), reseller + billing/WHMCS API, installer 1-click (WordPress dll.), staging/clone site, integrasi CDN, Redis/Memcached per-tenant, scanner malware, shell jail ala CageFS (bubblewrap), network namespace per-tenant, backup offsite terjadwal, sub-user/kolaborator per tenant, HA panel.

---

## 13. Keputusan Terbuka (konfirmasi pemilik produk)

| # | Keputusan | Default di PRD | Dampak bila diubah |
|---|-----------|----------------|--------------------|
| 1 | Nama produk | Nusa Panel (working title) | Rename crate/binary/UI |
| 2 | Web server | Nginx | Caddy: ubah renderer + ACME + model reload (Phase 3) |
| 3 | Granularitas master PHP-FPM | Per (tenant, versi) | Per-site: isolasi lebih ketat antar-site, RAM lebih besar |
| 4 | DB panel | SQLite | PostgreSQL: tambah daemon + migrasi repository |
| 5 | Frontend | React + shadcn/ui | Vue/Svelte: tulis ulang komponen dasar (Phase 0) |
| 6 | DB tenant default | MariaDB (+ PostgreSQL opsional) | Phase 8-T5 |
| 7 | OS target awal | Ubuntu 22.04/24.04, Debian 12 | RHEL-family butuh jalur paket berbeda (PHP/nginx/ACL) |
| 8 | Lisensi | Belum ditentukan | Mempengaruhi pemilihan dependency |
| 9 | Multi-server di v1 | Tidak (RPC dibuat agnostik transport) | Pindah ke roadmap lebih awal |

---

## 14. Risiko dan Mitigasi

| Risiko | Mitigasi |
|--------|----------|
| Scope sangat besar | Fase kecil, task ber-ID, acceptance jelas, tidak ada loncat fase |
| Agent berjalan sebagai root | RPC bertipe, validasi ganda, allowlist binary, audit, fuzz, hardening systemd |
| Config nginx/php rusak mematikan situs | Apply atomic + rollback, test golden, uji chaos |
| Tenant lolos isolasi | Suite isolasi wajib di CI, ACL + cgroup + sandbox berlapis, `hidepid` |
| Port loopback lintas tenant | Unix socket default untuk Python, nftables per-UID (Phase 10) |
| Perbedaan distro/repo (Sury, nginx.org) | Matriks CI 3 OS, versi di-pin, pesan prasyarat jelas |
| FS tanpa dukungan quota | Deteksi + fallback scan terjadwal + peringatan di UI |
| Kontensi tulis SQLite (analytics) | DB terpisah, satu writer, batch insert, WAL |
| Rate limit Let's Encrypt | Staging, cache, backoff, jangan re-issue tanpa perlu |
| Supply chain runtime (Node/Python) | Verifikasi checksum/signature, mirror opsional |
| Waktu compile Rust lama | Workspace terpecah, `mold`, `sccache`, cache CI |

---

## Lampiran A — Template `AGENTS.md`

```markdown
# AGENTS.md
Baca PRD.md sebelum bekerja. Aturan ringkas:
1. Satu task (ID P<fase>-T<n>) per sesi. Jangan loncat fase.
2. Develop dan test hanya di VM. Jangan jalankan panel-agent di host.
3. panel-api TIDAK PERNAH root. Agent hanya menerima command bertipe.
4. Dilarang shell string interpolation, unwrap() di kode non-test, path mentah di agent.
5. Definition of Done: fmt + clippy -D warnings + test + docs/PROGRESS.md diperbarui.
6. Ambigu? Pakai default aman di PRD, catat ADR, lanjut.
7. Desain UI mengikuti PRD bagian 8 (token, copy, aksesibilitas).
```

## Lampiran B — Format `docs/PROGRESS.md`

```markdown
# Progress
## Phase 3 — Site engine
- [x] P3-T1 Model site dan domain — PR #12, test: sites::validation
- [ ] P3-T2 Baseline nginx — sedang dikerjakan
  - Catatan: ADR-0004 (struktur snippet)
## Blocker / pertanyaan
- (kosong)
```

## Lampiran C — Glosarium

**Tenant**: pengguna hosting dengan satu user Linux. **Site**: satu proyek web (static/PHP/Node/Python/proxy). **Agent**: daemon root yang melakukan operasi sistem. **Slice**: grup cgroup systemd untuk batas resource. **Apply atomic**: render → validasi → swap → reload → rollback otomatis bila gagal. **Pulse strip**: sparkline 60 menit terakhir yang menjadi elemen visual khas panel.
