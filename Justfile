set shell := ["bash", "-uc"]

fmt-apply:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

build:
    cargo build --locked --all-targets --features hyperlight

clippy:
    cargo clippy --locked --all-targets --features hyperlight -- -D warnings

clippyw:
    cargo clippy --locked --all-targets --no-default-features --target x86_64-pc-windows-gnu -- -D warnings

test:
    cargo test --locked --all-targets --no-default-features

vendor-test:
    cargo test --locked --manifest-path vendor/hyperlight-sandbox-src/Cargo.toml -p hyperlight-sandbox --lib

guest-path-smoke python=".venv/bin/python":
    test -x "{{python}}"
    "{{python}}" scripts/verify_python_guest_path.py

kvm-test:
    test -c /dev/kvm
    test -n "${HYPERLIGHT_PYTHON_GUEST:-}"
    test -r "$HYPERLIGHT_PYTHON_GUEST"
    cargo test --locked --features hyperlight --test kvm_python -- --ignored --nocapture --test-threads=1

docs:
    RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --features hyperlight

audit:
    cargo audit

deny:
    cargo deny check
