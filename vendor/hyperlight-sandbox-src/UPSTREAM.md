# Upstream source and local change

This directory contains the minimal Rust source needed from
`hyperlight-dev/hyperlight-sandbox` commit
`e38f49d149111f66ee2265c6d6c216fab62d018c` (workspace version 0.7.0).

Local changes:

- the workspace member list is reduced to the two crates used by this project;
- `wasmtime-wasi-http` is updated from `44.0.2` to `48.0.4` because the
  resolved 44.0.3 line has active RustSec advisories, including critical
  `RUSTSEC-2026-0327`.

`src/hyperlight_sandbox/src/http.rs` is ported to Wasmtime's current
`default_send_request` and `RequestOptions` API. The patch is covered by the
adapter's build, Clippy, vendored library unit tests, and Linux/KVM integration
tests.

The vendored source remains licensed under Apache-2.0. See `LICENSE`.
