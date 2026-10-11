#!/usr/bin/env bash
# ==============================================================================
# Nusa Panel - Idempotent Production Installer
# https://github.com/nusa-panel/panel
# Target: Ubuntu 22.04 LTS, Ubuntu 24.04 LTS, Debian 12
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PANEL_PORT=3000
UNATTENDED=false
FORCE_OS=false
SKIP_FIREWALL=false

# Colors for terminal output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

print_banner() {
    cat << "EOF"
================================================================================
                    _   _ _   _ ____    _      ____   _    _   _ _____ _     
                   | \ | | | | / ___|  / \    |  _ \ / \  | \ | | ____| |    
                   |  \| | | | \___ \ / _ \   | |_) / _ \ |  \| |  _| | |    
                   | |\  | |_| |___) / ___ \  |  __/ ___ \| |\  | |___| |___ 
                   |_| \_|\___/|____/_/   \_\ |_| /_/   \_\_| \_|_____|_____|
================================================================================
EOF
}

print_help() {
    cat << EOF
Penggunaan: $0 [OPSI]

Opsi yang tersedia:
  --port <PORT>        Tentukan port web Nusa Panel (default: 3000)
  --unattended         Jalankan dalam mode otomatis tanpa konfirmasi interaktif
  --force-os           Lewati verifikasi ketat versi sistem operasi
  --skip-firewall      Lewati rekomendasi konfigurasi firewall
  -h, --help           Tampilkan pesan bantuan ini

Contoh:
  sudo ./install.sh
  sudo ./install.sh --unattended --port 8080
EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --port)
            PANEL_PORT="$2"
            shift 2
            ;;
        --unattended)
            UNATTENDED=true
            shift
            ;;
        --force-os)
            FORCE_OS=true
            shift
            ;;
        --skip-firewall)
            SKIP_FIREWALL=true
            shift
            ;;
        -h|--help)
            print_help
            exit 0
            ;;
        *)
            echo -e "${RED}[ERROR] Opsi tidak dikenal: $1${NC}" >&2
            print_help
            exit 1
            ;;
    esac
done

# 1. Root permission check
check_root() {
    if [[ "$(id -u)" -ne 0 ]]; then
        echo -e "${RED}[ERROR] Skrip penginstal ini harus dijalankan dengan hak akses root (sudo).${NC}" >&2
        exit 1
    fi
}

# 2. Operating System validation (Ubuntu 22.04/24.04, Debian 12)
check_os() {
    echo -e "${BLUE}[1/8] Memeriksa kompatibilitas sistem operasi...${NC}"
    if [[ ! -f /etc/os-release ]]; then
        if [[ "$FORCE_OS" == "false" ]]; then
            echo -e "${RED}[ERROR] Berkas /etc/os-release tidak ditemukan.${NC}" >&2
            exit 1
        fi
        return 0
    fi

    # Source os-release
    . /etc/os-release

    OS_ID="${ID:-unknown}"
    OS_VER="${VERSION_ID:-unknown}"

    echo "      Terdeteksi OS: ${NAME:-$OS_ID} ${VERSION:-$OS_VER}"

    local supported=false
    if [[ "$OS_ID" == "ubuntu" && ("$OS_VER" == "22.04" || "$OS_VER" == "24.04") ]]; then
        supported=true
    elif [[ "$OS_ID" == "debian" && "$OS_VER" =~ ^12 ]]; then
        supported=true
    fi

    if [[ "$supported" == "true" ]]; then
        echo -e "      ${GREEN}[OK] Sistem operasi didukung secara resmi.${NC}"
    else
        if [[ "$FORCE_OS" == "true" ]]; then
            echo -e "      ${YELLOW}[PERINGATAN] OS ${OS_ID} ${OS_VER} bukan target resmi, namun dilanjutkan (--force-os).${NC}"
        else
            echo -e "${RED}[ERROR] Nusa Panel hanya mendukung Ubuntu 22.04 LTS, Ubuntu 24.04 LTS, atau Debian 12.${NC}" >&2
            echo -e "        Gunakan flag --force-os jika Anda yakin ingin melanjutkan." >&2
            exit 1
        fi
    fi
}

# 3. Kernel and cgroup v2 verification
check_cgroup() {
    echo -e "${BLUE}[2/8] Memeriksa hierarki cgroup v2 & kernel...${NC}"
    KERNEL_VER="$(uname -r)"
    echo "      Kernel: ${KERNEL_VER}"

    if [[ -f /sys/fs/cgroup/cgroup.controllers ]]; then
        echo -e "      ${GREEN}[OK] cgroup v2 controllers aktif.${NC}"
    else
        echo -e "      ${YELLOW}[PERINGATAN] Hierarki cgroup v2 tidak terdeteksi di /sys/fs/cgroup/cgroup.controllers.${NC}"
        echo -e "      Isolasi resource CPU & Memori tenant memerlukan cgroup v2."
        echo -e "      Pertimbangkan untuk menambahkan 'systemd.unified_cgroup_hierarchy=1' pada parameter bootloader kernel."
    fi
}

