# Run all standard quality gates; mutation scans remain opt-in.
check:
    cargo machete
    cargo fmt --all -- --check
    cargo clippy --all-targets --locked -- -D warnings
    python3 scripts/test_pre_commit.py
    python3 scripts/test_mutation_runner.py
    cargo test --all-targets --locked
