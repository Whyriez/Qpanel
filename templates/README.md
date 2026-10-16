# Templates

Direktori ini berisi template konfigurasi berbasis `minijinja` untuk:
- `nginx/`: Server block situs (static, php, node, python, proxy) dan snippet pendukung (security-headers, acme, fastcgi-params).
- `php/`: `php-fpm.conf` dan pool per situs (`pool.d/<site>.conf`).
- `systemd/`: Service units untuk daemon Node.js, Python (gunicorn/uvicorn), dan cron timer.

Template Nginx memakai global MiniJinja `qpanel_default_error_page`, yang disediakan
oleh `NginxTemplateRenderer`. Consumer template mandiri perlu mengisinya dengan
`panel_webserver::error_pages::render_default_not_found()`. Respons HTML ditanam
dalam vhost sehingga tidak membutuhkan berkas error tambahan pada server.
