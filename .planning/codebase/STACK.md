# Technology Stack

**Analysis Date:** 2026-01-31

## Languages

**Primary:**
- Rust 2021 edition - CLI application and core library

## Runtime

**Environment:**
- Rust stable (specified in `rust-toolchain.toml`)

**Package Manager:**
- Cargo (Rust's package manager)
- Lockfile: `Cargo.lock` present

## Frameworks

**Core CLI:**
- `clap` 4.x - Command-line argument parsing with derive macros, environment variable support

**Database:**
- `rusqlite` 0.31 - SQLite driver with bundled SQLite and JSON support
  - Features: `bundled`, `serde_json`
  - Provides synchronous database access, transactions, migrations

**Testing:**
- Built-in Rust test framework (`#[test]` macros)
- In-memory database support via `rusqlite` for unit tests

**Build/Dev:**
- `just` - Command runner (justfile at repository root)
- `cargo clippy` - Linter
- `cargo fmt` - Code formatter

## Key Dependencies

**Critical:**
- `rusqlite` 0.31 - SQLite database engine, core to all data persistence
- `serde` 1.x + `serde_json` 1.x - Serialization framework for JSON specs, metadata, and API responses
- `clap` 4.x - CLI argument parsing and subcommand routing

**Infrastructure:**
- `anyhow` 1.x - Error handling and context propagation
- `camino` 1.x - UTF-8 safe path handling with serde support
- `uuid` 1.x - ID generation (v4 random UUIDs with serde serialization)
- `chrono` 0.4 - Datetime handling with serde support
- `console` 0.15 - Terminal output formatting and styling
- `fs-err` 3.x - Filesystem operations with better error messages
- `xshell` 0.2 - Shell command execution (used in CLI commands)
- `serde_yaml` 0.9 - YAML export/parsing for decision snapshots

**HTTP Clients:**
- `ureq` 2.x with `json` feature - Synchronous HTTP client for API integrations
  - Used by: `src/pricing/bestbuy.rs`, `src/pricing/ebay.rs`
  - No async runtime - blocking HTTP calls

## Configuration

**Environment Variables:**
- `SP_DIR` - Database directory location (default: `.sp` in current directory)
- `SP_BESTBUY_API_KEY` - Best Buy API authentication key
- `SP_EBAY_APP_ID` - eBay OAuth2 app ID
- `SP_EBAY_CERT_ID` - eBay OAuth2 certificate ID

**Database Configuration:**
- Default location: `.sp/decisions.db`
- Overridable via `--dir` CLI flag or `SP_DIR` environment variable
- SQLite pragmas configured in `src/core/db.rs`:
  - `PRAGMA foreign_keys = ON` - Enforce referential integrity
  - `PRAGMA journal_mode = WAL` - Write-Ahead Logging for better concurrency
  - `PRAGMA synchronous = NORMAL` - Balance between durability and performance

**Build Configuration:**
- Release profile in `Cargo.toml`:
  - `lto = true` - Link-time optimization
  - `strip = true` - Binary stripping for smaller size

**Linting Configuration:**
- Clippy: All warnings enabled (warn level)
- Relaxed rules: `should_implement_trait = warn` (allows gradual cleanup)

## Platform Requirements

**Development:**
- Rust toolchain with rustfmt and clippy
- SQLite C libraries (bundled via rusqlite feature)
- Standard C library (glibc or equivalent)

**Production:**
- Self-contained binary (SQLite bundled)
- No external runtime dependencies
- Unix-like OS (Darwin/Linux tested, Windows likely supported)
- Filesystem access to `.sp` directory for database

## Build Commands

Available via `justfile` at project root:

```bash
just fmt              # Format code with rustfmt
just lint             # Run clippy (warnings only)
just lint-strict      # Run clippy (warnings as errors)
just test             # Run all tests
just check            # Run fmt --check + lint + test (pre-commit)
cargo build --release # Build optimized binary
```

## Binary Output

- Binary name: `sp` (specified in `Cargo.toml` [[bin]] section)
- Output location: `target/release/sp` after `cargo build --release`

---

*Stack analysis: 2026-01-31*