# 4. Prerequisites package installation
install_packages() {
    echo -e "${BLUE}[3/8] Memperbarui indeks paket dan memasang dependensi...${NC}"
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq

    local packages=(
        "nginx"
        "nftables"
        "curl"
        "tar"
        "ca-certificates"
        "sqlite3"
        "systemd"
        "acl"
        "util-linux"
    )

    apt-get install -y --no-install-recommends "${packages[@]}"
    echo -e "      ${GREEN}[OK] Seluruh paket dependensi terpasang.${NC}"
}

# 5. System users and directory hierarchy
setup_users_and_directories() {
    echo -e "${BLUE}[4/8] Mengonfigurasi pengguna sistem dan hierarki direktori...${NC}"

    # Group: panel
    if ! getent group panel >/dev/null 2>&1; then
        groupadd -r panel
        echo "      Grup 'panel' dibuat."
    fi

    # User: panel-api (non-root service daemon)
    if ! id -u panel-api >/dev/null 2>&1; then
        useradd -r -g panel -d /var/lib/panel -s /usr/sbin/nologin -c "Vpanel API Daemon" panel-api
        echo "      Pengguna sistem 'panel-api' dibuat."
    fi

    # Directories
    mkdir -p /etc/panel/nginx
    mkdir -p /var/lib/panel/backups
    mkdir -p /var/backups/panel
    mkdir -p /var/log/panel
    mkdir -p /run/panel

    # Permissions & Ownerships
    chmod 0755 /etc/panel
    chmod 0755 /etc/panel/nginx
    chmod 0750 /var/lib/panel
    chown -R panel-api:panel /var/lib/panel

    chmod 0700 /var/backups/panel
    chown root:root /var/backups/panel

    chmod 0755 /var/log/panel
    chown root:panel /var/log/panel

    chmod 0755 /run/panel
    chown root:panel /run/panel

    echo -e "      ${GREEN}[OK] Direktori sistem terkonfigurasi.${NC}"
}

