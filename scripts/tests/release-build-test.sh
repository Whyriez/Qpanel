#!/usr/bin/env bash
# Mocked release compilation/packaging: never execute cargo, containers, or agents.
set -Eeuo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT
MOCK_REPO="$fixture/repo"
TRACE="$fixture/build.trace"
export MOCK_REPO TRACE
mkdir -p "$MOCK_REPO/scripts" "$MOCK_REPO/templates" "$MOCK_REPO/target/release/build"
cp "$REPO/scripts/compile-release.sh" "$REPO/scripts/build-release.sh" "$REPO/scripts/check-glibc.sh" "$MOCK_REPO/scripts/"
cp "$REPO/install.sh" "$REPO/uninstall.sh" "$MOCK_REPO/"
cp "$REPO/ADMIN_ACCOUNTS.md" "$MOCK_REPO/"
cp -r "$REPO/templates/systemd" "$REPO/templates/nginx" "$MOCK_REPO/templates/"
printf '[workspace.package]\nversion = "0.1.1"\n' > "$MOCK_REPO/Cargo.toml"
printf 'incompatible GLIBC_2.39 cached host build script\n' > "$MOCK_REPO/target/release/build/legacy-build-script"

rustc() { [[ "$*" == -vV ]] || return 99; printf 'host: %s\n' "$FAKE_HOST"; }
getconf() { [[ "$*" == GNU_LIBC_VERSION ]] || return 99; printf '%s\n' "$FAKE_LIBC"; }
cargo() {
    printf 'cargo:%s:%s\n' "$CARGO_TARGET_DIR" "$*" >> "$TRACE"
    [[ "$CARGO_INCREMENTAL" == 0 ]] || return 99
    [[ "$*" == "build --locked --release --target $FAKE_HOST -p panel-agent -p panel-api -p panel-cli" ]] || return 99
    [[ "$PWD" == "$MOCK_REPO" && "$CARGO_TARGET_DIR" != "$MOCK_REPO/target" ]] || return 99
    [[ "${FAKE_CARGO_FAIL:-0}" == 0 ]] || return 9
    local output="$CARGO_TARGET_DIR/$FAKE_HOST/release" binary
    mkdir -p "$output"
    for binary in panel-agent panel-api panelctl; do
        printf 'fresh native %s %s\n' "$FAKE_HOST" "$binary" > "$output/$binary"
    done
}
readelf() {
    printf 'readelf:%s\n' "$*" >> "$TRACE"
    [[ "$1" == --version-info && -f "$2" ]] || return 99
    printf '%s\n' "${FAKE_SYMBOLS:-GLIBC_2.35}"
}
export -f rustc getconf cargo readelf
FAKE_LIBC='glibc 2.35'
FAKE_CARGO_FAIL=0
FAKE_SYMBOLS=GLIBC_2.35
export FAKE_HOST FAKE_LIBC FAKE_CARGO_FAIL FAKE_SYMBOLS
count=0
pass() { count=$((count + 1)); printf 'PASS %s\n' "$1"; }

for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
    FAKE_HOST="$target"
    : > "$TRACE"
    # An incompatible old target tree must not be used for compilation or packaging.
    mkdir -p "$MOCK_REPO/target/$target/release"
    for binary in panel-agent panel-api panelctl; do
        printf 'stale incompatible cached binary\n' > "$MOCK_REPO/target/$target/release/$binary"
    done
    output="$(unset CARGO_TARGET_DIR; bash "$MOCK_REPO/scripts/compile-release.sh" "$target" 2>&1)"
    grep -Fq "cargo:$MOCK_REPO/target/release-native/$target:" "$TRACE"
    [[ -f "$MOCK_REPO/target/release-native/$target/$target/release/panel-api" ]]
    pass "native $target uses isolated build directory and locked dependencies"

    : > "$TRACE"
    output="$(unset CARGO_TARGET_DIR; bash "$MOCK_REPO/scripts/build-release.sh" "$target" 2>&1)"
    archive="$MOCK_REPO/release/nusa-panel-v0.1.1-$target.tar.gz"
    # Checksums contain archive basenames and must be verified from the release directory.
    (cd "$MOCK_REPO/release" && sha256sum -c "$(basename "$archive").sha256" --status)
    [[ "$(grep -c '^readelf:' "$TRACE")" == 3 ]]
    extract="$fixture/extracted-$target"
    mkdir -p "$extract"
    tar -xzf "$archive" -C "$extract"
    for binary in panel-agent panel-api panelctl; do
        [[ "$(cat "$extract/nusa-panel-v0.1.1-$target/bin/$binary")" == "fresh native $target $binary" ]]
    done
    [[ -f "$extract/nusa-panel-v0.1.1-$target/templates/nginx/nusa-panel.conf" ]]
    cmp -s "$REPO/templates/nginx/nusa-reject-unknown.conf" "$extract/nusa-panel-v0.1.1-$target/templates/nginx/nusa-reject-unknown.conf"
    cmp -s "$REPO/ADMIN_ACCOUNTS.md" "$extract/nusa-panel-v0.1.1-$target/docs/admin-accounts.md"
    [[ -f "$extract/nusa-panel-v0.1.1-$target/templates/systemd/panel-api.service" ]]
    cmp -s "$REPO/templates/systemd/panel-proc-hardening.service" "$extract/nusa-panel-v0.1.1-$target/templates/systemd/panel-proc-hardening.service"
    [[ "$output" == *'[SUKSES]'* ]]
    pass "native $target GLIBC gate and tarball read freshly compiled artifacts"
