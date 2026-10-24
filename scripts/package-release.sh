#!/usr/bin/env bash
# Shared packaging entry point for local builds and GitHub Releases.
# This script only reads build artifacts; it never executes panel binaries.
set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${1:-x86_64-unknown-linux-gnu}"
case "$TARGET" in
    x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu) ;;
    *) echo "Unsupported release target: $TARGET" >&2; exit 1 ;;
esac
VERSION="$(awk '
    /^\[workspace[.]package\]$/ { package=1; next }
    /^\[/ { package=0 }
    package && /^version[[:space:]]*=/ { split($0, parts, "\""); print parts[2]; exit }
' "$REPO/Cargo.toml")"
TAG="${2:-v$VERSION}"
[[ "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ && "$TAG" == "v$VERSION" ]] || {
    printf 'Release tag must match Cargo.toml: expected v%s, received %s\n' "$VERSION" "$TAG" >&2
    exit 1
}
[[ $# -le 3 ]] || { echo 'Too many packaging arguments' >&2; exit 1; }
case "${3:-}" in
    --check-version) printf 'Release version validated: %s (%s)\n' "$TAG" "$TARGET"; exit 0 ;;
    '') ;;
    *) echo 'Unknown packaging option' >&2; exit 1 ;;
esac
cd "$REPO"
BIN_DIR="${CARGO_TARGET_DIR:-$REPO/target/release-native/$TARGET}/$TARGET/release"
OUTPUT_DIR="$REPO/release"
STAGE_NAME="qonpanel-$TAG-$TARGET"
ARCHIVE_NAME="$STAGE_NAME.tar.gz"

# One explicit payload list for every release path. Missing files fail the build.
sources=(
    "$BIN_DIR/panel-api" "$BIN_DIR/panel-agent" "$BIN_DIR/panelctl"
    install.sh uninstall.sh UPGRADE_FROM
    scripts/bootstrap-install.sh scripts/panel-update-worker.sh
    scripts/server-backup-worker.py templates/systemd/qonpanel-server-backup.service
    templates/systemd/panel-update.service
    templates/systemd/panel-api.service
    templates/systemd/panel-agent.service
    templates/systemd/panel-proc-hardening.service
    templates/systemd/panel-runtime-order.conf
    templates/nginx/nusa-panel.conf
    templates/nginx/nusa-reject-unknown.conf
    ADMIN_ACCOUNTS.md
    UPDATING.md
    SERVER_BACKUP.md
    web/dist/index.html
)
destinations=(
    bin/panel-api bin/panel-agent bin/panelctl
    install.sh uninstall.sh UPGRADE_FROM
    scripts/bootstrap-install.sh scripts/panel-update-worker.sh
    scripts/server-backup-worker.py templates/systemd/qonpanel-server-backup.service
    templates/systemd/panel-update.service
    templates/systemd/panel-api.service
    templates/systemd/panel-agent.service
    templates/systemd/panel-proc-hardening.service
    templates/systemd/panel-runtime-order.conf
    templates/nginx/nusa-panel.conf
    templates/nginx/nusa-reject-unknown.conf
    docs/admin-accounts.md
    docs/updating.md
    docs/server-backup.md
    web/dist/index.html
)
missing=0
for source in "${sources[@]}"; do
    if [[ ! -f "$source" || ! -r "$source" || ! -s "$source" ]]; then
        printf 'Required release file missing or empty: %s\n' "$source" >&2
        missing=1
    fi
done
if ! compgen -G 'web/dist/assets/*.js' >/dev/null; then
    echo 'Required release assets missing: web/dist/assets/*.js (run the frontend build)' >&2
    missing=1
fi
[[ "$missing" == 0 ]] || exit 1

# Stage in a private, unique directory so failures cannot publish a partial archive.
mkdir -p "$OUTPUT_DIR"
temporary="$(mktemp -d "$OUTPUT_DIR/.package.XXXXXXXX")"
trap 'rm -rf "$temporary"' EXIT
stage="$temporary/$STAGE_NAME"
for index in "${!sources[@]}"; do
    destination="$stage/${destinations[$index]}"
    mkdir -p "$(dirname "$destination")"
    cp "${sources[$index]}" "$destination"
done
cp -R web/dist/. "$stage/web/dist/"
chmod 0755 "$stage/bin/"* "$stage/install.sh" "$stage/uninstall.sh"
tar -czf "$temporary/$ARCHIVE_NAME" -C "$temporary" "$STAGE_NAME"
# Verify every required member in the completed archive, then generate its checksum.
listing="$(tar -tzf "$temporary/$ARCHIVE_NAME")"
for destination in "${destinations[@]}"; do
    grep -Fxq "$STAGE_NAME/$destination" <<< "$listing" || {
        printf 'Incomplete release archive: %s\n' "$destination" >&2
        exit 1
    }
done
(cd "$temporary" && sha256sum "$ARCHIVE_NAME" > "$ARCHIVE_NAME.sha256")
# A separate legacy root is required by already-installed bootstraps.
LEGACY_NAME="nusa-panel-$TAG-$TARGET"
cp -R "$stage" "$temporary/$LEGACY_NAME"
tar -czf "$temporary/$LEGACY_NAME.tar.gz" -C "$temporary" "$LEGACY_NAME"
legacy_listing="$(tar -tzf "$temporary/$LEGACY_NAME.tar.gz")"
for destination in "${destinations[@]}"; do
    grep -Fxq "$LEGACY_NAME/$destination" <<< "$legacy_listing" || exit 1
done
(cd "$temporary" && sha256sum "$LEGACY_NAME.tar.gz" > "$LEGACY_NAME.tar.gz.sha256")
mv "$temporary/$ARCHIVE_NAME" "$temporary/$ARCHIVE_NAME.sha256" \
    "$temporary/$LEGACY_NAME.tar.gz" "$temporary/$LEGACY_NAME.tar.gz.sha256" "$OUTPUT_DIR/"
printf '[SUKSES] Paket rilis: %s/%s\nChecksum: %s/%s.sha256\n' \
    "$OUTPUT_DIR" "$ARCHIVE_NAME" "$OUTPUT_DIR" "$ARCHIVE_NAME"
