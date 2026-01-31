//! sp sync - Export database to YAML (read-only snapshot)
//!
//! Creates YAML files in export/ directory for human readability.
//! These files are read-only - the database is the source of truth.

use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use clap::Args;
use console::style;
use serde::Serialize;
use std::collections::HashMap;

use crate::core::db::Database;
use crate::core::models::{Configuration, Decision, Item, Price};

#[derive(Args)]
pub struct SyncArgs {
    /// Output directory (default: export/)
    #[arg(long, default_value = "export")]
    pub output: Utf8PathBuf,

    /// Only export catalog (skip configurations/decisions)
    #[arg(long)]
    pub catalog_only: bool,
}

pub fn run(db_path: Utf8PathBuf, args: SyncArgs) -> Result<()> {
    if !db_path.exists() {
        bail!("Database not found at {}. Run `sp init` first.", db_path);
    }

    let db = Database::open(&db_path)?;

    // Create output directory
    std::fs::create_dir_all(&args.output)?;

    // Export items by category
    export_catalog(&db, &args.output)?;

    if !args.catalog_only {
        // Export current configuration
        export_current(&db, &args.output)?;

        // Export decision history
        export_decisions(&db, &args.output)?;
    }

    println!("{} Exported to {}/", style("✓").green(), args.output);

    Ok(())
}

fn export_catalog(db: &Database, output: &Utf8PathBuf) -> Result<()> {
    let catalog_dir = output.join("catalog");
    std::fs::create_dir_all(&catalog_dir)?;

    // Get all items grouped by category
    let mut stmt = db.conn().prepare(
        "SELECT id, name, category, brand, specs, tags, metadata, archived, created_at, updated_at
         FROM items WHERE archived = 0 ORDER BY category, name",
    )?;

    let items: Vec<Item> = stmt
        .query_map([], Item::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    // Group by category
    let mut by_category: HashMap<String, Vec<&Item>> = HashMap::new();
    for item in &items {
        by_category
            .entry(item.category.clone())
            .or_default()
            .push(item);
    }

    // Write each category to a file
    for (category, items) in by_category {
        let path = catalog_dir.join(format!("{}.yaml", category));
        let content = serde_yaml::to_string(&items)?;
        std::fs::write(
            &path,
            format!(
                "# {} catalog (auto-generated, read-only)\n# Source: sp sync\n\n{}",
                category, content
            ),
        )?;
    }

    // Also get latest prices for each item
    let mut price_stmt = db.conn().prepare(
        "SELECT p.id, p.item_id, p.source, p.price, p.currency, p.condition, p.url, p.observed_at, p.metadata
         FROM prices p
         INNER JOIN (
             SELECT item_id, MAX(observed_at) as max_date
             FROM prices
             GROUP BY item_id
         ) latest ON p.item_id = latest.item_id AND p.observed_at = latest.max_date",
    )?;

    let prices: Vec<Price> = price_stmt
        .query_map([], Price::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    if !prices.is_empty() {
        let prices_path = catalog_dir.join("_prices.yaml");
        let content = serde_yaml::to_string(&prices)?;
        std::fs::write(
            &prices_path,
            format!(
                "# Latest prices (auto-generated, read-only)\n# Source: sp sync\n\n{}",
                content
            ),
        )?;
    }

    Ok(())
}

fn export_current(db: &Database, output: &Utf8PathBuf) -> Result<()> {
    // Get current configuration
    let current: Option<Configuration> = db
        .conn()
        .query_row(
            "SELECT id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at
             FROM configurations WHERE is_current = 1 LIMIT 1",
            [],
            Configuration::from_row,
        )
        .ok();

    if let Some(config) = current {
        let path = output.join("current.yaml");
        let content = serde_yaml::to_string(&config)?;
        std::fs::write(&path, format!("# Current deployed configuration (auto-generated, read-only)\n# Source: sp sync\n\n{}", content))?;
    }

    Ok(())
}

#[derive(Serialize)]
struct DecisionExport {
    #[serde(flatten)]
    decision: Decision,
    // Could include resolved option details here
}

fn export_decisions(db: &Database, output: &Utf8PathBuf) -> Result<()> {
    let history_dir = output.join("history");
    std::fs::create_dir_all(&history_dir)?;

    // Get all decisions
    let mut stmt = db.conn().prepare(
        "SELECT id, purpose, status, options, chosen_option, chosen_config_id, rationale, decided_at, decided_by, created_at, metadata
         FROM decisions ORDER BY created_at DESC",
    )?;

    let decisions: Vec<Decision> = stmt
        .query_map([], Decision::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    for decision in decisions {
        let date = decision.created_at.format("%Y-%m-%d");
        let path = history_dir.join(format!("{}-{}.yaml", date, &decision.id[..8]));
        let content = serde_yaml::to_string(&decision)?;
        std::fs::write(
            &path,
            format!(
                "# Decision session (auto-generated, read-only)\n# Source: sp sync\n\n{}",
                content
            ),
        )?;
    }

    Ok(())
}