done

FAKE_HOST=x86_64-unknown-linux-gnu
for custom_dir in "$fixture/custom build output" 'target/custom output'; do
    : > "$TRACE"
    output="$(export CARGO_TARGET_DIR="$custom_dir"; bash "$MOCK_REPO/scripts/build-release.sh" "$FAKE_HOST" 2>&1)"
    grep -Fq "cargo:$custom_dir:" "$TRACE"
    grep -Fq "readelf:--version-info $custom_dir/$FAKE_HOST/release/panel-api" "$TRACE"
    [[ "$output" == *'[SUKSES]'* ]]
    pass "packaging honors explicit CARGO_TARGET_DIR: $custom_dir"
done

for invalid in wrong-host new-glibc old-glibc unsupported-target; do
    FAKE_HOST=x86_64-unknown-linux-gnu
    FAKE_LIBC='glibc 2.35'
    target="$FAKE_HOST"
    case "$invalid" in
        wrong-host) target=aarch64-unknown-linux-gnu ;;
        new-glibc) FAKE_LIBC='glibc 2.39' ;;
        old-glibc) FAKE_LIBC='glibc 2.23' ;;
        unsupported-target) target=wasm32-unknown-unknown ;;
    esac
    : > "$TRACE"
    status=0
    output="$(unset CARGO_TARGET_DIR; bash "$MOCK_REPO/scripts/compile-release.sh" "$target" 2>&1)" || status=$?
    [[ "$status" != 0 && ! -s "$TRACE" ]]
    case "$invalid" in
        wrong-host) [[ "$output" == *'Native release build required'* ]] ;;
        new-glibc|old-glibc) [[ "$output" == *'Release build requires GLIBC 2.35'* ]] ;;
        unsupported-target) [[ "$output" == *'Unsupported release target'* ]] ;;
    esac
    pass "preflight rejects $invalid before cargo starts"
done

FAKE_LIBC='glibc 2.35'
for failure in compiler glibc-gate; do
    FAKE_CARGO_FAIL=0
    FAKE_SYMBOLS=GLIBC_2.35
    if [[ "$failure" == compiler ]]; then FAKE_CARGO_FAIL=1; else FAKE_SYMBOLS=GLIBC_2.39; fi
    archive="$MOCK_REPO/release/nusa-panel-v0.1.1-$FAKE_HOST.tar.gz"
    rm -f "$archive" "$archive.sha256"
    : > "$TRACE"
    status=0
    output="$(unset CARGO_TARGET_DIR; bash "$MOCK_REPO/scripts/build-release.sh" "$FAKE_HOST" 2>&1)" || status=$?
    [[ "$status" != 0 && ! -f "$archive" && "$output" != *'[SUKSES]'* ]]
    if [[ "$failure" == compiler ]]; then
        [[ "$status" == 9 ]]
        ! grep -q '^readelf:' "$TRACE"
    else
        [[ "$output" == *'Unsupported GLIBC baseline'* ]]
    fi
    pass "$failure failure prevents packaging and success output"
done

[[ "$(cat "$MOCK_REPO/target/release/build/legacy-build-script")" == 'incompatible GLIBC_2.39 cached host build script' ]]
pass 'legacy incompatible build cache remains unused and untouched'

workflow="$REPO/.github/workflows/release.yml"
grep -Fq 'runs-on: ${{ matrix.runner }}' "$workflow"
grep -q 'runner: ubuntu-22.04$' "$workflow"
grep -q 'runner: ubuntu-22.04-arm$' "$workflow"
! grep -Eq 'cross build|cargo install cross|use_cross' "$workflow"
grep -Fq "prefix-key: release-native-glibc235-v1" "$workflow"
grep -Fq "cache-targets: 'false'" "$workflow"
grep -Fq "cache-bin: 'false'" "$workflow"
grep -Fq 'CARGO_TARGET_DIR: target/release-native/${{ matrix.target }}' "$workflow"
grep -Fq 'scripts/compile-release.sh' "$workflow"
! grep -Fq 'cp target/${{ matrix.target }}' "$workflow"
pass 'release workflow uses native runners, source-only cache and isolated artifact paths'

printf '%s release build checks passed (mock compiler/readelf, no real release build).\n' "$count"
