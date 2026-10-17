# Qpanel

Panel hosting Rust dengan API non-root, agent privileged dan isolasi tenant.

Versi development tetap **0.1.0** sampai diputuskan naik versi. Panel memakai
Nginx pada `http://SERVER_IP:17890`, diteruskan ke
`panel-api` di `127.0.0.1:7700`. Database panel berada di
`/var/lib/panel/panel.db`. Job/progress dan audit memakai
`/var/lib/panel/panel.operations.db`, sedangkan metrik/analitik memakai
`/var/lib/panel/analytics.db`. Operations dan analitik memiliki writer tunggal
serta reader terpisah. Port 80/443 tetap untuk website tenant.

Upgrade mengimpor job/audit lama sekali sebelum worker dimulai. Backup menyertakan
core dan operations; pemulihan pasangan database dilakukan offline. Detail alur
dan acceptance tersedia di [pengelolaan database](docs/database-management.md).

Jika installer menolak template yang hilang, lihat
[panduan paket release dan pemulihan](RELEASE_PACKAGING.md). Push source saja
belum memperbarui arsip GitHub Release.

Instalasi menggunakan paket release lengkap. Lihat
[panduan pembuatan website PHP/Node.js/Python](SITE_RUNTIME.md),
[catatan perbaikan historis](docs/release-v0.1.1.md),
[panduan administrator](docs/admin-guide.md), dan
[arsitektur](docs/architecture.md).

```bash
sudo bash install.sh --unattended
# Port publik alternatif:
sudo bash install.sh --unattended --port 19090
```

Instalasi baru membuat username dan password administrator otomatis, ditampilkan
sekali setelah pemeriksaan kesehatan berhasil. Upgrade mempertahankan akun lama.
Ubah kredensial melalui **Profil & keamanan**, atau pulihkan melalui SSH:

```bash
sudo panelctl admin list
sudo panelctl admin reset-password --username NAMA
panelctl --help
panelctl admin --help
```

Lihat [panduan akun administrator](ADMIN_ACCOUNTS.md) untuk instalasi, pemulihan,
2FA, password custom melalui stdin, dan daftar bantuan CLI.

Development dan integration test sistem mengikuti [AGENTS.md](AGENTS.md).
Jangan menjalankan `panel-agent` di laptop/host pengembang.
