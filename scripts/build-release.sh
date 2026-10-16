#!/usr/bin/env bash
# ==============================================================================
# Qpanel - Release Packaging Script
# Targets: x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${1:-x86_64-unknown-linux-gnu}"
OUTPUT_DIR="${SCRIPT_DIR}/release"
VERSION="$(grep -m1 '^version' "${SCRIPT_DIR}/Cargo.toml" | cut -d '"' -f 2 || echo "0.1.0")"
cd "$SCRIPT_DIR"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$SCRIPT_DIR/target/release-native/$TARGET}"
BIN_DIR="$CARGO_TARGET_DIR/$TARGET/release"

echo "================================================================================"
echo "          MEMBANGUN PAKET RILIS QPANEL v${VERSION} (${TARGET})              "
echo "================================================================================"

mkdir -p "${OUTPUT_DIR}"

# 1. Build frontend bundle
if [[ -d "${SCRIPT_DIR}/web" && -f "${SCRIPT_DIR}/web/package.json" ]]; then
    echo "==> Membangun aset antarmuka web (npm run build)..."
    (cd "${SCRIPT_DIR}/web" && npm run build)
fi

# 2. Build Rust release binaries
echo "==> Mengompilasi biner release untuk target ${TARGET}..."
bash "$SCRIPT_DIR/scripts/compile-release.sh" "$TARGET"

bash "${SCRIPT_DIR}/scripts/check-glibc.sh" \
    "$BIN_DIR/panel-api" \
    "$BIN_DIR/panel-agent" \
    "$BIN_DIR/panelctl"

# 3. Create staging directory
STAGE_NAME="nusa-panel-v${VERSION}-${TARGET}"
STAGE_DIR="${OUTPUT_DIR}/${STAGE_NAME}"
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/bin"
mkdir -p "${STAGE_DIR}/templates/systemd"
mkdir -p "${STAGE_DIR}/templates/nginx"

# Copy binaries
cp -f "$BIN_DIR/panel-agent" "${STAGE_DIR}/bin/"
cp -f "$BIN_DIR/panel-api" "${STAGE_DIR}/bin/"
cp -f "$BIN_DIR/panelctl" "${STAGE_DIR}/bin/"
chmod 0755 "${STAGE_DIR}/bin/"*

# Copy templates and scripts
cp -f "${SCRIPT_DIR}/templates/systemd/"*.service "${STAGE_DIR}/templates/systemd/"
cp -f "${SCRIPT_DIR}/templates/nginx/nusa-panel.conf" "${STAGE_DIR}/templates/nginx/"
cp -f "${SCRIPT_DIR}/install.sh" "${STAGE_DIR}/"
cp -f "${SCRIPT_DIR}/uninstall.sh" "${STAGE_DIR}/"
chmod 0755 "${STAGE_DIR}/install.sh" "${STAGE_DIR}/uninstall.sh"

# Copy web dist assets if present
if [[ -d "${SCRIPT_DIR}/web/dist" ]]; then
    mkdir -p "${STAGE_DIR}/web"
    cp -r "${SCRIPT_DIR}/web/dist" "${STAGE_DIR}/web/"
fi

# 4. Create tar.gz archive
ARCHIVE_NAME="${STAGE_NAME}.tar.gz"
echo "==> Mengemas tarball arsip ${ARCHIVE_NAME}..."
tar -czf "${OUTPUT_DIR}/${ARCHIVE_NAME}" -C "${OUTPUT_DIR}" "${STAGE_NAME}"

# 5. Compute SHA256 checksum
(cd "${OUTPUT_DIR}" && sha256sum "${ARCHIVE_NAME}" > "${ARCHIVE_NAME}.sha256")

# Cleanup staging directory
rm -rf "${STAGE_DIR}"

echo "================================================================================"
echo "[SUKSES] Paket rilis selesai dibuat:"
echo "  Arsip    : ${OUTPUT_DIR}/${ARCHIVE_NAME}"
echo "  Checksum : ${OUTPUT_DIR}/${ARCHIVE_NAME}.sha256"
echo "================================================================================"
