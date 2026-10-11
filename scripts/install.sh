#!/usr/bin/env bash
# scripts/install.sh: Convenience wrapper calling root install.sh
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec "${SCRIPT_DIR}/install.sh" "$@"
