# OpenShell Hyperlight Adapter

`openshell-hyperlight` is a Linux/KVM command-line adapter that executes one
JSON-described Python workload in
[hyperlight-sandbox](https://github.com/hyperlight-dev/hyperlight-sandbox) and
returns one structured JSON response.

This is an initial integration layer, not an OpenShell `IsolationBackend`,
compute driver, supervisor middleware, or policy engine. OpenShell does not
currently define a local tool-plugin API. An agent or tool runner invokes this
binary as a subprocess, while OpenShell governs the binary through its normal
filesystem, process, and network policy.

## Security defaults

- No network access unless exact HTTP origins and methods are listed.
- No filesystem access unless host directories are mapped explicitly.
- Input is read-only at `/input`; output is limited and writable at `/output`.
- No host tools are registered in schema version 1.
- Every request specifies a wall-clock deadline, output capture limits, heap
  size, and stack size.
- A dedicated worker process owns the Hyperlight VM. The parent terminates that
  process when the deadline expires.
- Unknown JSON fields and unsupported capabilities are rejected.

See [the threat model](docs/threat-model.md) before deploying this adapter.

## Prerequisites

- x86-64 Linux with KVM available as `/dev/kvm`
- the pinned `nightly-2026-10-06` Rust toolchain
- Access to a Hyperlight Python AOT guest matching the pinned
  `hyperlight-sandbox` revision

The Rust dependencies are locked in `Cargo.lock`. The Hyperlight Sandbox crates
are vendored from commit
[`e38f49d149111f66ee2265c6d6c216fab62d018c`](https://github.com/hyperlight-dev/hyperlight-sandbox/commit/e38f49d149111f66ee2265c6d6c216fab62d018c)
because the production `0.7.0` workspace crates are not the unrelated
`hyperlight-sandbox` 0.0.1 crate on crates.io. The vendored manifest updates
`wasmtime-wasi-http` to the fixed 48.0.4 line; see
[`UPSTREAM.md`](vendor/hyperlight-sandbox-src/UPSTREAM.md).

The matching Hyperlight Sandbox WIT interface and encoded world are included in
the vendored source. Cargo uses the encoded world through `.cargo/config.toml`;
this is required to build a runtime compatible with the Python guest ABI.

The nightly toolchain is required because patched Wasmtime requires Rust 1.95,
while stable Rust 1.95 made Hyperlight's custom guest target unstable. The
toolchain is date-pinned in `rust-toolchain.toml`.

Install the packaged Python guest and print its path:

```bash
python3 -m venv .venv
.venv/bin/pip install 'hyperlight-sandbox-python-guest==0.7.0'
PYTHON_GUEST="$(
  .venv/bin/python -c 'from python_guest import path; print(path)'
)"
test -r "$PYTHON_GUEST"
```

Build the adapter:

```bash
cargo build --locked --release --features hyperlight
```

## JSON CLI contract

The public command reads exactly one JSON object from standard input and writes
exactly one JSON object to standard output:

```bash
target/release/openshell-hyperlight execute \
  --python-guest "$PYTHON_GUEST" \
  < examples/request.json
```

Example success:

```json
{
  "schema_version": 1,
  "status": "ok",
  "execution": {
    "stdout": "30\n",
    "stdout_truncated": false,
    "stderr": "",
    "stderr_truncated": false,
    "exit_code": 0,
    "output_files": []
  }
}
```

Adapter failures use `status: "error"` and a stable `error.code`. A guest
non-zero exit uses `status: "guest_error"` and preserves guest output. A
deadline uses `status: "timeout"`. Process exit is zero when a structured
execution response was produced and non-zero when the adapter itself failed.

The trusted operator supplies `--python-guest`; untrusted request JSON cannot
select guest code. Full field semantics are documented in
[the protocol reference](docs/protocol.md).

## OpenShell integration

The smallest supported integration is to place the adapter, its trusted Python
guest, and an agent-specific wrapper/tool declaration in the sandbox workload
image. The wrapper sends JSON on stdin and parses JSON on stdout. Tool
registration belongs to the agent or tool runner; this project does not claim
that OpenShell registers the command.

An OpenShell policy must make the binary and guest artifact readable. It must
also expose any request input/output directories. Start from
[the example policy fragment](examples/openshell-policy.yaml) and replace all
paths with the exact paths in the workload image.

If a request grants network origins, OpenShell must independently allow those
same destinations for the adapter binary through `network_policies`. Hyperlight
enforces the request allowlist inside the VM; OpenShell enforces its own policy
outside it. This adapter does not translate, fetch, or weaken OpenShell policy.

The selected OpenShell compute runtime must expose `/dev/kvm` to the workload.
OpenShell policy YAML does not itself grant KVM device access, and this
repository does not configure a compute driver to do so. If `/dev/kvm` is not
available, execution fails rather than falling back to a weaker backend.

OpenShell primary-source references used for this design:

- [extension points](https://github.com/NVIDIA/OpenShell/blob/4a0208fa695d8881f4210e8a89d94af6d895c267/docs/extensibility/overview.mdx)
- [policy schema](https://github.com/NVIDIA/OpenShell/blob/4a0208fa695d8881f4210e8a89d94af6d895c267/docs/how-it-works/policies/schema.mdx)
- [network policy behavior](https://github.com/NVIDIA/OpenShell/blob/4a0208fa695d8881f4210e8a89d94af6d895c267/docs/how-it-works/policies/network-rules.mdx)

## Development

```bash
just fmt-apply
just build
just clippy
just test
just vendor-test
```

The real Hyperlight integration test is separate:

```bash
HYPERLIGHT_PYTHON_GUEST="$PYTHON_GUEST" just kvm-test
```

`just test` never substitutes another Python runtime. `just kvm-test` requires
Linux, `/dev/kvm`, the AOT guest, and the `hyperlight` feature. CI keeps this
test in a dedicated KVM job.

## Status and scope

Schema version 1 intentionally supports one-shot Python execution only. There
is no daemon, MCP server, host-tool dispatch, persistent VM pool, policy
translation, credential injection, or compatibility promise beyond the pinned
upstream revision. See [architecture](docs/architecture.md) and
[limitations](docs/threat-model.md#non-goals-and-limitations).

Licensed under Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
