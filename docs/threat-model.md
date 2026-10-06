# Threat model

## Security objective

The adapter reduces the consequences of executing attacker-controlled Python by
placing the interpreter in a Hyperlight hardware-isolated micro-VM and exposing
only explicitly requested host capabilities. It must fail closed when a
capability is absent, malformed, or unsupported.

## Threats addressed

- Guest Python directly reading arbitrary host files: only an explicit input
  directory is exposed read-only.
- Guest Python directly writing arbitrary host files: Hyperlight writes only to
  an adapter-owned temporary output directory, with logical size and file-count
  limits. The adapter copies new entries into the explicit destination without
  overwriting or recursively deleting caller-selected paths.
- Unlisted network egress: Hyperlight network access starts empty and receives
  exact origin/method grants.
- Accidental host function exposure: schema version 1 registers no host tools.
- Indefinite execution: the VM lives in a dedicated worker process that the
  parent terminates at the request deadline.
- Contract confusion: unknown JSON fields, unknown enum values, duplicate
  methods, unsupported tools, and overlapping filesystem mappings are rejected.
- Silent downgrade: Linux/KVM/Hyperlight is the only execution path. There is no
  native-Python, container, process, or QEMU fallback.

## Defense in depth with OpenShell

The adapter capability model is not a replacement for OpenShell policy.
OpenShell should independently restrict the adapter binary's filesystem and
network access. A network request succeeds only if both layers allow it.

This project does not consume OpenShell policy documents. It therefore does not
translate or enforce OpenShell credential bindings, request-body rewrites,
REST/GraphQL/MCP path rules, deny rules, policy advisor approvals, binary
identity pins, process UID/GID, or Landlock configuration.

## Non-goals and limitations

- This is not an OpenShell isolation backend, compute driver, middleware,
  interceptor, gateway, or policy implementation.
- The CLI does not authenticate its caller. The enclosing tool runner and
  OpenShell sandbox are expected to control who can execute it.
- KVM and the host kernel are trusted. Hypervisor or kernel escapes, hardware
  flaws, speculative-execution attacks, side channels, denial of service
  against shared host resources, and malicious host administrators are outside
  this milestone.
- Hyperlight's filesystem limits are logical, not physical disk quotas. Place
  output on a separately quota-controlled filesystem for hostile multi-tenant
  use.
- Output capture limits are applied after Hyperlight returns strings. They limit
  the response, not peak guest/runtime memory used while producing output.
- The timeout is wall-clock process termination, not a deterministic CPU or
  instruction budget. Timed-out output remains in an adapter-owned temporary
  directory and is not copied to the requested destination.
- Domain authorization relies on hyperlight-sandbox's URL and HTTP stack. The
  adapter does not add DNS pinning, response-size limits, TLS certificate
  policy, IP allowlists, or path-level network rules.
- Input content and output files are not scanned. A caller must treat all guest
  output as untrusted.
- No secrets or OpenShell-managed credentials are injected into the guest.
- No VM pooling or snapshots are used. Each request creates a fresh worker and
  sandbox.
- The adapter has not been independently audited and makes no latency,
  throughput, or absolute security claims.

## Reporting vulnerabilities

Do not file public issues for suspected vulnerabilities. Follow
[SECURITY.md](../SECURITY.md).
