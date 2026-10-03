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
- [x] P1-T2 Bootstrap admin: `panelctl admin create`, `reset-password`, `list`, Argon2id password hashing, dan token setup sekali pakai (`setup-token [generate|status|verify]`) untuk first-run — test: admin_test, setup_token_test
- [x] P1-T3 Auth: login/logout, Argon2id, cookie sesi (`HttpOnly+Secure+SameSite=Strict`), rotasi sesi, proteksi CSRF (`X-CSRF-Token`), lockout brute-force, ganti password — test: auth_test
- [x] P1-T4 2FA: TOTP (SHA1, 6 digits, 30s, skew tolerance 1), URI/QR code base64 generation, verifikasi kode, 8 single-use recovery codes di-hash (SHA-256), challenge saat login, rescue command `panelctl admin reset-2fa <username>` — test: two_factor_test, totp::tests, repository_test::test_recovery_code_repository, admin_test::test_admin_reset_2fa
- [x] P1-T5 RBAC: role `admin`/`user`/`reseller`, middleware `RequireAdmin` & `RequireAdminCsrf`, fungsi policy terpusat (`panel_core::rbac`), isolasi resource tenant, rute manajemen admin + test authz rute lengkap — test: rbac_test, rbac::tests
- [x] P1-T6 Audit log: penulis audit (API dan agent), repositori filter dinamis, API list + filter terpaginasi, penegakan isolasi RBAC tenant — test: audit_test, repository_test::test_audit_log_repository, agent::audit::tests
- [x] P1-T7 Job system: tabel jobs, worker loop claim atomik, state machine (queued/running/succeeded/failed/cancelled), retry + backoff exponential, idempotensi key, SSE progress streaming, pemulihan crash startup, isolasi RBAC tenant — test: job_test, repository_test::test_job_repository_and_recovery
- [x] P1-T8 UI inti: login, 2FA challenge & recovery code, wizard first-run, profil/keamanan (ganti kata sandi, 2FA setup QR & recovery codes), catatan audit terpaginasi & modal payload, antrean tugas (JobsPage) & live SSE progress monitor, JobTray floating dock, CommandPalette (Ctrl+K) — test: LoginPage.test, SecurityProfilePage.test, AuditLogPage.test, CommandPalette.test, StatusPill.test

## Phase 2 — Tenant user dan isolasi
- [x] P2-T1 Agent: lifecycle user Linux: user.create (validasi format `p_<slug>`, alokasi UID 10000–59999, mode home `0700` + ACL web server `u:www-data:--x`, subdirs `sites` `0750`/`tmp`/`.ssh`/`.panel-trash`, group `panel-tenants`, tanpa password login), user.suspend (kunci shadow + pkill proses), user.unsuspend (buka kunci + pulihkan shell), user.delete (retensi ke trash-index vs purge permanen) — test: user_lifecycle_ipc_test, user::tests, repository_test::test_user_repository_crud
- [x] P2-T2 Plan: CRUD plan; penerapan batas ke slice `panel-u<uid>.slice` (drop-in `MemoryMax`, `MemoryHigh`, `CPUQuota`, `TasksMax`, `IOWeight`) — test: plan_slice_ipc_test, repository_test::test_plan_repository_crud, slice::tests
- [x] P2-T3 Quota disk: deteksi dukungan FS (ext4/XFS), set/get quota, job pelaporan pemakaian (`quota.report`), scanner direktori rekursif aman anti-symlink traversal (`quota.scan`) — test: quota_ipc_test, quota::tests, ADR-0009
- [x] P2-T4 Pembaca resource: baca cgroup v2 per slice (`cpu.stat`, `memory.current`, `memory.peak`, `io.stat`, `pids.current`, `pids.peak`) lewat agent RPC — test: resource_ipc_test, resource::tests, ADR-0010
- [x] P2-T5 Hardening host: opsi `hidepid=2` untuk `/proc`, pembuatan grup `panel-tenants`, pengecekan prasyarat (cgroup v2, ACL, quota) dengan pesan diagnostik jelas — test: hardening_ipc_test, hardening::tests, ADR-0011
- [x] P2-T6 API + UI: CRUD plan (sinkronisasi batas ke slice & kuota tenant aktif), wizard provisioning tenant 3 langkah, suspend/unsuspend/delete dengan ConfirmDangerDialog, impersonasi admin dengan banner & audit trail, live resource meter cgroup v2 & disk, hardening diagnostics & apply — test: tenants_plans_api_test, TenantsPage.test, PlansPage.test, ADR-0012
- [x] P2-T7 Suite isolasi: cross-tenant integration tests (cross-tenant read/write/list, /proc hidepid, OOM terkurung, fork bomb tertahan TasksMax, symlink escape, suspend isolation, vm_runner) — test: isolation-tests (13 tests), ADR-0013

## Phase 3 — Site engine: static + Nginx + domain + SSL
- [x] P3-T1 Model site dan domain: validasi IDN/punycode via idna, anti-injeksi, keunikan domain global, model Site & Domain, SqliteSiteRepository & SqliteDomainRepository, migrasi schema — test: domain::tests, site_domain_repository_test, ADR-0014
- [x] P3-T2 Baseline nginx: installer menulis `nginx.conf` ter-tuning & include dirs, catch-all reject unknown host (HTTP 444 & ssl_reject_handshake), vhost panel (`:8443`), snippet acme/security-headers/deny-sensitive/fastcgi-params, format log `panel_json`, IPC NginxBaselineApply — test: baseline::tests, nginx_baseline_ipc_test, ADR-0015
- [x] P3-T3 Renderer + apply atomic: render minijinja dengan filter anti-injeksi, staging directory, validasi sintaks nginx -t, swap sites-available/sites-enabled, debounce reload 500ms, automatic rollback terbukti mengembalikan config lama bila sintaks rusak — test: template::tests (golden + malicious injection), pipeline::tests (rollback), site_atomic_apply_ipc_test, ADR-0016
- [x] P3-T4 Site static: docroot provisioning (/home/p_<slug>/sites/<domain>/public), izin POSIX ACL www-data, placeholder index.html ramah pengguna, CRUD API, lifecycle (enable/disable/delete), audit log — test: static_site_api_test, ADR-0017
- [x] P3-T5 Alias dan redirect domain: alias, subdomain, www ↔ non-www, force HTTPS, pencegahan injeksi redirect target, kompilasi ulang vhost atomik — test: template::tests (domain redirects + malicious target injection), domain_redirect_api_test, ADR-0018
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
