# ADR-0001: Pilihan Stack Teknologi & Arsitektur Workspace

- **Status**: Diterima (Accepted)
- **Tanggal**: 2026-10-03
- **Konteks**: PRD Nusa Panel v0.1

## Konteks dan Masalah

Membangun control panel web hosting modern yang memiliki:
1. Footprint memori (RAM) sangat kecil (target idle API ≤ 40MB, agent ≤ 20MB, total sedang ≤ 150MB).
2. Isolasi tenant yang ketat (user Linux per tenant, systemd slice cgroup v2, sandbox systemd, path confinement).
3. UI modern dengan analitik latensi mendalam dan file manager kustom.
4. Pemisahan privilege tegas: `panel-api` berjalan tanpa hak akses root (user `panel`), sedangkan `panel-agent` berjalan sebagai root hanya untuk operasi sistem yang divalidasi bertipe melalui Unix socket dengan verifikasi `SO_PEERCRED`.

## Keputusan Arsitektur

### 1. Backend & Runtime
- **Bahasa**: Rust (edition 2024, stable) untuk performa, zero GC, kontrol memori ketat, dan keamanan memori.
- **HTTP Server**: `axum` + `tokio` + `tower-http` untuk async IO, WebSocket, SSE, dan ekosistem tower middleware.
- **Database Panel**: SQLite (WAL mode) via `sqlx` dengan busy timeout dan connection pooling. Database terpisah untuk analytics (`analytics.db`).
- **Template Engine**: `minijinja` untuk rendering konfigurasi Nginx, PHP-FPM, dan systemd service.
- **IPC Protocol**: Frame length-delimited over Unix Domain Socket (`tokio-util`), typed JSON payload, verifikasi `SO_PEERCRED`.

### 2. Frontend
- **Framework**: React 19 + TypeScript + Vite.
- **Styling**: Tailwind CSS v4 + custom design tokens (`web/src/styles/tokens.css`).
- **Komponen & Ikon**: Radix UI primitives, Lucide React, Sonner (toast).
- **Distribusi**: `web/dist` di-embed ke dalam binary `panel-api` menggunakan `rust-embed`.

### 3. Struktur Workspace (Cargo)
- `crates/panel-core`: Tipe domain, newtype validator (Username, Domain, Port, dll.), error types.
- `crates/panel-proto`: Definisi RPC API <-> Agent, pesan bertipe, versi protokol.
- `crates/panel-api`: HTTP API non-root, auth, RBAC, background jobs, OpenAPI generator.
- `crates/panel-agent`: Daemon root eksekusi operasi sistem bertipe.
- `crates/panel-db`: Migrasi SQLite dan repository pattern.
- `crates/panel-webserver`: Renderer Nginx configuration dan atomic apply / rollback.
- `crates/panel-runtime`: Pengelola PHP-FPM, Node.js, Python, dan proxy.
- `crates/panel-fs`: Library confined file operations (`openat2` / `cap-std`).
- `crates/panel-analytics`: Parser log Nginx, metric aggregator, analytics DB query.
- `crates/panel-cli`: Binary CLI `panelctl` untuk admin, diagnose, backup.

## Konsekuensi

- Membutuhkan pemisahan privilege yang disiplin: tidak ada pemanggilan shell mentah (`sh -c`) dan tidak ada path traversal.
- Pengujian agent dan isolasi harus dilakukan di Linux VM (Ubuntu 24.04 dengan cgroup v2 dan systemd).
