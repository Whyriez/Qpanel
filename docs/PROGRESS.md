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

- [x] P5-T1 panel-fs path-confined: library resolusi path confined tanpa string concat, pencegahan traversal (..), sanitasi null byte & unicode fullwidth slash, kebijakan symlink (DenyAll & ResolveBeneath), pembatasan panjang path, proteksi penghapusan root jail, penulisan atomik, paginasi dan filter entri direktori, suite pengujian adversarial 10 skenario — test: adversarial_test (10 tests), ADR-0030
- [x] P5-T2 fs-worker drop privilege: model eksekusi worker terisolasi dengan drop privilege ke UID/GID tenant, protokol IPC bertipe (FsWorkerRequest/FsWorkerResponse), pool worker per-tenant (FsWorkerPool), pemulihan otomatis dari crash/terminasi, siklus hidup idle timeout reaping (0 RAM idle footprint), suite pengujian konkurensi & isolasi tenant — test: worker_test (3 tests), ADR-0031
- [x] P5-T3 API file manager: endpoint RESTful lengkap (list, stat, mkdir, rename/move, copy, trash, trash list, trash restore, trash empty, permanent delete) dengan validasi RBAC multi-tenant terisolasi, mapping otomatis user ke UID/GID Linux tenant (`p_<user>`), siklus hidup `.panel-trash` aman dalam kuota disk tenant dengan metadata deduplikasi, audit logging terintegrasi — test: file_manager_api_test (2 integration tests), ADR-0032
- [x] P5-T4 Upload/download chunked resumable: staging di `.panel-upload-tmp/` dengan penulisan chunk berbasis offset, pengecekan kuota disk di muka (pre-flight quota check 413 Payload Too Large), finalisasi atomik (rename .part ke file tujuan dengan pembuatan direktori induk otomatis), protokol resume tahan putus koneksi (deteksi offset mismatch 409 Conflict), download streaming HTTP Range (206 Partial Content), evaluasi ETag (304 Not Modified), dan streaming ZIP direktori on-the-fly — test: file_transfer_api_test (3 integration tests), ADR-0033
- [x] P5-T5 Arsip zip/tar.gz aman: kompresi streaming ZIP & TAR.GZ, proteksi anti Zip-Slip (sanitasi path, penolakan path traversal .., null byte, absolute/volume prefix), penolakan symlink/hardlink ke luar root jail, guardrails anti Zip-Bomb (pembatasan rasio max 100x, batas max entries 50.000, batas uncompressed 10 GB), pemrosesan otomatis arsip >10MB via background job dengan pelaporan progress live, endpoint RESTful `POST /api/v1/files/archive/compress` & `POST /api/v1/files/archive/extract` dengan isolasi RBAC tenant — test: archive_test (6 tests), archive_api_test (4 integration tests), ADR-0034
- [x] P5-T6 Permission editor: manajemen izin berkas POSIX DAC via grid centang 3x3 dan input oktal tersinkronisasi, recursive chmod dengan pemisahan mode file (0644) dan direktori (0755), pendeteksian dan warning banner otomatis untuk mode 0777/world-writable, pembatasan ketat kepemilikan berkas (chown ditolak untuk tenant non-admin 403 Forbidden dan dicegah di level ConfinedFs), REST API chmod/chmod-recursive/chown lengkap dengan audit logging, serta komponen UI FilePermissionModal teruji — test: permission_test (5 tests), permission_api_test (3 integration tests), FilePermissionModal.test (4 tests), ADR-0035
- [x] P5-T7 CodeMirror 6 editor & preview: integrasi editor CodeMirror 6 dengan penomoran baris, tema gelap, pintasan Ctrl+S, status bar metrik berkas (baris, karakter, ukuran, encoding), guardrails batas ukuran 5 MB (otomatis beralih ke mode read-only), deteksi konflik konkurensi (optimistic concurrency 409 Conflict via mtime/ETag) dengan banner dialog pilihan Reload atau Overwrite paksa, modal pratinjau multi-tipe (gambar dengan kontrol zoom/rotasi, PDF embed, media player HTML5 untuk video/audio, dan fallback file biner), serta endpoint RESTful GET & PUT `/api/v1/files/content` terlindungi RBAC tenant — test: file_editor_api_test (3 integration tests), FileEditorModal.test (4 tests), FilePreviewModal.test (13 tests), ADR-0036
- [x] P5-T8 Pencarian file: mesin pencarian berkas dan konten (grep-like) di dalam confined filesystem `panel-fs` dengan batas waktu timeout (default 15 detik), kuota batas hasil (default 100 entri), deteksi otomatis berkas biner (inspeksi null byte pada 512 byte pertama), batas ukuran file isi (5 MB), pengabaian direktori sampah `.panel-trash` dan staging `.panel-upload-tmp`, pencegahan traversal `..`, dukungan mode sinkron dan background job (`as_job: true`) via JobRunner (`files.search`), endpoint RESTful `POST /api/v1/files/search` teruji, serta komponen UI `FileSearchModal` interaktif (filter ekstensi, toggle cari isi, cuplikan baris kode, dan banner batas waktu/hasil) — test: panel-fs::search_test (5 unit tests), file_search_api_test (3 integration tests), FileSearchModal.test (5 tests), ADR-0037
- [x] P5-T9 UI file manager custom: antarmuka pengelola berkas sesuai Wireframe 8.4 (breadcrumb navigasi path, pohon folder collapsible di panel kiri, toggle tampilan List vs Grid virtual, multi-select via checkbox / Shift+klik / Ctrl+klik, rubber band marquee drag selection box, drag-and-drop berkas internal dan drop berkas desktop OS, context menu klik kanan kaya fitur, shortcut keyboard Ctrl+A/Delete/F2/Ctrl+C/Ctrl+V/Ctrl+F/Esc, floating upload tray dengan progres chunked resumable dan pause/resume/cancel, meter kuota disk visual, empty/loading/error state, drawer tempat sampah .panel-trash, tombol "Terminal di sini" disabled untuk Phase 6, integrasi modal-modal P5, serta rute mandiri FileManagerPage dan tab Berkas di SiteDetailPage) — test: FileManager.test (5 tests), ADR-0038

## Phase 6 — Web terminal dan SSH

- [x] P6-T1 Session manager PTY di agent: alokasi pseudoterminal (master/slave PTY), spawn login shell sebagai UID/GID Linux tenant (`p_<slug>`), attachment proses ke slice cgroup v2 tenant (`panel-u<uid>.slice`), validasi direktori kerja `cwd` terkurung aman dalam home tenant anti-traversal & symlink escape via `panel-fs`, hardening proses (`PR_SET_NO_NEW_PRIVS`, `rlimit` NPROC & NOFILE), I/O streaming non-blocking, resize window terminal (`TIOCSWINSZ`), batas sesi konkuren per tenant, idle timeout reaper berkala, protokol IPC typed `panel-proto` dan klien API `panel-api` — test: panel_agent::pty::tests (4 tests), pty_ipc_test (1 integration test), ADR-0039
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
