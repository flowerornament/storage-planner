# Extending the Storage Planner

This guide explains how to extend the Rust implementation.

## Project Structure

```
src/
├── main.rs                 # CLI entry point
├── lib.rs                  # Library exports
├── core/                   # Domain-agnostic abstractions
│   ├── db.rs               # SQLite database layer
│   ├── models.rs           # Item, Price, Configuration, Event
│   ├── events.rs           # Append-only event logging
│   └── specs.rs            # Spec parsing (capacity, speed, noise)
├── cli/                    # CLI commands
│   ├── mod.rs              # Command dispatch
│   ├── item.rs             # sp item *
│   ├── price.rs            # sp price *
│   ├── config.rs           # sp config *
│   ├── decide.rs           # sp decide *
│   └── analyze.rs          # sp analyze
├── domains/                # Domain-specific modules
│   └── storage/
│       ├── models.rs       # Node, Volume, Dataset, SyncRegime
│       └── analysis.rs     # Redundancy, capacity, RPO/RTO analysis
└── pricing/                # Price API integrations
    ├── mod.rs              # PriceFetcher trait
    ├── ebay.rs
    ├── bestbuy.rs
    └── keepa.rs
```

## Adding a New CLI Command

1. Create a new file in `src/cli/` (e.g., `src/cli/mycommand.rs`)

2. Define args and implementation:

```rust
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;

#[derive(Args)]
pub struct MyCommandArgs {
    /// Description of the argument
    #[arg(long)]
    pub some_option: Option<String>,
}

pub fn run(db_path: Utf8PathBuf, args: MyCommandArgs) -> Result<()> {
    // Implementation
    Ok(())
}
```

3. Register in `src/cli/mod.rs`:

```rust
mod mycommand;

#[derive(Subcommand)]
pub enum Commands {
    // ...existing commands...
    MyCommand(mycommand::MyCommandArgs),
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            // ...existing...
            Commands::MyCommand(args) => mycommand::run(db_path, args),
        }
    }
}
```

## Adding an Analysis Check

In `src/cli/analyze.rs`, add to the match in `run()`:

```rust
for check_name in checks_to_run {
    let check = match check_name {
        "cost" => analyze_cost(&config),
        "capacity" => analyze_capacity(&item_specs),
        "my_check" => analyze_my_check(&config, &item_specs),  // Add here
        _ => bail!("Unknown check: {}", check_name),
    };
    checks.push(check);
}
```

Implement the check function:

```rust
fn analyze_my_check(config: &Configuration, item_specs: &[(String, JsonValue)]) -> Check {
    // Your analysis logic
    let (status, message) = if some_condition {
        (CheckStatus::Pass, "All good".to_string())
    } else {
        (CheckStatus::Warn, "Issue found".to_string())
    };

    Check {
        name: "my_check".to_string(),
        status,
        details: serde_json::json!({
            "message": message,
            // Additional details
        }),
    }
}
```

## Adding a New Domain

1. Create a new domain module: `src/domains/computing/`

2. Define domain models in `models.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub id: String,
    pub component_type: ComponentType,
    // ...
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComponentType {
    Cpu,
    Gpu,
    Ram,
    Storage,
}
```

3. Add domain-specific analysis in `analysis.rs`

4. Export from `src/domains/mod.rs`:

```rust
pub mod computing;
pub mod storage;
```

## Adding a Price API Integration

1. Implement the `PriceFetcher` trait in a new file:

```rust
// src/pricing/newapi.rs
use super::{PriceFetcher, PriceResult};
use crate::core::models::PriceSource;
use anyhow::Result;

pub struct NewApiFetcher {
    api_key: Option<String>,
}

impl NewApiFetcher {
    pub fn new() -> Self {
        Self {
            api_key: std::env::var("SP_NEWAPI_KEY").ok(),
        }
    }
}

impl PriceFetcher for NewApiFetcher {
    fn fetch(&self, query: &str) -> Result<Vec<PriceResult>> {
        if !self.is_available() {
            anyhow::bail!("NewAPI key not configured");
        }

        // Make HTTP request using ureq
        let resp: serde_json::Value = ureq::get("https://api.example.com/search")
            .query("q", query)
            .query("key", self.api_key.as_ref().unwrap())
            .call()?
            .into_json()?;

        // Parse response into PriceResult
        Ok(vec![])
    }

    fn is_available(&self) -> bool {
        self.api_key.is_some()
    }

    fn source(&self) -> PriceSource {
        PriceSource::Manual  // Add new variant to PriceSource enum
    }
}
```

2. Add to `PriceSource` enum in `src/core/models.rs`:

```rust
pub enum PriceSource {
    Ebay,
    BestBuy,
    Keepa,
    Amazon,
    NewApi,  // Add here
    Manual,
}
```

3. Wire up in `src/cli/price.rs` fetch command

## Database Migrations

Schema is defined in `src/core/db.rs` in the `SCHEMA` constant. For migrations:

1. Add new tables/columns to the schema
2. Consider backwards compatibility
3. For production use, implement proper migration versioning

## Testing

Add tests in the same file using `#[cfg(test)]`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_function() {
        // Use Database::open_memory() for test databases
        let mut db = Database::open_memory().unwrap();
        db.migrate().unwrap();

        // Test logic
    }
}
```

Run tests:
```bash
cargo test
cargo test -- --nocapture  # Show println output
```

## Build & Release

```bash
cargo build                 # Debug build
cargo build --release       # Release build (optimized)
cargo install --path .      # Install to ~/.cargo/bin/
```
