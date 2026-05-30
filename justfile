set shell := ["bash", "-euo", "pipefail", "-c"]
set positional-arguments := false

[private]
default:
    @just --list

# Format code
[group('check')]
fmt:
    cargo fmt

# Run clippy (warnings shown but not fatal)
[group('check')]
lint:
    cargo clippy --all-targets

# Run clippy strict (warnings as errors - for CI/pre-commit)
[group('check')]
lint-strict:
    cargo clippy --all-targets -- -D warnings

# Run tests
[group('check')]
test:
    cargo test

# Run all checks (fmt + lint + test)
[group('check')]
check:
    cargo fmt --check
    cargo clippy --all-targets
    cargo test

# Release build
[group('build')]
build:
    cargo build --release

# Update release versions and scaffold changelog.
[group('release')]
[arg('version', pattern='[0-9]+\.[0-9]+\.[0-9]+', help='Semver release, e.g. 0.2.1')]
release-bump version:
    python3 scripts/release.py bump {{quote(version)}}

# Release-readiness checks and validation.
[group('release')]
release-verify:
    python3 scripts/release.py verify

# Create and push an annotated release tag, then publish origin/release.
[group('release')]
[arg('version', pattern='[0-9]+\.[0-9]+\.[0-9]+', help='Semver release, e.g. 0.2.1')]
[confirm("This will tag and force-update origin/release. Continue?")]
release-tag version:
    python3 scripts/release.py tag {{quote(version)}}
