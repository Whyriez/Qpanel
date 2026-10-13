# Revisi v0.1.1 — P10-T6

Implementasi lokal untuk `FIX_V0.1.1.md`. Belum dirilis atau dideploy.

## Akses produksi

```text
Browser/LAN → Nginx :17890 → panel-api 127.0.0.1:7700
                                   ↓
                    /var/lib/panel/panel.db (panel-api:panel)
                                   ↓
                    /run/panel/agent.sock (root:panel 0660)
```

Port 80/443 tetap untuk situs tenant. Port 7700 tidak dibuka ke LAN/WAN.
HTTP adalah baseline fresh install revisi ini; TLS panel memerlukan konfigurasi
HTTPS terpisah. Cookie secure mengikuti mode transport yang dikonfigurasi.
Unit menetapkan `PANEL_PUBLIC_SCHEME=http` untuk setup/login fresh install.
Untuk deployment HTTPS, sesuaikan `cookie_secure` pada konfigurasi autentikasi
lengkap di panel.toml; konfigurasi eksplisit tetap dihormati.

## Instalasi dan upgrade

Gunakan paket lengkap dari release yang telah memuat perbaikan ini. Memanggil
bootstrap sebelum release diterbitkan masih dapat mengunduh v0.1.0.

```bash
# Setelah domain get.nusapanel.com aktif:
curl -fsSL https://get.nusapanel.com | sudo bash
# Bootstrap GitHub yang digunakan saat domain belum aktif:
curl -fsSL https://raw.githubusercontent.com/Whyriez/Vpanel/main/scripts/bootstrap-install.sh | sudo bash

# Dari direktori paket release yang sudah diekstrak:
sudo bash install.sh --unattended
sudo bash install.sh --unattended --port 19090
sudo env PANEL_PUBLIC_PORT=19090 bash install.sh --unattended

# Upgrade v0.1.0: pakai installer BARU, bukan panelctl lama yang belum diperbaiki.
sudo bash /tmp/nusa-panel-v0.1.1-x86_64-unknown-linux-gnu/install.sh --upgrade --unattended
# Setelah CLI baru tersedia, jalur yang sama dapat dipanggil melalui:
sudo panelctl upgrade --binary-dir /tmp/nusa-panel-v0.1.1-x86_64-unknown-linux-gnu/bin
```

Upgrade mewajibkan snapshot serta restart layanan. Opsi melewati backup tidak
diterima untuk deployment produksi. Snapshot sebelum perubahan tersimpan di
`/var/backups/panel/install-*`. File tenant di `/home` tidak diganti; config
Nginx custom/default tidak dihapus. Layout include Nginx yang tidak standar
ditolak secara eksplisit sebelum mengganti site panel.

Uninstall: `sudo bash uninstall.sh` atau `sudo panelctl uninstall --yes`.
Data tetap dipertahankan kecuali `--purge`; hanya site Nginx panel dicopot.
Aturan firewall tidak dihapus otomatis karena dapat dimiliki pengguna.

## Pemeriksaan lokal yang aman

```bash
bash scripts/tests/install-regression-test.sh
cargo fmt --check
cargo clippy --workspace --lib --bins -- -D warnings
cargo test -p panel-core --lib
cargo test -p panel-api --lib config::tests
cargo test -p panel-webserver --lib baseline::tests
```

Pemeriksaan ini tidak membuktikan instalasi produksi, reboot atau akses LAN.
Tidak ada perubahan UI pada revisi ini.

Hasil lokal Windows pada 13 Oktober 2026: 26 regression installer, sintaks 11
skrip Bash, 91 unit tests terpilih, formatting Rust dan Clippy lulus. Tes Unix
socket tersedia tetapi tidak dijalankan di host Windows.

## Acceptance manual yang belum dijalankan

Jangan jalankan harness berikut pada host atau server produksi. Hanya untuk VM
baru disposable, setelah pengguna mengizinkan pengujian.

```bash
sudo env NUSA_DISPOSABLE_VM=yes bash scripts/tests/install-vm-acceptance.sh --install /path/release
# Reboot dilakukan secara eksplisit oleh operator VM:
sudo reboot
sudo env NUSA_DISPOSABLE_VM=yes bash scripts/tests/install-vm-acceptance.sh --after-reboot
# VM terpisah dengan v0.1.0 dan tenant fixtures:
sudo env NUSA_DISPOSABLE_VM=yes bash scripts/tests/install-vm-acceptance.sh --upgrade /path/new-release
```

- [ ] Fresh Debian 12 melalui bootstrap paket v0.1.1.
- [ ] Fresh Ubuntu 22.04 dan Ubuntu 24.04.
- [ ] Ketiga binary `--version` tanpa warning GLIBC.
- [ ] Ownership database/WAL/SHM dan socket sesuai identity service.
- [ ] API restart/stop tidak menghapus agent socket.
- [ ] API dapat start dari `/` sebagai non-root.
- [ ] Health internal/proxy dan doctor setelah reboot.
- [ ] Browser LAN membuka UI dan setup/login dapat digunakan.
- [ ] Upgrade berulang mempertahankan akun/token, file tenant dan config custom.
- [ ] Fault injection Nginx/reload, API dan migrasi memicu rollback nyata.
- [ ] Uninstall mempertahankan tenant, site default dan aturan firewall lain.

Lihat [ADR 0078](adr/0078-production-install-runtime-repair.md).
