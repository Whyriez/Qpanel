#!/usr/bin/env bash
# P10-T6: compile release binaries natively on the supported GLIBC build baseline.
set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${1:-x86_64-unknown-linux-gnu}"
case "$TARGET" in
    x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu) ;;
    *) echo "Unsupported release target: $TARGET" >&2; exit 1 ;;
esac
cd "$REPO"
host="$(rustc -vV | sed -n 's/^host: //p')"
[[ "$host" == "$TARGET" ]] || {
    echo "Native release build required: target=$TARGET, Rust host=$host. Use the matching Ubuntu 22.04 runner." >&2
    exit 1
}
libc="$(getconf GNU_LIBC_VERSION)"
[[ "$libc" == 'glibc 2.35' ]] || {
    echo "Release build requires GLIBC 2.35 (Ubuntu 22.04); detected: $libc" >&2
    exit 1
}
# Host build scripts/proc macros must never reuse a different runtime's target/.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO/target/release-native/$TARGET}"
export CARGO_INCREMENTAL=0
printf 'Native release build: %s; %s; artifacts: %s\n' "$host" "$libc" "$CARGO_TARGET_DIR"
cargo build --locked --release --target "$TARGET" -p panel-agent -p panel-api -p panel-cli
