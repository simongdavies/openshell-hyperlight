# Architecture

## Components

```mermaid
flowchart LR
    A[Agent or tool runner] -->|JSON stdin| P[Adapter parent]
    P -->|validated JSON| W[Dedicated worker process]
    W --> H[hyperlight-sandbox]
    H --> V[Wasm CPython guest in Hyperlight micro-VM]
    V -->|optional explicit capabilities| F[/input and /output]
    V -->|optional exact origins and methods| N[HTTP destinations]
    P -->|deadline: terminate worker| W
    P -->|JSON stdout| A
```

The parent process parses and validates the request, starts a worker, writes the
validated request to it, and enforces the wall-clock deadline. The worker is
the only process that creates a Hyperlight sandbox. Killing the worker tears
down the VM instead of leaving an uninterruptible in-process execution thread.

The worker builds `hyperlight_sandbox::SandboxBuilder<Wasm>` with a trusted
Python AOT guest selected by the operator. No `ToolRegistry` entries are
registered. Filesystem mappings, filesystem quotas, memory sizes, and network
permissions come only from the validated request.

## Trust boundaries

Trusted:

- adapter binary and configuration;
- `--python-guest` AOT module;
- OpenShell operator policy and compute configuration;
- host kernel, KVM, Hyperlight, and pinned transitive dependencies.

Untrusted:

- Python source;
- request-selected input data;
- guest stdout, stderr, exit status, and output files;
- network responses from explicitly allowed origins.

## OpenShell boundary

OpenShell remains responsible for launching and confining the adapter process.
The adapter does not implement any OpenShell extension protocol. It does not
receive OpenShell policy documents and does not assert that its capability
model is equivalent to OpenShell's.

Network access therefore requires two independent grants:

1. the request's exact Hyperlight origin/method capability; and
2. an OpenShell `network_policies` rule for the adapter binary.

Filesystem access similarly requires both an adapter mapping and access under
the OpenShell/Landlock policy.

## Dependency boundary

The Hyperlight Sandbox crates are vendored from one commit because the current
production workspace is not available under the corresponding crates.io
package/version. Its manifest is patched to use a non-vulnerable
`wasmtime-wasi-http` line; the exact change is recorded in the vendor directory.
The matching WIT source and encoded WIT world are vendored because
`hyperlight-wasm` consumes `WIT_WORLD` while compiling its guest runtime.
`Cargo.lock` pins all transitive dependencies. Any upstream revision change
requires regenerating those artifacts, API review, license review, audit, and a
real KVM test.
