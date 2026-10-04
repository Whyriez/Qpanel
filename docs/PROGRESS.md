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
- [x] P3-T4 Site static: docroot provisioning (/home/p\_<slug>/sites/<domain>/public), izin POSIX ACL www-data, placeholder index.html ramah pengguna, CRUD API, lifecycle (enable/disable/delete), audit log — test: static_site_api_test, ADR-0017
- [x] P3-T5 Alias dan redirect domain: alias, subdomain, www ↔ non-www, force HTTPS, pencegahan injeksi redirect target, kompilasi ulang vhost atomik — test: template::tests (domain redirects + malicious target injection), domain_redirect_api_test, ADR-0018
- [x] P3-T6 ACME: Let's Encrypt auto-SSL, HTTP-01 challenge, fallback self-signed, custom cert upload, strict 0600 key storage, renewal scheduler (30 hari), backoff + staging support — test: ssl_acme_api_test, ADR-0019
- [x] P3-T7 Log per site & WebSocket live tail: logrotate baseline Nginx format panel_json (14 hari kompresi, kill USR1), typed IPC SiteLogsReadRequest/Response, REST endpoint filter & tail, WebSocket live tailing dengan CSWSH origin defense, autentikasi session, dan backlog — test: site_logs_api_test (3 tests), ADR-0020
- [x] P3-T8 UI site management & detail: daftar situs dengan indikator sparkline PulseStrip, wizard buat site 4 langkah (tipe -> domain -> tenant -> tinjau), detail situs 5 tab (Ringkasan, Domain alias & redirect, SSL Let's Encrypt / self-signed / custom upload, Log live tail WebSocket terminal, Pengaturan & ConfirmDangerDialog) — test: SitesPage.test, SiteDetailPage.test, ADR-0021

## Phase 4 — PHP multi-versi terisolasi

- [x] P4-T1 Manajer runtime PHP: repositori Sury PHP PPA (`ppa:ondrej/php`), instalasi/penghapusan versi PHP (7.4, 8.0, 8.1, 8.2, 8.3, 8.4) via background jobs dengan progress, pencegahan penghapusan versi aktif di database, masking otomatis `systemctl mask phpX.Y-fpm.service` pada instalasi agar FPM OS tidak pernah berjalan sebagai root, UI RuntimePage & konfirmasi bahaya — test: panel-runtime::tests, panel-proto::tests, panel-agent::tests, php_runtime_test, RuntimePage.test, ADR-0022
- [x] P4-T2 Ekstensi PHP: daftar kurasi 16 ekstensi, enable/disable & install/remove per versi (admin), preferensi ekstensi per-situs di settings_json, background jobs, IPC typed, UI PhpExtensionsModal — test: panel-runtime::tests, panel-proto::tests, panel-agent::tests, php_extension_test, RuntimePage.test, ADR-0023
- [x] P4-T3 Master PHP-FPM per (tenant, versi): generator unit systemd panel-fpm@<user>-<ver>.service di dalam slice panel-u<uid>.slice, sandbox ketat tanpa root, konfigurasi master & pool per-situs (pm=ondemand, idle_timeout=10s, limits dari plan), socket UDS 0660 www-data, staging & atomic swap dengan validasi sintaks php-fpm -t, auto-stop master saat seluruh pool terhapus, typed IPC & REST API masters/status/reload/restart — test: panel-runtime::fpm::tests, panel-proto::tests, panel-agent::php::tests, php_fpm_test, ADR-0024
- [x] P4-T4 Template Nginx PHP: preset generic, WordPress, Laravel (`public/`), custom front-controller (`app.php`); blok file sensitif (.env, .git, .user.ini, composer, package, wp-config, uploads/*.php); batas upload dinamis aman (`client_max_body_size`); FastCGI microcache opsional (`panel_php_cache` baseline zone, bypass aturan sesi/query) — test: panel-webserver::tests (22 unit tests), php_site_vhost_test (7 integration tests), ADR-0025
- [x] P4-T5 Ganti versi atomic + rollback: alur zero-downtime cutover (ensure target master -> apply target pool -> atomic cutover UDS socket Nginx -> post-cutover health check -> cleanup old pool dengan pembebasan RAM 0 worker idle), automatic rollback seketika saat health check/reload gagal (revert Nginx vhost atomik ke socket versi lama, hapus pool target, log audit, respons 500 informatif), typed IPC PhpFpmHealthCheck/Response, endpoint POST /api/v1/sites/{id}/php/version, UI modal ganti versi PHP & kartu status — test: panel-agent::php::tests, php_version_switch_test (3 integration tests), SiteDetailPage.test, ADR-0026
- [x] P4-T6 Pengaturan PHP: UI & backend untuk pengaturan php.ini terkurasi (memory_limit, upload/post max, execution time, timezone, OPcache) dengan validasi bounded dan batas paket langganan tenant; raw php.ini allowlist parser khusus admin; integrasi pool FPM php_admin_value/flag dan reload seketika tanpa downtime; UI PhpSettingsCard & integrasi SiteDetailPage — test: panel-core::php::tests, panel-proto::tests, panel-runtime::fpm::tests, panel-agent::php::tests, php_settings_test (5 integration tests), PhpSettingsCard.test, ADR-0027
- [x] P4-T7 Log PHP & status FPM: query metrik engine PHP-FPM real-time via UDS FastCGI (/fpm-status?json), pembacaan terisolasi error log & slow log per tenant (/var/log/panel/<tenant>/) dengan tail limit dan filter pencarian query, integrasi REST endpoint & WebSocket live tail (/ws/logs/{id}?log_type=php_error|php_slow), widget UI PhpStatusCard & pemilih log interaktif di SiteDetailPage — test: panel-agent::php::tests (37 tests), panel-proto::tests, php_logs_and_status_test (4 integration tests), PhpSettingsCard.test, ADR-0028
- [x] P4-T8 Uji kompatibilitas & isolasi PHP: verifikasi preset WordPress (wp-config denial, upload php execution block, rewrite permalinks) & Laravel (public/ docroot, .env/vendor protection, rewrite front-controller); konkurensi multi-tenant & multi-versi (Alice PHP 8.1, Bob PHP 8.3); perlindungan ganda pembacaan file lintas-tenant (POSIX DAC mode 0700 & open_basedir immutable via php_admin_value); zero idle workers ondemand (0 RAM waste); suite pengujian kernel VM (step 7/7 vm_runner.sh) — test: isolation-tests::php_isolation (19 tests), php_compatibility_and_isolation_test (3 integration tests), ADR-0029

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
