# Temporary ZIP security backport (P10-T6an)

This directory contains the published `utoipa-swagger-ui` 9.0.2 source under its
original MIT OR Apache-2.0 licenses. It is a third-party dependency, excluded
from the application workspace. The upstream library source in `src/` is unchanged.

Source: https://crates.io/crates/utoipa-swagger-ui/9.0.2

Original crate archive SHA-256:
`d047458f1b5b65237c2f6dc6db136945667f40a7668627b3490b9513a3d43a55`

The root `[patch.crates-io]` uses this copy because upstream 9.0.2 requires ZIP
3.x, affected by RUSTSEC-2026-0367, 0368 and 0369. Published Swagger UI 10.0.1
still requires affected ZIP 8.x and also requires utoipa 6; upgrading Swagger
alone would neither fix the audit nor preserve the current utoipa 5 API.

Local changes:

- Raise the ZIP build dependency to 9.0.4 and the dependency MSRV to Rust 1.88.
- Move the distribution extractor from `build.rs` to `extract.rs` so the actual
  build implementation can be exercised by the panel-fs regression suite.
- Keep enclosed-path validation, reject symlink assets, and mask extracted Unix
  permissions with `0o777`, dropping SUID, SGID and sticky bits. Upgrading ZIP
  alone would not fix the build script's own permission restoration.
- Omit registry cache markers, the package lockfile and the unused original
  pre-normalization manifest. Preserve both licenses and upstream documentation.

Validation:

```text
cargo test -p panel-fs --tests --locked
cargo test -p panel-api --test swagger_assets_test --locked
cargo deny --all-features check --show-stats
```

The regression tests import `extract.rs` directly; they cover normal assets,
traversal, absolute/relative symlink targets, and (on Unix) hostile raw permission
metadata. The API smoke test makes in-process requests for embedded assets and
the panel OpenAPI schema; it does not start a daemon.

Remove this patch when a published upstream version supports ZIP >=9.0.0 and is
compatible with the workspace's OpenAPI integration. Re-run the audit, archive
tests and Swagger asset test before removal. Do not suppress these advisories.
