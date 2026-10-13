#!/usr/bin/env bash
# Safe local tests: no agent, real services, firewall changes, or production writes.
set -Eeuo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$REPO/install.sh"
count=0
pass() { count=$((count + 1)); printf 'PASS %s\n' "$1"; }

[[ "$PATH" == /usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin ]]
pass 'canonical installer PATH'
[[ "$PANEL_PUBLIC_PORT" == 17890 ]]
pass 'default public port'
for valid in 1024 17890 19090 65535; do
    PANEL_PUBLIC_PORT="$valid"; validate_port; [[ "$PANEL_PUBLIC_PORT" == "$valid" ]]
done
pass 'valid configurable ports'
for invalid in '' 80 443 1023 7700 65536 99999999999999999999 -1 12.3 '17890;id' abc; do
    PANEL_PUBLIC_PORT="$invalid"
    if validate_port >/dev/null 2>&1; then echo "Accepted invalid port: $invalid" >&2; exit 1; fi
done
pass 'invalid, internal and injectable ports rejected'
parse_args --port 19090 --unattended --upgrade
[[ "$PANEL_PUBLIC_PORT" == 19090 && "$PORT_EXPLICIT" == true && "$UPGRADE" == true ]]
pass 'argument forwarding'
if parse_args --port >/dev/null 2>&1; then exit 1; fi
pass 'missing port value rejected'
if parse_args --unknown >/dev/null 2>&1; then exit 1; fi
pass 'unknown option rejected'
PANEL_PUBLIC_PORT=17890
config="$(render_panel_site)"
[[ "$config" == *'listen 17890;'* && "$config" == *'listen [::]:17890;'* ]]
[[ "$config" == *'proxy_pass http://127.0.0.1:7700;'* && "$config" == *'proxy_set_header Upgrade $http_upgrade;'* ]]
pass 'IPv4/IPv6 public proxy and WebSocket headers'
PANEL_PUBLIC_PORT=19090
[[ "$(render_panel_site)" == *'listen 19090;'* ]]
pass 'custom port rendering'

# Simulate a conflicting listener; must fail without calling systemctl/kill.
ss() { printf 'LISTEN 0 511 0.0.0.0:19090 *:* users:(("other",pid=42,fd=6))\n'; }
systemctl() { echo 'Unexpected service mutation' >&2; exit 99; }
if check_port_available >/dev/null 2>&1; then exit 1; fi
pass 'occupied public port rejected without stopping services'
ss() { return 0; }
check_port_available
pass 'unused public port accepted'

# Mock health failures. Installer must stop at a failed component.
systemctl() { return 1; }
if validate_health >/dev/null 2>&1; then exit 1; fi
pass 'failed service fails installer health gate'
curl() { return 1; }
sleep() { :; }
if wait_health http://127.0.0.1:7700/healthz >/dev/null 2>&1; then exit 1; fi
pass 'failed HTTP health gate returns nonzero'
curl() { printf '200'; }
wait_health http://127.0.0.1:7700/healthz
pass 'healthy HTTP endpoint accepted'

# Only the public panel port may be added to an existing firewall.
ufw() {
    if [[ "$1" == status ]]; then printf 'Status: active\n'; else printf '%s\n' "$*"; fi
}
SKIP_FIREWALL=false
firewall_output="$(configure_firewall)"
[[ "$firewall_output" == *'allow 19090/tcp'* ]]
[[ "$firewall_output" != *'allow 7700'* && "$firewall_output" != *'allow 80/'* && "$firewall_output" != *'allow 443/'* ]]
pass 'active UFW opens only the configured public port'
SKIP_FIREWALL=true
[[ -z "$(configure_firewall)" ]]
pass 'skip firewall preserves existing rules'

