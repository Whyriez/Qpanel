# Nusa Panel

Panel hosting Rust dengan API non-root, agent privileged dan isolasi tenant.

Produksi v0.1.1 memakai Nginx pada `http://SERVER_IP:17890`, diteruskan ke
`panel-api` di `127.0.0.1:7700`. Database panel berada di
`/var/lib/panel/panel.db`. Port 80/443 tetap untuk website tenant.

Instalasi menggunakan paket release lengkap. Lihat
[panduan revisi v0.1.1](docs/release-v0.1.1.md),
[panduan administrator](docs/admin-guide.md), dan
[arsitektur](docs/architecture.md).

```bash
sudo bash install.sh --unattended
# Port publik alternatif:
sudo bash install.sh --unattended --port 19090
```

Development dan integration test sistem mengikuti [AGENTS.md](AGENTS.md).
Jangan menjalankan `panel-agent` di laptop/host pengembang.
