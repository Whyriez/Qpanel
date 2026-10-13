#!/usr/bin/env bash
# Nusa Panel production install/upgrade. Never run on a development host.
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
set -Eeuo pipefail

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
SETUP_TOKEN=""
MANAGED_FILES=(
    /usr/local/bin/panel-api /usr/local/bin/panel-agent /usr/local/bin/panelctl
    /etc/systemd/system/panel-api.service /etc/systemd/system/panel-agent.service
    /etc/nginx/sites-available/nusa-panel.conf /etc/nginx/sites-enabled/nusa-panel.conf
    /etc/panel/public-port
)

fail() { echo "[ERROR] $*" >&2; return 1; }
print_help() {
    cat <<'EOF'
Penggunaan: ./install.sh [opsi]
  --port <PORT>    Port publik 1024..65535, selain 7700 (default: 17890)
  --unattended     Instalasi otomatis
  --upgrade        Upgrade transaksional dengan snapshot dan rollback
  --force-os       Lewati pemeriksaan versi distro
  --skip-firewall  Jangan menambahkan aturan firewall
  -h, --help       Tampilkan bantuan
PANEL_PUBLIC_PORT juga dapat mengatur port publik. API internal selalu 127.0.0.1:7700.
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --port)
                [[ $# -ge 2 ]] || { fail '--port membutuhkan nilai'; return 1; }
                PANEL_PUBLIC_PORT="$2"; PORT_EXPLICIT=true; shift 2 ;;
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
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y --no-install-recommends nginx nftables curl tar ca-certificates \
        sqlite3 systemd acl util-linux iproute2 passwd
}

find_release() {
    BIN_SOURCE="${SCRIPT_DIR}/bin"
    [[ -d "$BIN_SOURCE" ]] || BIN_SOURCE="${SCRIPT_DIR}/target/release"
    local binary
    for binary in panel-api panel-agent panelctl; do
        [[ -x "$BIN_SOURCE/$binary" ]] || { fail "Biner paket tidak ditemukan: $BIN_SOURCE/$binary"; return 1; }
        "$BIN_SOURCE/$binary" --version
    done
    [[ -r "$SCRIPT_DIR/templates/systemd/panel-api.service" &&
       -r "$SCRIPT_DIR/templates/systemd/panel-agent.service" &&
       -r "$SCRIPT_DIR/templates/nginx/nusa-panel.conf" ]] || {
        fail 'Template systemd/Nginx paket tidak lengkap'; return 1;
    }
}

begin_transaction() {
    BACKUP_DIR="$(mktemp -d /var/backups/panel/install-XXXXXXXX)"
    chmod 0700 "$BACKUP_DIR"
    mkdir -p "$BACKUP_DIR/files"
    local path
    for path in "${MANAGED_FILES[@]}"; do
        if [[ -e "$path" || -L "$path" ]]; then
            cp -a --parents "$path" "$BACKUP_DIR/files/"
        fi
    done
    # Snapshot existing panel config for review/recovery; tenant vhosts are never replaced.
    [[ ! -d /etc/panel ]] || cp -a /etc/panel "$BACKUP_DIR/panel-config"
    systemctl is-active --quiet panel-api && API_WAS_ACTIVE=true
    systemctl is-active --quiet panel-agent && AGENT_WAS_ACTIVE=true
    systemctl is-active --quiet nginx && NGINX_WAS_ACTIVE=true
    systemctl is-enabled --quiet panel-api && API_WAS_ENABLED=true
    systemctl is-enabled --quiet panel-agent && AGENT_WAS_ENABLED=true
    systemctl is-enabled --quiet nginx && NGINX_WAS_ENABLED=true
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
        # Only restore the panel database; tenant files and live analytics stay intact.
        for path in panel.db panel.db-wal panel.db-shm; do
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
    echo '[ERROR] Instalasi belum berhasil. Periksa: journalctl -u panel-api -u panel-agent -u nginx --no-pager -n 100' >&2
    exit "$code"
}

setup_identity() {
    getent group panel >/dev/null || groupadd -r panel
    if ! id panel-api >/dev/null 2>&1; then
        useradd -r -g panel -d /var/lib/panel -s /usr/sbin/nologin -c 'Nusa Panel API' panel-api
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
    for file in /var/lib/panel/panel.db{,-wal,-shm}; do
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
    systemctl daemon-reload
}

initialize_database() {
    # The setup-token command runs migrations, always under the service identity.
    # It is idempotent: an existing administrator/token is never overwritten.
    runuser -u panel-api -- /usr/local/bin/panelctl --db-path /var/lib/panel/panel.db setup-token status
    local admins token
    admins="$(runuser -u panel-api -- sqlite3 /var/lib/panel/panel.db "SELECT COUNT(*) FROM users WHERE role='admin';")"
    if [[ "$admins" == 0 ]]; then
        token="$(runuser -u panel-api -- sqlite3 /var/lib/panel/panel.db "SELECT value FROM settings WHERE key='setup.first_run_token';")"
        if [[ -n "$token" ]]; then
            SETUP_TOKEN="$token"
        else
            runuser -u panel-api -- /usr/local/bin/panelctl --db-path /var/lib/panel/panel.db setup-token generate >/dev/null
            SETUP_TOKEN="$(runuser -u panel-api -- sqlite3 /var/lib/panel/panel.db "SELECT value FROM settings WHERE key='setup.first_run_token';")"
            [[ -n "$SETUP_TOKEN" ]] || { fail 'Token first-run tidak berhasil dibuat'; return 1; }
        fi
    fi
    chown -R panel-api:panel /var/lib/panel
    local file
    for file in /var/lib/panel/panel.db{,-wal,-shm}; do
        [[ ! -f "$file" ]] || chmod 0640 "$file"
    done
}

render_panel_site() {
    sed "s/@PANEL_PUBLIC_PORT@/${PANEL_PUBLIC_PORT}/g" "$SCRIPT_DIR/templates/nginx/nusa-panel.conf"
}

install_nginx_proxy() {
    install -d -m 0755 /etc/nginx/sites-available /etc/nginx/sites-enabled
    STAGING_DIR="$(mktemp -d /etc/nginx/.nusa-stage-XXXXXXXX)"
    mkdir "$STAGING_DIR/sites-enabled"
    cp -a /etc/nginx/sites-enabled/. "$STAGING_DIR/sites-enabled/"
    rm -f "$STAGING_DIR/sites-enabled/nusa-panel.conf"
    render_panel_site > "$STAGING_DIR/nusa-panel.conf"
    ln -s "$STAGING_DIR/nusa-panel.conf" "$STAGING_DIR/sites-enabled/nusa-panel.conf"
    # Validate against the real global configuration and every existing tenant site.
    # Refuse unsupported custom include layouts instead of overwriting global nginx.conf.
    grep -Eq 'include[[:space:]]+/etc/nginx/sites-enabled/\*;' /etc/nginx/nginx.conf || {
        fail 'Nginx tidak memuat /etc/nginx/sites-enabled/*; konfigurasi custom dipertahankan'; return 1;
    }
    sed "s|include[[:space:]]\+/etc/nginx/sites-enabled/\*;|include $STAGING_DIR/sites-enabled/*;|g" \
        /etc/nginx/nginx.conf > "$STAGING_DIR/nginx.conf"
    nginx -t -c "$STAGING_DIR/nginx.conf"
    install -m 0644 "$STAGING_DIR/nusa-panel.conf" /etc/nginx/sites-available/.nusa-panel.new
    mv -f /etc/nginx/sites-available/.nusa-panel.new /etc/nginx/sites-available/nusa-panel.conf
    rm -f /etc/nginx/sites-enabled/.nusa-panel.new
    ln -s /etc/nginx/sites-available/nusa-panel.conf /etc/nginx/sites-enabled/.nusa-panel.new
    mv -Tf /etc/nginx/sites-enabled/.nusa-panel.new /etc/nginx/sites-enabled/nusa-panel.conf
    nginx -t
    systemctl enable nginx
    if systemctl is-active --quiet nginx; then systemctl reload nginx; else systemctl start nginx; fi
    printf '%s\n' "$PANEL_PUBLIC_PORT" > /etc/panel/public-port
    chmod 0644 /etc/panel/public-port
    rm -rf "$STAGING_DIR"
    STAGING_DIR=""
}

configure_firewall() {
    [[ "$SKIP_FIREWALL" == false ]] || return 0
    if command -v ufw >/dev/null && LC_ALL=C ufw status | grep -q '^Status: active'; then
        ufw allow "${PANEL_PUBLIC_PORT}/tcp" comment 'Nusa Panel public proxy'
    elif command -v firewall-cmd >/dev/null && firewall-cmd --state >/dev/null 2>&1; then
        firewall-cmd --permanent --add-port="${PANEL_PUBLIC_PORT}/tcp"
        firewall-cmd --add-port="${PANEL_PUBLIC_PORT}/tcp"
    elif nft list ruleset 2>/dev/null | grep -Eq 'hook input|policy drop'; then
        echo "[PERINGATAN] Firewall nftables custom terdeteksi. Pastikan ${PANEL_PUBLIC_PORT}/tcp diizinkan; aturan tidak diubah." >&2
    fi
    echo "Firewall/security group penyedia harus mengizinkan ${PANEL_PUBLIC_PORT}/tcp. Jangan buka 7700."
}

wait_health() {
    local url="$1" attempt
    for attempt in {1..30}; do
        if curl --connect-timeout 1 --max-time 2 -fsS "$url" >/dev/null; then return 0; fi
        sleep 1
    done
    fail "Health HTTP gagal: $url"
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
    for file in /var/lib/panel/panel.db{,-wal,-shm}; do
        if [[ -e "$file" && "$(stat -c '%U:%G' "$file")" != panel-api:panel ]]; then
            fail "Ownership SQLite salah: $file"; return 1
        fi
    done
    local bindings
    bindings="$(ss -H -lnt 'sport = :7700' | awk '{print $4}')"
    [[ "$bindings" == 127.0.0.1:7700 ]] || { fail 'API harus listen hanya pada 127.0.0.1:7700'; return 1; }
    wait_health http://127.0.0.1:7700/healthz
    nginx -t
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
    find_release
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
    validate_health
    configure_firewall
    TRANSACTION=false
    trap - ERR INT TERM
    local server_ip
    server_ip="$(hostname -I 2>/dev/null | awk '{print $1}')" || server_ip="127.0.0.1"
    server_ip="${server_ip:-127.0.0.1}"
    [[ "$server_ip" != *:* ]] || server_ip="[$server_ip]"
    printf '\nNUSA PANEL INSTALLED\nPanel: http://%s:%s\nInternal API: 127.0.0.1:7700\n' "$server_ip" "$PANEL_PUBLIC_PORT"
    printf 'Services: panel-agent active, panel-api active, nginx active\nHealth: SQLite OK, IPC Agent OK, Internal API OK, Web Proxy OK\n'
    [[ -z "$SETUP_TOKEN" ]] || printf 'Token pengaturan: %s\n' "$SETUP_TOKEN"
    printf 'Snapshot sebelum perubahan: %s\n' "$BACKUP_DIR"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then main "$@"; fi
