# Testing Patterns

**Analysis Date:** 2026-01-31

## Test Framework

**Runner:**
- Rust built-in test framework (no external test runner needed)
- Config: Configured in `Cargo.toml` under `[dev-dependencies]`
- No custom test harness

**Assertion Library:**
- Built-in `assert!`, `assert_eq!`, `assert_ne!` macros
- No external assertion library (e.g., no `assert2`, `insta`)

**Run Commands:**
```bash
just test              # Run all tests
cargo test             # Direct cargo command
cargo test --lib      # Library tests only
cargo test -- --nocapture  # Show println! output
```

**Code Coverage:**
- No coverage tooling configured
- Target: Implicit (>80% for critical modules)

## Test File Organization

**Location:**
- Co-located with source code (Rust convention)
- Tests live in `#[cfg(test)] mod tests { }` blocks at end of each file
- No separate `/tests/` directory for integration tests (not used in this codebase)

**Naming:**
- Test function: `#[test] fn test_<feature_name>()`
- Modules with tests: append `mod tests` to each file that has tests

**Structure:**
```
src/
├── core/
│   ├── db.rs
│   │   └── tests (4 tests)
│   ├── models.rs
│   │   └── tests (5 tests)
│   ├── specs.rs
│   │   └── tests (6 tests)
│   └── events.rs
│       └── tests (3 tests)
├── pricing/
│   ├── url_parser.rs
│   │   └── tests (9 tests)
│   ├── ebay.rs
│   │   └── tests (5 tests)
│   ├── bestbuy.rs
│   │   └── tests (4 tests)
│   ├── fallback.rs
│   │   └── tests (3 tests)
│   └── product.rs
│       └── tests (4 tests)
└── domains/storage/
    └── analysis.rs
        └── tests (2 tests)
```

**Total Tests:** ~45 tests across the codebase

## Test Structure

**Setup Pattern:**
Database tests use in-memory SQLite for isolation:
```rust
#[test]
fn test_migrate() {
    let mut db = Database::open_memory().unwrap();
    db.migrate().unwrap();
    assert!(db.is_initialized().unwrap());
}
```

**Data Construction:**
Direct struct construction with `.new()` builders:
```rust
#[test]
fn test_product_info_suggested_id() {
    let mut product = ProductInfo::new("870 EVO 4TB");
    product.brand = Some("Samsung".to_string());
    assert_eq!(product.suggested_item_id(), "samsung-870-evo-4tb");
}
```

**Teardown Pattern:**
Implicit cleanup via `Database::open_memory()` scope (Rust RAII)
- No explicit teardown needed for in-memory databases
- Temporary files use `tempfile` crate (dev-dependency)

## Mocking

**Framework:**
- No mocking framework (mockito, mock not in dependencies)
- Manual test doubles or in-memory implementations

**Patterns:**
Use in-memory database for testing database-dependent code:
```rust
#[test]
fn test_transaction() {
    let mut db = Database::open_memory().unwrap();
    db.migrate().unwrap();

    let result = db.transaction(|tx| {
        tx.execute(
            "INSERT INTO items (id, name, category, specs) VALUES (?1, ?2, ?3, ?4)",
            ["test-1", "Test Item", "ssd", "{}"],
        )?;
        Ok(42)
    });

    assert_eq!(result.unwrap(), 42);
}
```

API testing uses direct struct construction without HTTP mocking:
```rust
#[test]
fn test_ebay_parse_condition() {
    let fetcher = EbayFetcher::new();
    let cond = fetcher.parse_condition(&Some("NEW".to_string()));
    assert_eq!(cond, ItemCondition::New);
}
```

**What to Mock:**
- External API calls: Test parsing/logic, not actual HTTP requests
- Database operations: Use `Database::open_memory()`
- File I/O: Use `tempfile` crate for temporary files

**What NOT to Mock:**
- Core parsing logic (test with real data)
- Enum conversions (test mapping logic directly)
- Error handling (use `.unwrap()` in tests, errors indicate failures)

## Fixtures and Factories

