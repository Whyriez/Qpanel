# Progress Nusa Panel

## Phase 0 — Fondasi dan tooling
- [x] P0-T1 Repo dan workspace: Cargo workspace dengan 10 crate sesuai 4.2, `web/` (Vite + React 19 + TS), `justfile`, `rustfmt.toml`, `clippy` lint config, `.editorconfig`, `deny.toml`.
- [x] P0-T2 VM dev: skrip `scripts/dev-vm.sh` (Multipass/Lima) membuat Ubuntu 24.04 dengan systemd, nginx, build tools, repo ter-mount. Dokumentasi cara pakai di `docs/dev.md`.
- [x] P0-T3 CI: GitHub Actions untuk `fmt`, `clippy -D warnings`, `test`, `cargo-deny`, serta `tsc`/lint/test untuk web. Cache cargo (`.github/workflows/ci.yml`).
- [x] P0-T4 IPC skeleton: `panel-proto` (envelope, `proto_version`, `Ping`, `Version`), `panel-agent` mendengarkan Unix socket + cek `SO_PEERCRED`, `panel-api` memanggilnya. Integration test (`crates/panel-api/tests/ipc_test.rs`).
- [x] P0-T5 API skeleton: Axum, loader config TOML (`/etc/panel/panel.toml`), `tracing` JSON, `/healthz`, `/api/v1/system/info`, OpenAPI via `utoipa`, graceful shutdown.
- [x] P0-T6 Frontend shell: token desain (8.2), font self-host, tema terang/gelap/sistem, layout shell (8.3), routing, client API, scaffolding i18n (id/en), komponen dasar (`StatusPill`, `PulseStrip`, `EmptyState`, `Skeleton`, `Toast`). Embed `web/dist` ke `panel-api` via `rust-embed`; Vite dev proxy untuk development.
- [x] P0-T7 Dokumen proyek: `AGENTS.md` (ringkasan aturan bagian 0), `docs/PROGRESS.md`, `docs/adr/0001-stack.md`.

## Phase 1 — Core panel (auth, DB, job, audit)
- [x] P1-T1 Database: SQLite WAL, migrasi sqlx, repository (`users`, `sessions`, `audit_logs`, `jobs`, `settings`, `api_tokens`) — test: repository_test
- [ ] P1-T2 Bootstrap admin: panelctl admin create
- [ ] P1-T3 Auth: login/logout, Argon2id, session cookie, CSRF, rate limit
- [ ] P1-T4 2FA: TOTP + recovery codes
- [ ] P1-T5 RBAC: middleware admin/user
- [ ] P1-T6 Audit log: audit writer + API
- [ ] P1-T7 Job system: tabel jobs, worker, SSE progress
- [ ] P1-T8 UI inti: login, 2FA, wizard, security profile, job tray

## Phase 2 — Tenant user dan isolasi
- [ ] P2-T1 Agent: lifecycle user Linux
- [ ] P2-T2 Plan: cgroup slice limits
- [ ] P2-T3 Quota disk: FS quota & scanner
- [ ] P2-T4 Pembaca resource: cgroup v2
- [ ] P2-T5 Hardening host: hidepid=2, group panel-tenants
- [ ] P2-T6 API + UI: tenant management & plan UI
- [ ] P2-T7 Suite isolasi: cross-tenant integration tests

## Phase 3 — Site engine: static + Nginx + domain + SSL
- [ ] P3-T1 Model site dan domain
- [ ] P3-T2 Baseline nginx
- [ ] P3-T3 Renderer + apply atomic
- [ ] P3-T4 Site static
- [ ] P3-T5 Alias dan redirect domain
- [ ] P3-T6 ACME: Let's Encrypt auto-SSL
- [ ] P3-T7 Log per site & WebSocket live tail
- [ ] P3-T8 UI site management & detail

## Phase 4 — PHP multi-versi terisolasi
- [ ] P4-T1 Manajer runtime PHP
- [ ] P4-T2 Ekstensi PHP
- [ ] P4-T3 Master PHP-FPM per (tenant, versi)
- [ ] P4-T4 Template Nginx PHP
- [ ] P4-T5 Ganti versi atomic + rollback
- [ ] P4-T6 Pengaturan PHP
- [ ] P4-T7 Log PHP & status FPM
- [ ] P4-T8 Uji kompatibilitas & isolasi PHP

## Phase 5 — File manager custom
- [ ] P5-T1 panel-fs path-confined
- [ ] P5-T2 fs-worker drop privilege
- [ ] P5-T3 API file manager
- [ ] P5-T4 Upload/download chunked resumable
- [ ] P5-T5 Arsip zip/tar.gz aman
- [ ] P5-T6 Permission editor
- [ ] P5-T7 CodeMirror 6 editor & preview
- [ ] P5-T8 Pencarian file
- [ ] P5-T9 UI file manager custom

## Phase 6 — Web terminal dan SSH
- [ ] P6-T1 Session manager PTY agent
- [ ] P6-T2 WebSocket terminal streaming
- [ ] P6-T3 UI terminal (xterm.js)
- [ ] P6-T4 Audit terminal
- [ ] P6-T5 SSH key management
- [ ] P6-T6 Shell policy (none/sftp/full)
- [ ] P6-T7 Salin perintah SSH

## Phase 7 — Node.js, Python, Reverse proxy
- [ ] P7-T1 Runtime Node.js
- [ ] P7-T2 Site Node.js
- [ ] P7-T3 Runtime + site Python (uv + venv)
- [ ] P7-T4 Reverse proxy & guard SSRF
- [ ] P7-T5 UI proses terpadu
- [ ] P7-T6 Uji aplikasi contoh

## Phase 8 — Kelengkapan detail project
- [ ] P8-T1 Environment variables terenkripsi
- [ ] P8-T2 Cron job (systemd timers)
- [ ] P8-T3 Aturan situs (redirect/headers/basic auth/CORS)
- [ ] P8-T4 Git deploy & webhook
- [ ] P8-T5 Database provisioner (MariaDB/PostgreSQL)
- [ ] P8-T6 Backup & snapshot restore
- [ ] P8-T7 Notifikasi sistem
- [ ] P8-T8 Halaman Ringkasan final

## Phase 9 — Analytics mendalam
- [ ] P9-T1 Collector metrics sistem
- [ ] P9-T2 Log tailer JSON
- [ ] P9-T3 Aggregator 1m/1h/1d
- [ ] P9-T4 Metrik aplikasi (FPM/Node/Python)
- [ ] P9-T5 Query API
- [ ] P9-T6 UI analitik mendalam & PulseStrip
- [ ] P9-T7 Insight engine
- [ ] P9-T8 Analitik keamanan
- [ ] P9-T9 Siklus data & retensi

## Phase 10 — Performance, hardening, installer, rilis
- [ ] P10-T1 Benchmark harness
- [ ] P10-T2 Optimizer server
- [ ] P10-T3 Optimasi panel & validasi RAM
- [ ] P10-T4 Hardening OWASP ASVS & nftables
- [ ] P10-T5 Chaos testing, panelctl doctor, reconcile
- [ ] P10-T6 Installer install.sh & upgrader
- [ ] P10-T7 Dokumentasi final
- [ ] P10-T8 Beta dogfooding VPS

## Blocker / Pertanyaan
- Tidak ada blocker saat ini.
