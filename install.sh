#!/usr/bin/env bash
# QonPanel production install/upgrade. Never run on a development host.
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
set -Eeuo pipefail
# Never trace generated credentials, including when invoked with bash -x.
set +x

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PORT_EXPLICIT=false
[[ -z "${PANEL_PUBLIC_PORT:-}" ]] || PORT_EXPLICIT=true
PANEL_PUBLIC_PORT="${PANEL_PUBLIC_PORT:-17890}"
UNATTENDED=false
FORCE_OS=false
SKIP_FIREWALL=false
UPGRADE=false
BACKUP_DIR=""
STAGING_DIR=""
STATE_SAVED=false
STATE_CHECKED=false
TRANSACTION=false
API_WAS_ACTIVE=false
AGENT_WAS_ACTIVE=false
NGINX_WAS_ACTIVE=false
API_WAS_ENABLED=false
AGENT_WAS_ENABLED=false
NGINX_WAS_ENABLED=false
ADMIN_USERNAME=""
ADMIN_PASSWORD=""
RELEASE_VERSION=""
RESTORE_DIGEST=""
MANAGED_FILES=(
    /usr/local/bin/panel-api /usr/local/bin/panel-agent /usr/local/bin/panelctl
    /etc/systemd/system/panel-api.service /etc/systemd/system/panel-agent.service
    /etc/systemd/system/panel-proc-hardening.service
    /etc/systemd/system/panel-fpm@.service.d/10-panel-runtime-order.conf
    /etc/systemd/system/site-.service.d/10-panel-runtime-order.conf
    /etc/nginx/sites-available/nusa-panel.conf /etc/nginx/sites-enabled/nusa-panel.conf
    /etc/nginx/conf.d/00-qpanel-reject-unknown.conf
    /etc/nginx/conf.d/default.conf /etc/nginx/sites-enabled/default
    /etc/panel/public-port
    /etc/panel/installed-release
    /etc/systemd/system/panel-update.service
    /etc/systemd/system/qonpanel-server-backup.service /usr/local/libexec/qonpanel-server-backup
    /usr/local/libexec/panel-update-worker /usr/local/libexec/panel-bootstrap
)

fail() { echo "[ERROR] $*" >&2; return 1; }
print_help() {
    cat <<'EOF'
Penggunaan: ./install.sh [opsi]
  --port <PORT>    Port publik 1024..65535, selain 7700 (default: 17890)
  --unattended     Instalasi otomatis
  --upgrade        Upgrade transaksional dengan snapshot dan rollback
  --restore-digest <SHA256>  Sahkan satu arsip migrasi sumber untuk restore
  --force-os       Lewati pemeriksaan versi distro
  --skip-firewall  Jangan menambahkan aturan firewall
  -h, --help       Tampilkan bantuan
Instalasi baru membuat username/password admin otomatis. Upgrade mempertahankan akun.
Bantuan administrasi: panelctl --help dan panelctl admin --help.
PANEL_PUBLIC_PORT juga dapat mengatur port publik. API internal selalu 127.0.0.1:7700.
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --port)
                [[ $# -ge 2 ]] || { fail '--port membutuhkan nilai'; return 1; }
                PANEL_PUBLIC_PORT="$2"; PORT_EXPLICIT=true; shift 2 ;;
            --restore-digest)
                [[ $# -ge 2 && "$2" =~ ^[a-f0-9]{64}$ ]] || { fail '--restore-digest memerlukan SHA-256 lowercase 64 digit'; return 1; }
                RESTORE_DIGEST="$2"; shift 2 ;;
            --unattended) UNATTENDED=true; shift ;;
            --force-os) FORCE_OS=true; shift ;;
            --skip-firewall) SKIP_FIREWALL=true; shift ;;
            --upgrade) UPGRADE=true; shift ;;
            -h|--help) print_help; return 2 ;;
            *) fail "Opsi tidak dikenal: $1"; return 1 ;;
        esac
    done
}

