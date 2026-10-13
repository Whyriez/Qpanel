#!/usr/bin/env bash
# ==============================================================================
# Nusa Panel - Public Bootstrap Installer
# Repository: Whyriez/Vpanel
# ==============================================================================

export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
set -Eeuo pipefail

# Configuration
GITHUB_OWNER="${NUSA_PANEL_OWNER:-Whyriez}"
GITHUB_REPO="${NUSA_PANEL_REPO:-Vpanel}"
GITHUB_API_URL="${NUSA_PANEL_GITHUB_API_URL:-https://api.github.com}"
GITHUB_DOWNLOAD_URL="${NUSA_PANEL_GITHUB_DOWNLOAD_URL:-https://github.com}"

TMP_DIR=""

# Initialize color and formatting defaults
RED=""
GREEN=""
BLUE=""
CYAN=""
BOLD=""
NC=""

# Cleanup handler on exit or signals
cleanup() {
    local exit_code=$?
    if [[ -n "${TMP_DIR:-}" && -d "${TMP_DIR}" ]]; then
        rm -rf "${TMP_DIR}"
    fi
    exit "${exit_code}"
}

# Color and formatting setup (auto-disabled if non-TTY or NO_COLOR set)
setup_colors() {
    if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
        RED='\033[0;31m'
        GREEN='\033[0;32m'
        BLUE='\033[0;34m'
        CYAN='\033[0;36m'
        BOLD='\033[1m'
        NC='\033[0m'
    else
        RED=''
        GREEN=''
        BLUE=''
        CYAN=''
        BOLD=''
        NC=''
    fi
}

log_step() {
    echo -e "${CYAN}$*${NC}"
}

log_sub() {
    echo -e "      $*"
}

log_error() {
    echo -e "${RED}ERROR: $*${NC}" >&2
}

# 1. Root check
check_root() {
    if [[ "${NUSA_PANEL_SKIP_ROOT_CHECK:-0}" == "1" ]]; then
        return 0
    fi
    if [[ "$(id -u)" -ne 0 ]]; then
        log_error "This installer must be run as root."
        echo "" >&2
        echo "Use:" >&2
        echo "curl -fsSL <URL> | sudo bash" >&2
        exit 1
    fi
}

# 2. OS check (Linux only)
detect_os() {
    if [[ "${NUSA_PANEL_SKIP_OS_CHECK:-0}" == "1" ]]; then
        return 0
    fi
    local os
    os="$(uname -s)"
    if [[ "${os}" != "Linux" ]]; then
        log_error "Nusa Panel can only be installed on Linux."
        exit 1
    fi
}

# 3. CPU architecture detection & mapping
detect_arch() {
    local raw_arch="$1"
    case "${raw_arch}" in
        x86_64|amd64)
            echo "x86_64-unknown-linux-gnu"
            ;;
        aarch64|arm64)
            echo "aarch64-unknown-linux-gnu"
            ;;
        *)
            log_error "Unsupported CPU architecture: ${raw_arch}"
            echo "Supported architectures:" >&2
            echo "- x86_64 / amd64" >&2
            echo "- aarch64 / arm64" >&2
            return 1
            ;;
    esac
}

# 4. Required command dependencies check
check_dependencies() {
    local missing=()
    for cmd in curl tar sha256sum mktemp uname grep sed awk; do
        if ! command -v "${cmd}" >/dev/null 2>&1; then
            missing+=("${cmd}")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Required tool(s) missing: ${missing[*]}"
        echo "Please install the missing tools using your system package manager." >&2
        exit 1
    fi
}

