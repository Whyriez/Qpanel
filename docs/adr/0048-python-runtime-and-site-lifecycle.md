# ADR 0048: Python Runtime and Site Lifecycle via uv, venv, and Unix Sockets

## Status
Accepted

## Konteks
Sesuai PRD Bagian 5.1 dan Spesifikasi Fase 7 (P7-T3 "Runtime + site Python (uv + venv)"), Nusa Panel harus mendukung hosting aplikasi Python mandiri secara terisolasi, aman, dan berkinerja tinggi:
1. **Manajemen Runtime Python Terkurasi**: Mendukung instalasi multi-versi Python (`3.10.16`, `3.11.11`, `3.12.9` default/stable, `3.13.2`) yang dikelola secara terisolasi di `/opt/panel/runtimes/python/<ver>` menggunakan toolchain modern berkecepatan tinggi `uv`.
2. **Virtualenv Per-Site Terisolasi (`.venv`)**: Setiap situs Python memiliki lingkungan virtual tersendiri di direktori kerja tenant (`/home/<user>/sites/<domain>/.venv` atau root aplikasi), dibuat dan dikelola di bawah UID tenant menggunakan `uv venv`.
3. **Eksekusi di Bawah Slice Cgroup Tenant**: Semua proses aplikasi dan perintah instalasi paket/migrasi dieksekusi di bawah UID tenant (`p_<slug>`) dan terikat ke cgroup slice tenant (`panel-u<uid>.slice`).
4. **Unix Domain Sockets untuk Reverse Proxy**: Alih-alih mengikat port TCP loopback (yang berpotensi konflik port atau probe lintas-tenant), aplikasi Python mengekspos Unix domain socket di `/run/panel/python/site_<id>.sock` dengan kepemilikan dan izin akses terkelola (`0775` / `0660` dengan `UMask=0007`).
5. **Kalkulasi Worker WSGI/ASGI**: Mendukung Gunicorn (WSGI), Uvicorn (ASGI), dan Gunicorn dengan Uvicorn worker class. Jumlah worker diturunkan secara dinamis dari jumlah core CPU `(2 * cores) + 1` dan dibatasi secara ketat oleh kuota proses paket tenant (`max_plan_processes`).
6. **Preset Framework Teruji**: Menyediakan konfigurasi bawaan untuk framework populer:
   - **Django**: Penanganan direktori statis (`/static/` -> alias) dan media pengguna (`/media/` -> alias) langsung di layer Nginx, WSGI entrypoint `wsgi:application`, perintah migrasi dan `collectstatic`.
   - **Flask**: Penanganan direktori statis (`/static/`), WSGI entrypoint `app:app`.
   - **FastAPI**: Server ASGI `uvicorn` dengan entrypoint `main:app`.
   - **Generic**: Konfigurasi fleksibel untuk skrip kustom atau WSGI/ASGI lain.
7. **Pengerasan Systemd Sesuai PRD 5.1**: Unit systemd `site-<id>.service` menerapkan direktif isolasi ketat: `ProtectSystem=strict`, `NoNewPrivileges=true`, `PrivateTmp=true`, `LimitNOFILE=65535`, `Restart=on-failure`, `RuntimeDirectory=panel/python`, `ReadWritePaths=/home/<user> /run/panel/python`.

## Keputusan Arsitektur

### 1. Modul Runtime Python (`panel-runtime::python`)
- Menyediakan katalog versi Python resmi terkurasi dengan tag status (`Stable`, `Eol`, `Legacy`).
- Membangun perintah CLI `uv python install <ver>` dan `uv python uninstall <ver>` dengan flag aman (`--install-dir /opt/panel/runtimes/python/<ver>`).
- Fungsi `calculate_default_python_workers(cpu_cores, max_plan_processes)` menghitung rasio worker optimal yang terkontrol batas kuota tenant.
- Menyediakan builder perintah untuk `uv venv`, `uv pip install`, eksekusi skrip, migrasi Django, dan pembuatan unit service.