api="$REPO/templates/systemd/panel-api.service"
agent="$REPO/templates/systemd/panel-agent.service"
grep -qx 'User=panel-api' "$api"
grep -qx 'Group=panel' "$api"
grep -qx 'WorkingDirectory=/var/lib/panel' "$api"
grep -qx 'ReadWritePaths=/var/lib/panel' "$api"
grep -qx 'ProtectSystem=strict' "$api"
! grep -q '^RuntimeDirectory=' "$api"
pass 'API service group, working directory, sandbox and runtime lifecycle'
grep -qx 'User=root' "$agent"
grep -qx 'Group=panel' "$agent"
grep -qx 'RuntimeDirectory=panel' "$agent"
grep -qx 'RuntimeDirectoryMode=0750' "$agent"
pass 'agent alone owns boot-time runtime directory'
grep -q 'DEFAULT_DATABASE_PATH.*"/var/lib/panel/panel.db"' "$REPO/crates/panel-core/src/production.rs"
grep -q 'DEFAULT_DATABASE_PATH' "$REPO/crates/panel-api/src/config.rs"
grep -q 'DEFAULT_DATABASE_PATH' "$REPO/crates/panel-cli/src/main.rs"
pass 'API and CLI share an absolute production database default'
grep -q 'runs-on: ubuntu-22.04' "$REPO/.github/workflows/release.yml"
grep -q 'scripts/check-glibc.sh' "$REPO/.github/workflows/release.yml"
grep -q 'templates/nginx/nusa-panel.conf' "$REPO/scripts/build-release.sh"
pass 'release baseline gate and proxy template packaging'

# Exercise the actual failure trap in an isolated child with every host mutation mocked.
for failing in validate_health install_nginx_proxy; do
    status=0
    output="$(bash -c '
        source "$1/install.sh"
        PORT_EXPLICIT=true
        check_platform() { :; }; find_release() { :; }; install_packages() { :; }
        check_port_available() { :; }; install() { :; }
        begin_transaction() { TRANSACTION=true; }
        setup_identity() { :; }; install_binaries_and_units() { :; }
        initialize_database() { :; }; install_nginx_proxy() { :; }
        systemctl() { :; }; wait_agent_ready() { :; }; validate_health() { :; }; configure_firewall() { :; }
        wait_health() { :; }; validate_api_binding() { :; }; activate_nginx_proxy() { :; }
        rollback() { echo ROLLBACK_CALLED; }
        if [[ "$2" == validate_health ]]; then
            validate_health() { return 1; }
        else
            install_nginx_proxy() { return 1; }
        fi
        main --unattended
    ' test "$REPO" "$failing" 2>&1)" || status=$?
    [[ "$status" != 0 && "$output" == *ROLLBACK_CALLED* && "$output" != *'NUSA PANEL INSTALLED'* ]]
    pass "$failing failure invokes rollback and suppresses success"
done

# Exercise file rollback with a temporary rootless fixture and mocked service manager.
fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT
original="$fixture/panel.conf"
new="$fixture/new-unit.service"
tenant="$fixture/tenant.conf"
printf 'old panel\n' > "$original"
printf 'tenant unchanged\n' > "$tenant"
BACKUP_DIR="$fixture/backup"
mkdir -p "$BACKUP_DIR/files"
cp -a --parents "$original" "$BACKUP_DIR/files/"
printf 'broken candidate\n' > "$original"
printf 'new file\n' > "$new"
MANAGED_FILES=("$original" "$new")
STATE_SAVED=false
systemctl() { return 0; }
rollback >/dev/null 2>&1
[[ "$(cat "$original")" == 'old panel' && ! -e "$new" && "$(cat "$tenant")" == 'tenant unchanged' ]]
pass 'rollback restores previous artifacts, removes new artifacts and preserves tenant fixture'

# The release gate examines required symbol versions, without executing any binary.
for version in GLIBC_2.35 GLIBC_2.39 none; do
    status=0
    output="$(bash -c '
        FAKE_GLIBC="$2"; export FAKE_GLIBC
        readelf() { printf "%s\n" "$FAKE_GLIBC"; }; export -f readelf
        bash "$1/scripts/check-glibc.sh" fixture-binary
    ' test "$REPO" "$version" 2>&1)" || status=$?
    if [[ "$version" == GLIBC_2.35 ]]; then [[ "$status" == 0 ]]; else [[ "$status" != 0 ]]; fi
    pass "GLIBC release gate: $version"
done
printf '%s regression checks passed. VM acceptance was not run.\n' "$count"
bash "$REPO/scripts/tests/install-startup-race-test.sh"