**Test Data:**
Inline construction using struct literals and builders:
```rust
#[test]
fn test_identifiers_from_json() {
    let json = serde_json::json!({
        "asin": "B089C5P5SX",
        "bestbuy_sku": "6405087"
    });

    let ids = Identifiers::from_json(&json);
    assert_eq!(ids.asin, Some("B089C5P5SX".to_string()));
    assert_eq!(ids.bestbuy_sku, Some("6405087".to_string()));
    assert!(ids.upc.is_none());
}
```

Capacity/Speed parsing test data:
```rust
#[test]
fn test_capacity_parse() {
    assert_eq!(Capacity::parse("4TB").unwrap().bytes, 4 * Capacity::TB);
    assert_eq!(Capacity::parse("500GB").unwrap().bytes, 500 * Capacity::GB);
    assert_eq!(Capacity::parse("4 TB").unwrap().bytes, 4 * Capacity::TB);
    assert_eq!(Capacity::parse("4tb").unwrap().bytes, 4 * Capacity::TB);  // Case-insensitive
}
```

**Location:**
- Test data defined inline in `#[test]` functions
- No separate fixture files or factory modules
- Use `serde_json::json!()` macro for JSON test data

## Coverage

**Requirements:**
- No explicit enforcement via CI
- Implicit expectation: Core modules (models, parsing, analysis) have >80% coverage

**View Coverage:**
Coverage tools not configured. Use:
```bash
cargo tarpaulin --out Html  # Via external tool (not in project)
# or
cargo llvm-cov             # Via llvm-cov plugin
```

**Focus Areas (implicitly high coverage):**
- `src/core/models.rs` - Data serialization/deserialization (5 tests)
- `src/core/specs.rs` - Unit parsing (Capacity, Speed, Noise) (6 tests)
- `src/pricing/url_parser.rs` - Retailer URL extraction (9 tests)
- `src/core/db.rs` - Database operations (4 tests)

## Test Types

**Unit Tests:**
- Scope: Individual functions and pure logic
- Approach: Test parsing, conversion, analysis functions in isolation
- Example: `test_capacity_parse()` in `src/core/specs.rs`

**Integration Tests:**
- Scope: Database transactions, CLI command workflows
- Approach: Use `Database::open_memory()` to test full data flow
- Example: `test_transaction()` in `src/core/db.rs`

**E2E Tests:**
- Framework: Not used
- Rationale: CLI is self-testing via command validation; YAML export provides snapshot testing

**Domain-Specific Tests:**
- Storage analysis: `src/domains/storage/analysis.rs` (2 tests)
- Tests for redundancy calculation and capacity reporting

## Common Patterns

**Assertion Style:**
Use direct assertions for clarity:
```rust
assert_eq!(value, expected);
assert!(condition);
assert!(option.is_none());
assert!(option.is_some());
```

**Error Testing:**
Test error conditions by checking `Result::is_err()` or error content:
```rust
#[test]
fn test_invalid_url() {
    let result = parse_url("https://invalid.com/page");
    assert!(result.is_err());
}
```

**Parsing Tests:**
Verify parsing with known inputs and outputs:
```rust
#[test]
fn test_speed_parse() {
    assert_eq!(
        Speed::parse("560MB/s").unwrap().bytes_per_sec,
        560 * Capacity::MB
    );
}
```

**Database Tests:**
Initialize, execute operation, assert state:
```rust
#[test]
fn test_stats_empty() {
    let mut db = Database::open_memory().unwrap();
    db.migrate().unwrap();
    let stats = db.stats().unwrap();
    assert_eq!(stats.items, 0);
    assert_eq!(stats.prices, 0);
}
```

**JSON Serialization Tests:**
Verify round-trip or specific field presence:
```rust
#[test]
fn test_agent_response_serialization() {
    let response = AgentFallbackResponse::new(FallbackReason::NoApiKeys, "Samsung 870 EVO 4TB");

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("fallback_required"));
    assert!(json.contains("Samsung 870 EVO 4TB"));
}
```

## Test Execution

**All Tests:**
```bash
just check    # Runs fmt --check, clippy, and test
cargo test    # Run with output
```

**Specific Test:**
```bash
cargo test test_capacity_parse   # Run single test
cargo test specs::               # Run all tests in specs module
```

**With Output:**
```bash
cargo test -- --nocapture       # Show println! in tests
```

---

*Testing analysis: 2026-01-31*