### 2. Generator Unit Systemd Sandboxed (`panel-agent::python_site`)
- Berkas unit dibuat di `/etc/systemd/system/site-<id>.service` dengan izin `0644`.
- Menjalankan server aplikasi (Gunicorn/Uvicorn) yang terinstal di `.venv` situs:
  ```ini
  [Unit]
  Description=Nusa Panel Python Site <site_id>
  After=network.target

  [Service]
  Type=simple
  User=<username>
  Group=<username>
  WorkingDirectory=<app_dir>
  Slice=panel-u<uid>.slice
  EnvironmentFile=-/etc/panel/env/site-<id>.env
  ExecStart=<venv_dir>/bin/gunicorn --workers <workers> --bind unix:/run/panel/python/site_<id>.sock <entrypoint>
  Restart=on-failure
  RestartSec=2
  ProtectSystem=strict
  NoNewPrivileges=true
  PrivateTmp=true
  LimitNOFILE=65535
  RuntimeDirectory=panel/python
  UMask=0007
  ReadWritePaths=/home/<username> /run/panel/python
  ```
- Berkas environment variable disimpan di `/etc/panel/env/site-<id>.env` dengan permission `0600` milik tenant.

### 3. Template Nginx Reverse Proxy ke Unix Socket (`panel-webserver`)
- Template `PYTHON_VHOST_TEMPLATE` mengalirkan request ke unix socket upstream:
  ```nginx
  location / {
      proxy_pass http://unix:/run/panel/python/site_<id>.sock;
      proxy_http_version 1.1;
      proxy_set_header Upgrade $http_upgrade;
      proxy_set_header Connection $connection_upgrade;
      proxy_set_header Host $host;
      proxy_set_header X-Real-IP $remote_addr;
      proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
      proxy_set_header X-Forwarded-Proto $scheme;
  }
  ```
- Mendukung pemetaan direktori statis dan media langsung di Nginx (`location /static/`, `location /media/`) dengan perlindungan terhadap berkas sensitif (`requirements.txt`, `pyproject.toml`, `.venv`, `__pycache__`).
- Validasi jalur Unix socket anti-injection (`validate_unix_socket_path`) memastikan tidak ada karakter berbahaya (`;`, `{`, `}`, newline, whitespace) dan path diawali `/` serta tidak mengandung path traversal `..`.

### 4. Eksekusi Tenant Job Aman
- Tenant dapat menjalankan perintah pemeliharaan:
  - `uv venv`
  - `uv pip install -r requirements.txt` / `pip install -r requirements.txt`
  - `python manage.py migrate` / `python manage.py collectstatic`
  - Perintah kustom lainnya.
- Perintah dieksekusi di bawah UID tenant dalam cgroup slice tenant menggunakan `systemd-run` dengan batas timeout dan output streaming (stdout, stderr, exit code, durasi).

### 5. API dan Antarmuka Pengguna (UI)
- REST API:
  - `/api/v1/runtimes/python/versions`: Daftar versi Python terkurasi dan status instalasi.
  - `/api/v1/runtimes/python/install` & `/{version}`: Instalasi dan penghapusan runtime Python via agent.
  - `/api/v1/sites/{id}/python/status`: Metrik proses (ActiveState, PID, RAM, CPU) dan probe socket unix.
  - `/api/v1/sites/{id}/python/action`: Aksi siklus hidup (`start`, `stop`, `restart`, `redeploy`).
  - `/api/v1/sites/{id}/python/job`: Eksekusi tugas tenant dengan terminal runner.
  - `/api/v1/sites/{id}/python/settings`: Pengambilan dan pembaruan pengaturan framework, worker, direktori static/media, dan env vars.
- UI:
  - `SiteWizardDialog`: Pilihan aplikasi Python dengan preset otomatis (Django, Flask, FastAPI, Generic).
  - `SiteDetailPage`: Tab khusus Python dengan kartu metrik `PythonStatusCard` dan form konfigurasi `PythonSettingsCard`.

## Konsekuensi
- **Positif**:
  - Isolasi tinggi dan performa optimal melalui per-site virtual environment dan komunikasi Unix domain socket.
  - Tidak ada risiko tabrakan port TCP loopback antar-tenant pada server bersama.
  - Sizing worker yang adaptif terhadap kapasitas CPU host dan batas paket tenant.
  - Dukungan lengkap untuk framework modern Python (Django, Flask, FastAPI) dengan offloading file statis/media ke Nginx.
- **Batasan**:
  - Socket directory `/run/panel/python` harus dapat diakses oleh Nginx worker (grup www-data) dan tenant, dikelola oleh systemd `RuntimeDirectory` dan `UMask=0007`.
