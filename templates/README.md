# Templates

Direktori ini berisi template konfigurasi berbasis `minijinja` untuk:
- `nginx/`: Server block situs (static, php, node, python, proxy) dan snippet pendukung (security-headers, acme, fastcgi-params).
- `php/`: `php-fpm.conf` dan pool per situs (`pool.d/<site>.conf`).
- `systemd/`: Service units untuk daemon Node.js, Python (gunicorn/uvicorn), dan cron timer.
