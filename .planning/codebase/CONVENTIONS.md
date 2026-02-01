# Coding Conventions

**Analysis Date:** 2026-01-31

## Naming Patterns

**Files:**
- Module files: lowercase with underscores (e.g., `url_parser.rs`, `ebay.rs`)
- Main entry: `main.rs`, library entry: `lib.rs`, module roots: `mod.rs`
- Subcommands grouped by domain (e.g., `src/cli/item.rs`, `src/pricing/ebay.rs`)

**Functions:**
- Snake case for all functions: `fn parse_capacity()`, `fn fetch_new_token()`, `fn analyze_redundancy()`
- Internal helper functions with leading underscore when needed (e.g., `fn _parse_duration()`)
- Test functions: `fn test_<feature>()` pattern

**Variables:**
- Snake case: `item_id`, `token_cache_dir`, `current_actor`, `single_points_of_failure`
- Constants in uppercase: `TOKEN_URL`, `BROWSE_API_URL`, `const SCHEMA`

**Types:**
- PascalCase for structs, enums, traits: `Item`, `Price`, `PriceSource`, `EbayFetcher`, `ParsedUrl`
- Enum variants in PascalCase: `ItemCondition::New`, `EventType::Created`, `Retailer::Amazon`

**Trait Implementations:**
- Trait methods use `impl Type` pattern, with explicit documentation for derived traits
- Builder pattern for fluent config: `fetcher.with_token_cache_dir(path).method_chain()`

## Code Style

**Formatting:**
- `cargo fmt` (Rustfmt) - auto-formatted on save
- Line length: follows Rustfmt defaults (typically 100 characters)
- Indentation: 4 spaces (standard Rust)

**Linting:**
- Tool: `cargo clippy` with `--all-targets`
- All clippy warnings enabled via `[lints.clippy] all = "warn"`
- Exception: `should_implement_trait = "warn"` for gradual cleanup of old code with missing trait impls
- Command: `just lint` (non-strict, warnings allowed) or `just lint-strict` (warnings as errors for CI)

## Import Organization

**Order:**
1. Standard library imports (`use std::...`)
2. External crate imports (alphabetical: `anyhow`, `camino`, `chrono`, `clap`, etc.)
3. Internal crate imports (`use crate::...`)
4. Type aliases and re-exports at module level

**Path Aliases:**
- Camino paths: `use camino::{Utf8Path, Utf8PathBuf}`
- JSON values: `use serde_json::Value as JsonValue`
- No glob imports (`use module::*`) except in tests

**Module Structure:**
Each module starts with `//!` documentation explaining purpose:
```rust
//! sp item - Manage items in the catalog
//!
//! Detailed explanation of module responsibility.
```

## Error Handling

**Strategy:**
- Use `anyhow::Result<T>` for most functions (easier error propagation)
- Use `rusqlite::Result<T>` for low-level database operations
- Database model methods (`insert`, `from_row`) return the specific error type of the driver

**Patterns:**
- `bail!("error message")` for explicit errors
- `with_context(|| "context info")` for wrapping errors with context
- `?` operator for propagation (avoid `.unwrap()` in production code)
- Graceful fallbacks: `.unwrap_or()`, `.unwrap_or_default()` where appropriate for non-critical parsing

**Error Examples:**
```rust
// Good: Context-aware error
Connection::open(path).with_context(|| format!("Failed to open database: {path}"))?

// Good: Structured bail
bail!("Could not extract ASIN from Amazon URL: {}", url)

// Good: Fallback on optional parsing
serde_json::from_str(&specs_str).unwrap_or_default()
```

## Logging

**Framework:** Console printing via `console` crate

**Patterns:**
- `console::style()` for colored/formatted terminal output
- No structured logging framework; simple `println!` for diagnostics
- Agent mode outputs JSON for machine consumption
- Status output uses styled text for readability

**Examples:**
```rust
use console::style;
println!("{}", style("Status: OK").green());
println!("{}", style("Warning").yellow());
```

## Comments

**When to Comment:**
- Function documentation: `///` above functions explaining purpose, parameters, returns
- Module documentation: `//!` at module top explaining scope and responsibility
- Complex logic: explain WHY, not WHAT (code shows WHAT)
- Trade-offs: document decisions where non-obvious

**Documentation Comments:**
- Use `///` for public APIs
- Include examples in doc comments for complex modules
- Include `#[example]` blocks where helpful

**Inline Comments:**
- Sparse usage; prefer self-documenting code
- Explain non-obvious business logic
- Mark TODOs/FIXMEs with `// TODO:` or `// FIXME:`

## Function Design

**Size:**
- Prefer functions under 50 lines
- Extract complex operations into helper functions
- Database operations often longer due to SQL/parameter binding

**Parameters:**
- Use builder/struct arguments for 3+ parameters
- Accept trait objects for flexibility: `impl Into<String>`, `&dyn Trait`
- Named arguments via struct fields (clap `#[derive(Args)]` pattern)

**Return Values:**
- Functions that mutate data or call APIs return `Result<T>`
- Pure analysis functions return the computed value directly
- Option<T> for values that are legitimately optional

**Examples:**
```rust
// Simple constructor
pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self

// Database operation with context
pub fn transaction<T, F>(&mut self, f: F) -> Result<T>
where
    F: FnOnce(&Transaction) -> Result<T>

// Pure analysis function
pub fn analyze_redundancy(...) -> RedundancyReport
```

## Module Design

**Exports:**
- `pub` only for public API
- Keep internal helpers private
- Re-export important types in `mod.rs` via `pub use`
- `pub use` centralizes what clients import

**Library Exports:**
In `src/lib.rs`:
```rust
pub mod core;
pub mod domains;
pub mod pricing;

pub use core::db::Database;
pub use core::models::{Configuration, Event, Item, Price};
```

**Barrel Files:**
- `mod.rs` files declare submodules and re-export key types
- Simplifies client imports: `use storage_planner::Database` instead of `::core::db::Database`

## Struct Design

**Serialization:**
- `#[derive(Debug, Clone, Serialize, Deserialize)]` for data types
- Serde attributes: `#[serde(rename_all = "lowercase")]` for enum variants
- JSON storage: prefer `serde_json::Value` for flexible specs and metadata
- Use `.unwrap_or_default()` for failed JSON parsing (graceful degradation)

**Timestamps:**
- Always use `DateTime<Utc>` from chrono
- Serialize via RFC3339: `.to_rfc3339()` and `parse_from_rfc3339()`
- Model methods handle conversion

**Generic Values:**
- Use `impl Into<String>` for String parameters
- `JsonValue` (type alias for `serde_json::Value`) for flexible nested data
- Allow optional fields to be `None` rather than default values

---

*Convention analysis: 2026-01-31*
