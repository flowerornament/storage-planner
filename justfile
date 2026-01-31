# Format code
fmt:
    cargo fmt

# Run clippy (warnings shown but not fatal)
lint:
    cargo clippy --all-targets

# Run clippy strict (warnings as errors - for CI/pre-commit)
lint-strict:
    cargo clippy --all-targets -- -D warnings

# Run tests
test:
    cargo test

# Run all checks (fmt + lint + test)
check:
    cargo fmt --check
    cargo clippy --all-targets
    cargo test