# 5. Version tag validation
validate_version() {
    local version="$1"
    if [[ ! "${version}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
        log_error "Invalid release tag: ${version}"
        return 1
    fi
    return 0
}

# 6. Resolve latest release from GitHub
resolve_latest_version() {
    if [[ -n "${NUSA_PANEL_VERSION:-}" ]]; then
        if ! validate_version "${NUSA_PANEL_VERSION}"; then
            exit 1
        fi
        echo "${NUSA_PANEL_VERSION}"
        return 0
    fi

    local tag=""
    local api_url="${GITHUB_API_URL}/repos/${GITHUB_OWNER}/${GITHUB_REPO}/releases/latest"

    # Attempt 1: GitHub API releases/latest
    local api_response
    if api_response="$(curl --fail --location --silent --show-error \
        -H "Accept: application/vnd.github.v3+json" \
        -H "User-Agent: NusaPanel-Bootstrap" \
        "${api_url}" 2>/dev/null)"; then
        tag="$(printf '%s\n' "${api_response}" | grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' | head -n 1 | sed -E 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')"
    fi

    # Attempt 2: Fallback to GitHub releases/latest redirect URL if API failed (e.g. rate limit)
    if [[ -z "${tag}" ]]; then
        local redirect_url="${GITHUB_DOWNLOAD_URL}/${GITHUB_OWNER}/${GITHUB_REPO}/releases/latest"
        local loc_header
        loc_header="$(curl --silent --head "${redirect_url}" 2>/dev/null | grep -i '^location:' | head -n 1 | tr -d '\r')"
        if [[ -n "${loc_header}" ]]; then
            tag="$(printf '%s\n' "${loc_header}" | sed -E 's/.*\/tag\/([^\/[:space:]]+).*/\1/')"
        fi
    fi

    if [[ -z "${tag}" ]]; then
        log_error "Failed to resolve latest Nusa Panel release from GitHub."
        exit 1
    fi

    if ! validate_version "${tag}"; then
        log_error "Failed to resolve latest Nusa Panel release from GitHub."
        exit 1
    fi

    echo "${tag}"
}

# 7. Build deterministic asset name and download URLs
build_asset_name() {
    local version="$1"
    local target="$2"
    echo "nusa-panel-${version}-${target}.tar.gz"
}

build_asset_url() {
    local version="$1"
    local asset_name="$2"
    echo "${GITHUB_DOWNLOAD_URL}/${GITHUB_OWNER}/${GITHUB_REPO}/releases/download/${version}/${asset_name}"
}

build_checksum_url() {
    local version="$1"
    echo "${GITHUB_DOWNLOAD_URL}/${GITHUB_OWNER}/${GITHUB_REPO}/releases/download/${version}/SHA256SUMS"
}

download_file() {
    local url="$1"
    local dest="$2"
    curl --fail --location --silent --show-error "${url}" -o "${dest}"
}

# 8. Verify SHA256 checksum of downloaded artifact
verify_checksum() {
    local checksum_file="$1"
    local asset_file="$2"
    local asset_name="$3"

    if [[ ! -f "${checksum_file}" ]]; then
        log_error "Checksum file not found: ${checksum_file}"
        return 1
    fi

    if [[ ! -f "${asset_file}" ]]; then
        log_error "Asset file not found: ${asset_file}"
        return 1
    fi

    local expected_hash
    expected_hash="$(grep -E "(^|[[:space:]]|\*)${asset_name}\$" "${checksum_file}" | head -n 1 | awk '{print $1}' | tr -d '\r')"

    if [[ -z "${expected_hash}" ]]; then
        log_error "Checksum entry for ${asset_name} not found in $(basename "${checksum_file}")."
        return 1
    fi

    local actual_hash
    actual_hash="$(sha256sum "${asset_file}" | awk '{print $1}')"

    local exp_lower="${expected_hash,,}"
    local act_lower="${actual_hash,,}"

    if [[ "${exp_lower}" != "${act_lower}" ]]; then
        log_error "Release checksum verification failed."
        echo "The downloaded archive will not be installed." >&2
        return 1
    fi

    return 0
}

# 9. Validate archive file paths before extracting
validate_entry_path() {
    local entry="$1"
    local clean_entry="${entry#./}"

    # Reject absolute paths (starting with /)
    if [[ "${clean_entry}" == /* ]]; then
        log_error "Release archive contains unsafe paths."
        return 1
    fi

    # Reject path traversal (containing .. path components)
    if [[ "${clean_entry}" == ".." || "${clean_entry}" == ../* || "${clean_entry}" == */.. || "${clean_entry}" == */../* ]]; then
        log_error "Release archive contains unsafe paths."
        return 1
    fi

    return 0
}

validate_archive() {
    local archive_file="$1"

    local entries
    if ! entries="$(tar -tzf "${archive_file}" 2>/dev/null)"; then
        log_error "Release archive is corrupt or cannot be read."
        return 1
    fi

    local entry
    while IFS= read -r entry; do
        [[ -z "${entry}" ]] && continue
        if ! validate_entry_path "${entry}"; then
            return 1
        fi
    done <<< "${entries}"

    return 0
}

# 10. Extract archive and locate bundled install.sh
extract_and_locate_installer() {
    local archive_file="$1"
    local dest_dir="$2"
    local version="$3"
    local target="$4"

    tar -xzf "${archive_file}" -C "${dest_dir}" || {
        log_error "Failed to extract release archive."
        return 1
    }

    local primary_dir="${dest_dir}/nusa-panel-${version}-${target}"
    local installer="${primary_dir}/install.sh"

    if [[ -f "${installer}" ]]; then
        echo "${primary_dir}"
        return 0
    fi

    # Fallback if tar extracted directly into dest_dir
    if [[ -f "${dest_dir}/install.sh" ]]; then
        echo "${dest_dir}"
        return 0
    fi

    log_error "Release package is invalid: install.sh not found."
    return 1
}

# 11. Execute bundled installer
run_installer() {
    local working_dir="$1"
    shift

    cd "${working_dir}"
    bash ./install.sh "$@"
}

# Main execution flow
main() {
    trap cleanup EXIT INT TERM
    setup_colors

    echo "Nusa Panel Installer"
    echo ""

    check_root
    check_dependencies

    log_step "[1/7] Checking environment..."
    detect_os
    log_sub "Linux detected"

    local raw_arch target
    raw_arch="$(uname -m)"
    target="$(detect_arch "${raw_arch}")" || exit 1
    log_sub "Architecture: ${raw_arch}"

    log_step "[2/7] Resolving latest release..."
    local version
    version="$(resolve_latest_version)"
    log_sub "Latest version: ${version}"

    TMP_DIR="$(mktemp -d -t nusapanel-install.XXXXXXXXXX 2>/dev/null || mktemp -d /tmp/nusapanel-install.XXXXXXXXXX)"

    local asset_name asset_url checksum_url
    asset_name="$(build_asset_name "${version}" "${target}")"
    asset_url="$(build_asset_url "${version}" "${asset_name}")"
    checksum_url="$(build_checksum_url "${version}")"

    log_step "[3/7] Downloading release..."
    log_sub "${asset_name}"
    if ! download_file "${asset_url}" "${TMP_DIR}/${asset_name}"; then
        log_error "Failed to download release asset: ${asset_name}"
        exit 1
    fi
    if ! download_file "${checksum_url}" "${TMP_DIR}/SHA256SUMS"; then
        log_error "Failed to download SHA256SUMS"
        exit 1
    fi

    log_step "[4/7] Verifying SHA-256 checksum..."
    if ! verify_checksum "${TMP_DIR}/SHA256SUMS" "${TMP_DIR}/${asset_name}" "${asset_name}"; then
        exit 1
    fi
    log_sub "OK"

    log_step "[5/7] Validating release archive..."
    if ! validate_archive "${TMP_DIR}/${asset_name}"; then
        exit 1
    fi
    log_sub "OK"

    log_step "[6/7] Extracting release..."
    local extracted_dir
    if ! extracted_dir="$(extract_and_locate_installer "${TMP_DIR}/${asset_name}" "${TMP_DIR}" "${version}" "${target}")"; then
        exit 1
    fi
    log_sub "OK"

    log_step "[7/7] Starting Nusa Panel installer..."
    echo ""

    local exit_code=0
    run_installer "${extracted_dir}" "$@" || exit_code=$?

    if [[ ${exit_code} -ne 0 ]]; then
        log_error "Nusa Panel installation failed."
        exit "${exit_code}"
    fi

    exit 0
}

# Run main if executed directly or piped into bash
if [[ "${BASH_SOURCE[0]}" == "${0}" ]] || [[ -z "${BASH_SOURCE[0]:-}" ]] || [[ "${BASH_SOURCE[0]}" == "bash" ]]; then
    main "$@"
fi
