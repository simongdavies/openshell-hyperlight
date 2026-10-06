# Contributing

Use the toolchain pinned in `rust-toolchain.toml` and develop on Linux with KVM.
Before submitting a change, run:

```bash
just fmt-check
just build
just clippy
just clippyw
just test
just docs
```

Run `just kvm-test` with the packaged Python AOT guest whenever execution,
capabilities, Hyperlight dependencies, or the protocol changes. A passing unit
suite is not evidence that the KVM path works.

Keep the public JSON contract default-deny and backward compatible within a
schema version. Add a new schema version rather than silently changing field
semantics. Do not add a weaker execution fallback.

All contributions must comply with [LICENSE](LICENSE). Please use a
Developer Certificate of Origin `Signed-off-by` trailer.
