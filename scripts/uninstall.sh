#!/usr/bin/env bash
# scripts/uninstall.sh: Convenience wrapper calling root uninstall.sh
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec "${SCRIPT_DIR}/uninstall.sh" "$@"