# 6. Baseline Nginx configuration
setup_nginx_baseline() {
    echo -e "${BLUE}[5/8] Memasang konfigurasi baseline Nginx...${NC}"

    local baseline_conf="/etc/panel/nginx/baseline.conf"
    if [[ ! -f "$baseline_conf" ]]; then
        cat << 'EOF' > "$baseline_conf"
# Nusa Panel Global Nginx Baseline Configuration
server_names_hash_bucket_size 128;
client_max_body_size 128M;
proxy_buffer_size 128k;
proxy_buffers 4 256k;
proxy_busy_buffers_size 256k;
EOF
    fi

    # Ensure /etc/nginx/conf.d/panel.conf includes panel configs if needed
    local nginx_inc="/etc/nginx/conf.d/panel.conf"
    if [[ ! -f "$nginx_inc" ]]; then
        cat << 'EOF' > "$nginx_inc"
# Include Nusa Panel Baseline and Virtual Hosts
include /etc/panel/nginx/*.conf;
EOF
    fi

    if nginx -t >/dev/null 2>&1; then
        systemctl reload nginx || systemctl restart nginx || true
        echo -e "      ${GREEN}[OK] Sintaks Nginx valid dan konfigurasi dimuat.${NC}"
    else
        echo -e "      ${YELLOW}[PERINGATAN] Sintaks Nginx memiliki peringatan. Periksa dengan 'nginx -t'.${NC}"
    fi
}

# 7. Install binaries and systemd units
install_binaries_and_services() {
    echo -e "${BLUE}[6/8] Memasang biner Nusa Panel dan layanan systemd...${NC}"

    # Search for binaries: locally built or script directory
    local bin_src_dir=""
    if [[ -f "${SCRIPT_DIR}/target/release/panel-api" ]]; then
        bin_src_dir="${SCRIPT_DIR}/target/release"
    elif [[ -f "${SCRIPT_DIR}/bin/panel-api" ]]; then
        bin_src_dir="${SCRIPT_DIR}/bin"
    elif [[ -f "/usr/local/bin/panel-api" ]]; then
        bin_src_dir="/usr/local/bin"
    fi

    if [[ -n "$bin_src_dir" && "$bin_src_dir" != "/usr/local/bin" ]]; then
        for b in panel-api panel-agent panelctl; do
            if [[ -f "${bin_src_dir}/${b}" ]]; then
                cp -f "${bin_src_dir}/${b}" "/usr/local/bin/${b}"
                chmod 0755 "/usr/local/bin/${b}"
            fi
        done
        echo "      Biner disalin dari ${bin_src_dir} ke /usr/local/bin/"
    else
        echo "      Menggunakan biner yang sudah ada di /usr/local/bin/"
    fi

    # Install systemd service unit files
    local template_dir="${SCRIPT_DIR}/templates/systemd"
    if [[ -d "$template_dir" ]]; then
        cp -f "${template_dir}/panel-agent.service" /etc/systemd/system/panel-agent.service
        cp -f "${template_dir}/panel-api.service" /etc/systemd/system/panel-api.service
        chmod 0644 /etc/systemd/system/panel-agent.service /etc/systemd/system/panel-api.service
    fi

    systemctl daemon-reload
    systemctl enable panel-agent.service panel-api.service >/dev/null 2>&1 || true
    systemctl restart panel-agent.service panel-api.service >/dev/null 2>&1 || true

    echo -e "      ${GREEN}[OK] Layanan systemd diaktifkan dan dijalankan.${NC}"
}

# 8. Database migration & setup token generation
setup_database_and_token() {
    echo -e "${BLUE}[7/8] Menginisialisasi basis data dan token first-run...${NC}"

    local db_file="/var/lib/panel/panel.db"
    
    # Run migrations and doctor auto-fix via panelctl if available
    if command -v panelctl >/dev/null 2>&1; then
        panelctl --db-path "$db_file" doctor --fix >/dev/null 2>&1 || true
        
        # Generate setup token
        SETUP_TOKEN="$(panelctl --db-path "$db_file" setup-token generate 2>/dev/null || echo "")"
        if [[ -z "$SETUP_TOKEN" ]]; then
            # Token might already exist or admin already created
            SETUP_TOKEN="(Token sudah pernah dibuat atau admin telah terdaftar)"
        fi
    else
        SETUP_TOKEN="(Jalankan 'panelctl setup-token generate' setelah biner terpasang)"
    fi

    # Ensure permissions on SQLite db
    if [[ -f "$db_file" ]]; then
        chown panel-api:panel "$db_file"
        chmod 0640 "$db_file"
    fi

    echo -e "      ${GREEN}[OK] Basis data diinisialisasi.${NC}"
}

# 9. Firewall guidance and configuration
configure_firewall() {
    echo -e "${BLUE}[8/8] Memeriksa status firewall...${NC}"

    if [[ "$SKIP_FIREWALL" == "true" ]]; then
        echo "      Pemeriksaan firewall dilewati (--skip-firewall)."
        return 0
    fi

    local ufw_active=false
    if command -v ufw >/dev/null 2>&1; then
        if ufw status | grep -q "Status: active"; then
            ufw_active=true
        fi
    fi

    if [[ "$ufw_active" == "true" ]]; then
        echo -e "      ${YELLOW}UFW Firewall terdeteksi aktif.${NC}"
        echo "      Membuka port yang dibutuhkan:"
        ufw allow 80/tcp comment 'Nusa Panel HTTP' >/dev/null 2>&1 || true
        ufw allow 443/tcp comment 'Nusa Panel HTTPS' >/dev/null 2>&1 || true
        ufw allow "${PANEL_PORT}/tcp" comment 'Nusa Panel Web API' >/dev/null 2>&1 || true
        echo -e "      ${GREEN}[OK] Aturan port 80, 443, dan ${PANEL_PORT} ditambahkan ke UFW.${NC}"
    else
        echo "      Firewall UFW tidak aktif. Pastikan port berikut terbuka pada firewall penyedia hosting Anda:"
        echo "        - Port 80/tcp   : Akses HTTP & tantangan ACME Let's Encrypt"
        echo "        - Port 443/tcp  : Akses HTTPS aman"
        echo "        - Port ${PANEL_PORT}/tcp : Antarmuka Web Nusa Panel"
        echo "        - Port 22/tcp   : Akses administrasi SSH"
    fi
}

main() {
    print_banner
    check_root
    check_os
    check_cgroup
    install_packages
    setup_users_and_directories
    setup_nginx_baseline
    install_binaries_and_services
    setup_database_and_token
    configure_firewall

    # Detect primary IP address
    SERVER_IP="$(hostname -I 2>/dev/null | awk '{print $1}' || echo "127.0.0.1")"

    echo ""
    echo "================================================================================"
    echo -e "               ${GREEN}SELAMAT! INSTALASI NUSA PANEL TELAH SELESAI${NC}              "
    echo "================================================================================"
    echo ""
    echo -e "  Alamat Web Panel : ${CYAN}http://${SERVER_IP}:${PANEL_PORT}${NC}"
    echo -e "  Token Pengaturan : ${YELLOW}${SETUP_TOKEN}${NC}"
    echo ""
    echo "  Langkah Selanjutnya:"
    echo "  1. Buka URL di atas melalui peramban web Anda."
    echo "  2. Masukkan Token Pengaturan di atas untuk membuat akun Administrator pertama."
    echo "  3. Token bersifat sekali pakai dan kedaluwarsa demi keamanan sistem."
    echo ""
    echo "  Perintah Bantuan CLI:"
    echo "  - Cek kesehatan panel  : panelctl doctor"
    echo "  - Cadangan sistem      : panelctl backup create"
    echo "  - Bantuan umum         : panelctl --help"
    echo "================================================================================"
    echo ""
}

main "$@"