validate_port() {
    [[ "$PANEL_PUBLIC_PORT" =~ ^[0-9]{4,5}$ ]] || { fail 'Port harus bilangan 1024..65535'; return 1; }
    PANEL_PUBLIC_PORT=$((10#$PANEL_PUBLIC_PORT))
    (( PANEL_PUBLIC_PORT >= 1024 && PANEL_PUBLIC_PORT <= 65535 && PANEL_PUBLIC_PORT != 7700 )) || {
        fail 'Port harus 1024..65535 dan bukan port internal 7700'; return 1;
    }
}

check_port_available() {
    local listeners
    listeners="$(ss -H -lntp "sport = :${PANEL_PUBLIC_PORT}")"
    [[ -z "$listeners" ]] && return 0
    # A repeated install may reuse only this panel's existing Nginx listener.
    if [[ -f /etc/nginx/sites-available/nusa-panel.conf && -L /etc/nginx/sites-enabled/nusa-panel.conf ]] &&
        [[ "$(readlink /etc/nginx/sites-enabled/nusa-panel.conf)" == /etc/nginx/sites-available/nusa-panel.conf ]] &&
        grep -Eq "^[[:space:]]*listen ${PANEL_PUBLIC_PORT};" /etc/nginx/sites-available/nusa-panel.conf &&
        ! grep -qv '"nginx"' <<< "$listeners"; then
        return 0
    fi
    fail "Port publik ${PANEL_PUBLIC_PORT} sudah digunakan. Pilih --port lain; layanan yang ada tidak dihentikan."
}

check_platform() {
    [[ "$(id -u)" == 0 ]] || { fail 'Installer harus dijalankan sebagai root'; return 1; }
    [[ -r /etc/os-release ]] || { fail '/etc/os-release tidak tersedia'; return 1; }
    . /etc/os-release
    if [[ "$FORCE_OS" != true ]]; then
        [[ ( "$ID" == ubuntu && ( "$VERSION_ID" == 22.04 || "$VERSION_ID" == 24.04 ) ) ||
            ( "$ID" == debian && "$VERSION_ID" == 12 ) ]] || {
            fail 'Distro didukung: Ubuntu 22.04/24.04 dan Debian 12'; return 1;
        }
    fi
    [[ -f /sys/fs/cgroup/cgroup.controllers ]] || { fail 'Kernel harus menggunakan cgroup v2'; return 1; }
}

install_packages() {
    # Application upgrades do not upgrade/restart host packages or tenant runtimes.
    if [[ "$UPGRADE" == true ]]; then
        local dependency
        for dependency in nginx nft curl tar sqlite3 systemctl setfacl flock ss runuser usermod; do
            command -v "$dependency" >/dev/null || { fail "Prasyarat upgrade tidak tersedia: $dependency"; return 1; }
        done
        return 0
    fi
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y --no-install-recommends nginx nftables curl tar ca-certificates \
        sqlite3 systemd acl util-linux iproute2 passwd python3 gpg dpkg-repack
}

find_release() {
    BIN_SOURCE="${SCRIPT_DIR}/bin"
    [[ -d "$BIN_SOURCE" ]] || BIN_SOURCE="${SCRIPT_DIR}/target/release"
    local binary path missing=0
    for path in UPGRADE_FROM templates/systemd/panel-api.service \
        templates/systemd/panel-agent.service \
        templates/systemd/panel-proc-hardening.service \
        templates/systemd/panel-runtime-order.conf \
        templates/systemd/panel-update.service scripts/bootstrap-install.sh scripts/panel-update-worker.sh \
        templates/systemd/qonpanel-server-backup.service scripts/server-backup-worker.py \
        templates/nginx/nusa-panel.conf \
        templates/nginx/nusa-reject-unknown.conf; do
        if [[ ! -f "$SCRIPT_DIR/$path" || ! -r "$SCRIPT_DIR/$path" || ! -s "$SCRIPT_DIR/$path" ]]; then
            printf '[ERROR] File wajib paket tidak ditemukan atau kosong: %s\n' "$path" >&2
            missing=1
        fi
    done
    for binary in panel-api panel-agent panelctl; do
        if [[ ! -x "$BIN_SOURCE/$binary" ]]; then
            printf '[ERROR] Biner paket tidak ditemukan: %s\n' "$BIN_SOURCE/$binary" >&2
            missing=1
        fi
    done
    [[ "$missing" == 0 ]] || {
        fail 'Paket rilis tidak lengkap. Unduh rilis baru yang sudah diperbaiki; jangan melewati pemeriksaan ini.'
        return 1
    }
    # Check package completeness before executing even the version commands.
    local output version
    for binary in panel-api panel-agent panelctl; do
        output="$("$BIN_SOURCE/$binary" --version)"
        printf '%s\n' "$output"
        version="${output##* }"
        [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { fail 'Versi biner paket tidak valid'; return 1; }
        [[ -z "$RELEASE_VERSION" || "$RELEASE_VERSION" == "v$version" ]] || { fail 'Versi API, agent dan CLI dalam paket tidak sama'; return 1; }
        RELEASE_VERSION="v$version"
    done
    [[ -z "${NUSA_PANEL_VERSION:-}" || "$NUSA_PANEL_VERSION" == "$RELEASE_VERSION" ]] || { fail 'Versi biner berbeda dengan tag release'; return 1; }
}

acquire_install_lock() {
    exec 9>/run/lock/panel-install.lock
    flock -n 9 || { fail 'Instalasi atau upgrade lain masih berjalan'; return 1; }
}
record_installed_release() {
    local release_digest="${NUSA_PANEL_RELEASE_SHA256:--}"
    [[ "$release_digest" == - || "$release_digest" =~ ^[a-f0-9]{64}$ ]] || { fail 'Digest release tidak valid'; return 1; }
    printf '%s %s\n' "$RELEASE_VERSION" "$release_digest" > /etc/panel/.installed-release.new
    chmod 0644 /etc/panel/.installed-release.new
    mv -f /etc/panel/.installed-release.new /etc/panel/installed-release
}
validate_upgrade() {
    local minimum installed=v0.1.0 ignored
    read -r minimum < "$SCRIPT_DIR/UPGRADE_FROM"
    [[ "$minimum" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || { fail 'Batas kompatibilitas paket tidak valid'; return 1; }
    [[ "$(printf '%s\n' "$minimum" "$RELEASE_VERSION" | sort -V | head -n 1)" == "$minimum" ]] || { fail 'Batas kompatibilitas melebihi versi paket'; return 1; }
    if [[ -f /var/lib/panel/panel.db ]]; then UPGRADE=true; fi
    if [[ "$UPGRADE" == true ]]; then
        [[ -f /var/lib/panel/panel.db ]] || { fail 'Upgrade memerlukan instalasi existing'; return 1; }
        local admin
        admin="$(runuser -u panel-api -- sqlite3 /var/lib/panel/panel.db "SELECT EXISTS(SELECT 1 FROM users WHERE role='admin');")"
        [[ "$admin" == 1 ]] || { fail 'Upgrade dihentikan: akun admin existing tidak ditemukan'; return 1; }
    fi
    if [[ -r /etc/panel/installed-release ]]; then
        read -r installed ignored < /etc/panel/installed-release
        [[ "$installed" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || { fail 'Catatan versi terpasang tidak valid'; return 1; }
        [[ "$(printf '%s\n' "$installed" "$RELEASE_VERSION" | sort -V | head -n 1)" == "$installed" ]] || { fail 'Downgrade ditolak; gunakan snapshot pemulihan untuk kembali ke versi lama'; return 1; }
    fi
    if [[ "$UPGRADE" == true && "$(printf '%s\n' "$minimum" "$installed" | sort -V | head -n 1)" != "$minimum" ]]; then
        fail "Paket ini memerlukan panel minimal $minimum. Terapkan versi perantara yang didukung terlebih dahulu."
        return 1
    fi
}

begin_transaction() {
    BACKUP_DIR="$(mktemp -d /var/backups/panel/install-XXXXXXXX)"
    chmod 0700 "$BACKUP_DIR"
    mkdir -p "$BACKUP_DIR/files"
    local path destination
    for path in "${MANAGED_FILES[@]}"; do
        if [[ -e "$path" || -L "$path" ]]; then
            # Coreutils 9.1 (Debian 12) mishandles -a --parents with absolute paths.
            # Build the destination explicitly; preserve file metadata and symlinks.
            destination="$BACKUP_DIR/files$path"
            mkdir -p -- "${destination%/*}" || {
                fail "Gagal membuat direktori snapshot untuk: $path"; return 1;
            }
            cp -a -- "$path" "$destination" || {
                fail "Gagal membuat snapshot file: $path"; return 1;
            }
        fi
    done
    # Snapshot existing panel config for review/recovery; tenant vhosts are never replaced.
    [[ ! -d /etc/panel ]] || cp -a /etc/panel "$BACKUP_DIR/panel-config"
    if systemctl is-active --quiet panel-api.service >/dev/null 2>&1; then API_WAS_ACTIVE=true; fi
    if systemctl is-active --quiet panel-agent.service >/dev/null 2>&1; then AGENT_WAS_ACTIVE=true; fi
    if systemctl is-active --quiet nginx.service >/dev/null 2>&1; then NGINX_WAS_ACTIVE=true; fi
    if systemctl is-enabled --quiet panel-api.service >/dev/null 2>&1; then API_WAS_ENABLED=true; fi
    if systemctl is-enabled --quiet panel-agent.service >/dev/null 2>&1; then AGENT_WAS_ENABLED=true; fi
    if systemctl is-enabled --quiet nginx.service >/dev/null 2>&1; then NGINX_WAS_ENABLED=true; fi
    TRANSACTION=true
    # Stop the old API first: v0.1.0 incorrectly owned /run/panel's lifecycle.
    if systemctl cat panel-api.service >/dev/null 2>&1; then systemctl stop panel-api.service; fi
    if systemctl cat panel-agent.service >/dev/null 2>&1; then systemctl stop panel-agent.service; fi
    if [[ -d /var/lib/panel ]]; then
        cp -a /var/lib/panel "$BACKUP_DIR/state"
        STATE_SAVED=true
    fi
    STATE_CHECKED=true
}

rollback() {
    echo '[ERROR] Deployment gagal; memulihkan snapshot sebelum instalasi.' >&2
    local failed=false path
    systemctl stop panel-api.service panel-agent.service || failed=true
    for path in "${MANAGED_FILES[@]}"; do
        rm -f "$path" || failed=true
        if [[ -e "$BACKUP_DIR/files$path" || -L "$BACKUP_DIR/files$path" ]]; then
            cp -a "$BACKUP_DIR/files$path" "$path" || failed=true
        fi
    done
    if [[ "$STATE_CHECKED" == true ]]; then
        # Core and operational state must roll back together; tenant files and analytics stay intact.
        for path in panel.db panel.db-wal panel.db-shm panel.operations.db panel.operations.db-wal panel.operations.db-shm; do
            rm -f "/var/lib/panel/$path" || failed=true
            if [[ "$STATE_SAVED" == true && -f "$BACKUP_DIR/state/$path" ]]; then
                cp -a "$BACKUP_DIR/state/$path" "/var/lib/panel/$path" || failed=true
            fi
        done
    fi
    systemctl daemon-reload || failed=true
    [[ "$API_WAS_ENABLED" == true ]] || systemctl disable panel-api.service >/dev/null 2>&1 || true
    [[ "$AGENT_WAS_ENABLED" == true ]] || systemctl disable panel-agent.service >/dev/null 2>&1 || true
    [[ "$NGINX_WAS_ENABLED" == true ]] || systemctl disable nginx.service >/dev/null 2>&1 || true
    if [[ "$AGENT_WAS_ACTIVE" == true ]]; then systemctl start panel-agent.service || failed=true; fi
    if [[ "$API_WAS_ACTIVE" == true ]]; then systemctl start panel-api.service || failed=true; fi
    if [[ "$NGINX_WAS_ACTIVE" == true ]]; then
        nginx -t && systemctl reload nginx || failed=true
    else
        systemctl stop nginx || failed=true
    fi
    echo "Snapshot pemulihan: $BACKUP_DIR" >&2
    [[ "$failed" == false ]] || echo '[ERROR] Pemulihan tidak lengkap; periksa journalctl dan snapshot.' >&2
}

on_error() {
    local code="$1"
    trap - ERR INT TERM
    set +e
    [[ "$TRANSACTION" != true ]] || rollback
    [[ -z "$STAGING_DIR" ]] || rm -rf "$STAGING_DIR"
    ADMIN_USERNAME=""; ADMIN_PASSWORD=""
    echo '[ERROR] Instalasi belum berhasil. Periksa: journalctl -u panel-api -u panel-agent -u nginx --no-pager -n 100' >&2
    exit "$code"
}

setup_identity() {
    getent group panel >/dev/null || groupadd -r panel
    getent group panel-tenants >/dev/null || groupadd -r panel-tenants
    if ! id panel-api >/dev/null 2>&1; then
        useradd -r -g panel -d /var/lib/panel -s /usr/sbin/nologin -c 'QonPanel API' panel-api
    else
        [[ "$(id -u panel-api)" != 0 ]] || { fail 'panel-api tidak boleh memiliki UID 0'; return 1; }
        usermod -g panel panel-api
    fi
    install -d -o root -g root -m 0755 /etc/panel /etc/panel/nginx
    install -d -o panel-api -g panel -m 0750 /var/lib/panel /var/lib/panel/backups
    install -d -o root -g panel -m 0755 /var/log/panel
    # /run/panel is created and secured by panel-agent.service on every boot.
    chown -R panel-api:panel /var/lib/panel
    local file
    for file in /var/lib/panel/panel.db{,-wal,-shm} /var/lib/panel/panel.operations.db{,-wal,-shm}; do
        [[ ! -f "$file" ]] || chmod 0640 "$file"
    done
}

install_binaries_and_units() {
    local binary
    for binary in panel-api panel-agent panelctl; do
        install -o root -g root -m 0755 "$BIN_SOURCE/$binary" "/usr/local/bin/.$binary.new"
        mv -f "/usr/local/bin/.$binary.new" "/usr/local/bin/$binary"
    done
    install -o root -g root -m 0644 "$SCRIPT_DIR/templates/systemd/panel-api.service" /etc/systemd/system/panel-api.service
    install -o root -g root -m 0644 "$SCRIPT_DIR/templates/systemd/panel-agent.service" /etc/systemd/system/panel-agent.service
    install -o root -g root -m 0644 "$SCRIPT_DIR/templates/systemd/panel-proc-hardening.service" /etc/systemd/system/panel-proc-hardening.service
    # Template and dash-prefix drop-ins also cover units generated by old builds.
    install -d -o root -g root -m 0755 /etc/systemd/system/panel-fpm@.service.d /etc/systemd/system/site-.service.d
    install -o root -g root -m 0644 "$SCRIPT_DIR/templates/systemd/panel-runtime-order.conf" /etc/systemd/system/panel-fpm@.service.d/10-panel-runtime-order.conf
    install -o root -g root -m 0644 "$SCRIPT_DIR/templates/systemd/panel-runtime-order.conf" /etc/systemd/system/site-.service.d/10-panel-runtime-order.conf
    install -o root -g root -m 0644 "$SCRIPT_DIR/templates/systemd/panel-update.service" /etc/systemd/system/panel-update.service
    install -o root -g root -m 0644 "$SCRIPT_DIR/templates/systemd/qonpanel-server-backup.service" /etc/systemd/system/qonpanel-server-backup.service
    install -d -o root -g root -m 0755 /usr/local/libexec
    install -o root -g root -m 0755 "$SCRIPT_DIR/scripts/server-backup-worker.py" /usr/local/libexec/qonpanel-server-backup
    # Atomic replacement keeps a running updater's script inode intact.
    install -o root -g root -m 0755 "$SCRIPT_DIR/scripts/panel-update-worker.sh" /usr/local/libexec/.panel-update-worker.new
    mv -f /usr/local/libexec/.panel-update-worker.new /usr/local/libexec/panel-update-worker
    install -o root -g root -m 0755 "$SCRIPT_DIR/scripts/bootstrap-install.sh" /usr/local/libexec/.panel-bootstrap.new
    mv -f /usr/local/libexec/.panel-bootstrap.new /usr/local/libexec/panel-bootstrap
    systemctl daemon-reload
}

initialize_database() {
    ADMIN_USERNAME=""; ADMIN_PASSWORD=""
    local result
    if ! result="$(runuser -u panel-api -- /usr/local/bin/panelctl --db-path /var/lib/panel/panel.db admin bootstrap --installer-output)"; then
        fail 'Inisialisasi akun/migrasi SQLite gagal sebagai panel-api; periksa panelctl dan akses database'
        return 1
    fi
    read_bootstrap_result "$result"
    if [[ "$UPGRADE" == true && -n "$ADMIN_USERNAME" ]]; then fail 'Upgrade tidak boleh membuat akun admin baru'; return 1; fi
    unset result
    chown -R panel-api:panel /var/lib/panel
    local file
    for file in /var/lib/panel/panel.db{,-wal,-shm} /var/lib/panel/panel.operations.db{,-wal,-shm}; do
        [[ ! -f "$file" ]] || chmod 0640 "$file"
    done
}

# Fixed protocol from the packaged CLI; never eval output or persist plaintext secrets.
read_bootstrap_result() {
    ADMIN_USERNAME=""; ADMIN_PASSWORD=""
    if [[ "$1" == preserved ]]; then return 0; fi
    if [[ "$1" =~ ^created\ (admin_[a-f0-9]{12})\ (Qp![a-f0-9]{32})$ ]]; then
        ADMIN_USERNAME="${BASH_REMATCH[1]}"
        ADMIN_PASSWORD="${BASH_REMATCH[2]}"
    else
        fail 'Respons bootstrap panelctl tidak valid; instalasi dibatalkan'
        return 1
    fi
}

print_installation_summary() {
    local server_ip="$1"
    printf '\n  QonPanel | Instalasi berhasil\n  ----------------------------------------\n'
    printf '  Panel     : http://%s:%s\n' "$server_ip" "$PANEL_PUBLIC_PORT"
    printf '  Status    : API, agent, Nginx, dan SQLite siap\n'
    printf '  Snapshot  : %s\n' "$BACKUP_DIR"
    if [[ -n "$ADMIN_USERNAME" && -n "$ADMIN_PASSWORD" ]]; then
        printf '\n  Akun administrator\n  Username  : %s\n  Password  : %s\n' "$ADMIN_USERNAME" "$ADMIN_PASSWORD"
        printf '  Simpan sekarang. Password hanya ditampilkan sekali.\n'
    else
        printf '\n  Akun administrator yang sudah ada dipertahankan.\n'
    fi
    printf '\n  Langkah berikutnya\n'
    printf '  Buka panel dan masuk dengan username serta password.\n'
    printf '  Ubah kredensial melalui Profil & keamanan, lalu aktifkan 2FA.\n'
    printf '\n  Bantuan CLI\n'
    printf '  panelctl --help                          Daftar semua perintah\n'
    printf '  panelctl admin --help                    Pengelolaan akun\n'
    printf '  sudo panelctl admin list                 Lihat username admin\n'
    printf '  sudo panelctl admin reset-password --username NAMA\n'
    printf '                                           Buat password pemulihan\n'
    printf '  sudo panelctl doctor                     Diagnosis layanan\n'
    printf '  panelctl backup --help                   Bantuan backup\n'
    printf '  Panduan: docs/admin-accounts.md (di paket rilis)\n\n'
    ADMIN_USERNAME=""; ADMIN_PASSWORD=""
}

render_panel_site() {
    sed "s/@PANEL_PUBLIC_PORT@/${PANEL_PUBLIC_PORT}/g" "$SCRIPT_DIR/templates/nginx/nusa-panel.conf"
}

is_packaged_nginx_default() {
    local path="$1" digest metadata package
    [[ -f "$path" && ! -L "$path" ]] || return 1
    digest="$(md5sum -- "$path")"; digest="${digest%% *}"
    for package in nginx-common nginx; do
        metadata="$(dpkg-query -W '-f=${Conffiles}\n' "$package" 2>/dev/null)" || continue
        if awk -v path="$path" -v digest="$digest" 'NF == 2 && $1 == path && $2 == digest { found=1 } END { exit !found }' <<< "$metadata"; then
            return 0
        fi
    done
    return 1
}

install_nginx_proxy() {
    install_nginx_proxy_at /etc/nginx /etc/panel/public-port
}

# Internal helper with explicit roots for rootless filesystem fixtures, not CLI/API inputs.
install_nginx_proxy_at() {
    local nginx_root="$1" port_file="$2" reject quarantine directives
    local disable_sites_default=false disable_conf_default=false
    reject="$nginx_root/conf.d/00-qpanel-reject-unknown.conf"
    if [[ -e "$reject" || -L "$reject" ]]; then
        [[ ! -L "$reject" ]] && cmp -s <(sed -E '1s/^# Managed by (Qpanel|Nusa Panel):/# Managed by QonPanel:/' "$reject") "$SCRIPT_DIR/templates/nginx/nusa-reject-unknown.conf" || {
            fail 'Catch-all QonPanel berisi perubahan operator; berkas tidak ditimpa'; return 1;
        }
    fi
    if [[ -e "$nginx_root/sites-enabled/default" || -L "$nginx_root/sites-enabled/default" ]]; then
      if [[ -L "$nginx_root/sites-enabled/default" ]] &&
       [[ "$(readlink -f "$nginx_root/sites-enabled/default")" == "$nginx_root/sites-available/default" ]] &&
       is_packaged_nginx_default "$nginx_root/sites-available/default"; then
        disable_sites_default=true
      else
        fail 'Vhost default Nginx sudah diubah atau checksum paket tidak tersedia; nonaktifkan default tersebut secara eksplisit. Berkas dipertahankan'; return 1
      fi
    fi
    if [[ -e "$nginx_root/conf.d/default.conf" || -L "$nginx_root/conf.d/default.conf" ]]; then
      if is_packaged_nginx_default "$nginx_root/conf.d/default.conf"; then disable_conf_default=true
      else fail 'conf.d/default.conf bukan default paket yang terverifikasi; berkas custom dipertahankan'; return 1
      fi
    fi
    install -d -m 0755 "$nginx_root/conf.d" "$nginx_root/sites-available" "$nginx_root/sites-enabled"
    STAGING_DIR="$(mktemp -d "$nginx_root/.nusa-stage-XXXXXXXX")"
    mkdir "$STAGING_DIR/sites-enabled" "$STAGING_DIR/sites-available" "$STAGING_DIR/conf.d"
    cp -a "$nginx_root/sites-enabled/." "$STAGING_DIR/sites-enabled/"
    cp -a "$nginx_root/sites-available/." "$STAGING_DIR/sites-available/"
    cp -a "$nginx_root/conf.d/." "$STAGING_DIR/conf.d/"
    if [[ "$disable_sites_default" == true ]]; then rm -f "$STAGING_DIR/sites-enabled/default"; fi
    if [[ "$disable_conf_default" == true ]]; then rm -f "$STAGING_DIR/conf.d/default.conf"; fi
    cp "$SCRIPT_DIR/templates/nginx/nusa-reject-unknown.conf" "$STAGING_DIR/conf.d/00-qpanel-reject-unknown.conf"
    rm -f "$STAGING_DIR/sites-enabled/nusa-panel.conf"
    render_panel_site > "$STAGING_DIR/nusa-panel.conf"
    ln -s "$STAGING_DIR/nusa-panel.conf" "$STAGING_DIR/sites-enabled/nusa-panel.conf"
    # Validate against the real global configuration and every existing tenant site.
    # Refuse unsupported custom include layouts instead of overwriting global nginx.conf.
    directives="$(sed 's/#.*//' "$nginx_root/nginx.conf")"
    grep -Eq "include[[:space:]]+$nginx_root/sites-enabled/\*;" <<< "$directives" &&
    grep -Eq "include[[:space:]]+$nginx_root/conf.d/\*.conf;" <<< "$directives" || {
        fail 'Nginx tidak memuat /etc/nginx/sites-enabled/*; konfigurasi custom dipertahankan'; return 1;
    }
    sed -e "s|include[[:space:]]\+$nginx_root/sites-enabled/\*;|include $STAGING_DIR/sites-enabled/*;|g" \
        -e "s|include[[:space:]]\+$nginx_root/conf.d/\*.conf;|include $STAGING_DIR/conf.d/*.conf;|g" \
        "$nginx_root/nginx.conf" > "$STAGING_DIR/nginx.conf"
    nginx -t -c "$STAGING_DIR/nginx.conf" || return 1
    install -m 0644 "$STAGING_DIR/nusa-panel.conf" "$nginx_root/sites-available/.nusa-panel.new"
    mv -f "$nginx_root/sites-available/.nusa-panel.new" "$nginx_root/sites-available/nusa-panel.conf"
    rm -f "$nginx_root/sites-enabled/.nusa-panel.new"
    ln -s "$nginx_root/sites-available/nusa-panel.conf" "$nginx_root/sites-enabled/.nusa-panel.new"
    mv -Tf "$nginx_root/sites-enabled/.nusa-panel.new" "$nginx_root/sites-enabled/nusa-panel.conf"
    install -m 0644 "$STAGING_DIR/conf.d/00-qpanel-reject-unknown.conf" "$reject.new"
    mv -f "$reject.new" "$reject"
    if [[ "$disable_sites_default" == true ]]; then
        quarantine="$(mktemp "$nginx_root/sites-available/default.qpanel-disabled-XXXXXXXX")"
        mv -f "$nginx_root/sites-enabled/default" "$quarantine"
    fi
    if [[ "$disable_conf_default" == true ]]; then
        quarantine="$(mktemp "$nginx_root/conf.d/default.conf.qpanel-disabled-XXXXXXXX")"
        mv -f "$nginx_root/conf.d/default.conf" "$quarantine"
    fi
    nginx -t || return 1
    printf '%s\n' "$PANEL_PUBLIC_PORT" > "$port_file"
    chmod 0644 "$port_file"
    rm -rf "$STAGING_DIR"
    STAGING_DIR=""
}

activate_nginx_proxy() {
    nginx -t
    systemctl enable nginx.service
    if systemctl is-active --quiet nginx.service; then
        systemctl reload nginx.service
    else
        systemctl start nginx.service
    fi
}

configure_firewall() {
    [[ "$SKIP_FIREWALL" == false ]] || return 0
    if command -v ufw >/dev/null && LC_ALL=C ufw status | grep -q '^Status: active'; then
        ufw allow "${PANEL_PUBLIC_PORT}/tcp" comment 'QonPanel public proxy'
    elif command -v firewall-cmd >/dev/null && firewall-cmd --state >/dev/null 2>&1; then
        firewall-cmd --permanent --add-port="${PANEL_PUBLIC_PORT}/tcp"
        firewall-cmd --add-port="${PANEL_PUBLIC_PORT}/tcp"
    elif nft list ruleset 2>/dev/null | grep -Eq 'hook input|policy drop'; then
        echo "[PERINGATAN] Firewall nftables custom terdeteksi. Pastikan ${PANEL_PUBLIC_PORT}/tcp diizinkan; aturan tidak diubah." >&2
    fi
    echo "Firewall/security group penyedia harus mengizinkan ${PANEL_PUBLIC_PORT}/tcp. Jangan buka 7700."
}

wait_health() {
    local url="$1" attempts="${2:-75}" attempt status
    for ((attempt=1; attempt<=attempts; attempt++)); do
        if status="$(curl --noproxy '*' --connect-timeout 1 --max-time 2 -fsS \
            --output /dev/null --write-out '%{http_code}' "$url" 2>/dev/null)" && [[ "$status" == 200 ]]; then
            return 0
        fi
        if (( attempt < attempts )); then sleep 0.2; fi
    done
    fail "Health check timeout setelah $attempts percobaan: $url"
}

validate_api_binding() {
    local bindings
    if ! bindings="$(ss -H -lnt 'sport = :7700' 2>/dev/null | awk 'NF {print $4}')"; then
        fail 'Gagal membaca listener API dengan ss (port 7700)'
        return 1
    fi
    # Accept multiple entries for the expected address, but no other interface.
    if grep -Fxq '127.0.0.1:7700' <<< "$bindings" &&
        ! grep -Fxqv '127.0.0.1:7700' <<< "$bindings"; then
        return 0
    fi
    {
        printf '[ERROR] API listener validation failed.\nExpected:\n  127.0.0.1:7700\nDetected:\n'
        printf '%s\n' "${bindings:-<none>}" | sed 's/^/  /'
    } >&2
    return 1
}

wait_agent_ready() {
    local attempt
    for attempt in {1..30}; do
        if [[ -S /run/panel/agent.sock ]] &&
            runuser -u panel-api -- /usr/local/bin/panelctl ipc-check >/dev/null 2>&1; then return 0; fi
        sleep 1
    done
    fail 'Agent tidak siap menerima RPC dari identity panel-api'
}

validate_health() {
    local service
    for service in panel-agent panel-api nginx; do
        systemctl is-active --quiet "$service" || { fail "Layanan gagal: $service"; return 1; }
    done
    [[ -S /run/panel/agent.sock ]] || { fail 'Socket agent tidak tersedia'; return 1; }
    [[ "$(stat -c '%U:%G:%a' /run/panel)" == root:panel:750 ]] || { fail 'Permission /run/panel salah'; return 1; }
    [[ "$(stat -c '%U:%G:%a' /run/panel/agent.sock)" == root:panel:660 ]] || { fail 'Permission agent.sock salah'; return 1; }
    runuser -u panel-api -- test -w /run/panel/agent.sock
    runuser -u panel-api -- /usr/local/bin/panelctl ipc-check
    [[ "$(runuser -u panel-api -- sqlite3 /var/lib/panel/panel.db 'PRAGMA quick_check;')" == ok ]] || {
        fail 'Integritas SQLite gagal'; return 1;
    }
    local file
    [[ "$(stat -c '%U:%G:%a' /var/lib/panel)" == panel-api:panel:750 ]] || { fail 'Permission direktori SQLite salah'; return 1; }
    for file in /var/lib/panel/panel.db{,-wal,-shm} /var/lib/panel/panel.operations.db{,-wal,-shm}; do
        if [[ -e "$file" && "$(stat -c '%U:%G' "$file")" != panel-api:panel ]]; then
            fail "Ownership SQLite salah: $file"; return 1
        fi
    done
    wait_health "http://127.0.0.1:${PANEL_PUBLIC_PORT}/healthz"
}

main() {
    # Preserve custom port on repeat install/upgrade unless explicitly overridden.
    local args_status=0
    parse_args "$@" || args_status=$?
    [[ "$args_status" != 2 ]] || return 0
    [[ "$args_status" == 0 ]] || return "$args_status"
    if [[ "$PORT_EXPLICIT" != true && -r /etc/panel/public-port ]]; then
        read -r PANEL_PUBLIC_PORT < /etc/panel/public-port
    fi
    validate_port
    check_platform
    acquire_install_lock
    find_release
    validate_upgrade
    install_packages
    check_port_available
    install -d -o root -g root -m 0700 /var/backups/panel
    trap 'on_error $?' ERR
    trap 'on_error 130' INT
    trap 'on_error 143' TERM
    begin_transaction
    setup_identity
    install_binaries_and_units
    initialize_database
    install_nginx_proxy
    systemctl enable panel-agent.service panel-api.service
    systemctl start panel-agent.service
    wait_agent_ready
    systemctl start panel-api.service
    # Type=simple reports started before bind/migrations finish: poll readiness first.
    wait_health http://127.0.0.1:7700/healthz
    validate_api_binding
    activate_nginx_proxy
    validate_health
    configure_firewall
    record_installed_release
    if [[ -n "$RESTORE_DIGEST" ]]; then
        /usr/bin/python3 /usr/local/libexec/qonpanel-server-backup --approve "$RESTORE_DIGEST"
    fi
    TRANSACTION=false
    trap - ERR INT TERM
    local server_ip
    server_ip="$(hostname -I 2>/dev/null | awk '{print $1}')" || server_ip="127.0.0.1"
    server_ip="${server_ip:-127.0.0.1}"
    [[ "$server_ip" != *:* ]] || server_ip="[$server_ip]"
    print_installation_summary "$server_ip"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then main "$@"; fi
