//! sp prime - Output context for agents
//!
//! Provides everything an agent needs to understand the current state
//! in a single command. Similar to `bd prime`.

use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use chrono::Utc;
use clap::Args;
use serde::Serialize;

use crate::core::db::Database;
use crate::core::events::EventLog;
use crate::core::models::{Configuration, Decision, Event, Item};

use super::OutputFormat;

#[derive(Args)]
pub struct PrimeArgs {
    /// Include full item catalog
    #[arg(long)]
    pub full_catalog: bool,

    /// Include recent events
    #[arg(long, default_value = "10")]
    pub recent_events: usize,
}

#[derive(Serialize)]
struct PrimeOutput {
    /// When this context was generated
    timestamp: String,

    /// Database statistics
    stats: Stats,

    /// Current deployed configuration (if any)
    current: Option<Configuration>,

    /// Active decision session (if any)
    active_decision: Option<Decision>,

    /// Items with stale prices (>7 days old)
    stale_prices: Vec<String>,

    /// Recent events
    recent_events: Vec<Event>,

    /// Items in catalog (if --full-catalog)
    #[serde(skip_serializing_if = "Option::is_none")]
    catalog: Option<Vec<Item>>,
}

#[derive(Serialize)]
struct Stats {
    items: i64,
    prices: i64,
    configurations: i64,
    decisions: i64,
    events: i64,
}

pub fn run(db_path: Utf8PathBuf, args: PrimeArgs, format: OutputFormat) -> Result<()> {
    if !db_path.exists() {
        bail!("Database not found at {}. Run `sp init` first.", db_path);
    }

    let db = Database::open(&db_path)?;
    let db_stats = db.stats()?;

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

    // Get active decision
    let active_decision: Option<Decision> = db
        .conn()
        .query_row(
            "SELECT id, purpose, status, options, chosen_option, chosen_config_id, rationale, decided_at, decided_by, created_at, metadata
             FROM decisions WHERE status = 'active' ORDER BY created_at DESC LIMIT 1",
            [],
            Decision::from_row,
        )
        .ok();

    // Get items with stale prices (>7 days)
    let stale_prices: Vec<String> = {
        let mut stmt = db.conn().prepare(
            "SELECT DISTINCT i.id
             FROM items i
             LEFT JOIN prices p ON p.item_id = i.id
             WHERE i.archived = 0
             GROUP BY i.id
             HAVING MAX(p.observed_at) < datetime('now', '-7 days')
                OR MAX(p.observed_at) IS NULL",
        )?;
        let result = stmt.query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    // Get recent events
    let recent_events = EventLog::recent(db.conn(), args.recent_events)?;

    // Get catalog if requested
    let catalog = if args.full_catalog {
        let mut stmt = db.conn().prepare(
            "SELECT id, name, category, brand, specs, tags, metadata, archived, created_at, updated_at
             FROM items WHERE archived = 0 ORDER BY category, name",
        )?;
        let items: Vec<Item> = stmt
            .query_map([], Item::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Some(items)
    } else {
        None
    };

    let output = PrimeOutput {
        timestamp: Utc::now().to_rfc3339(),
        stats: Stats {
            items: db_stats.items,
            prices: db_stats.prices,
            configurations: db_stats.configurations,
            decisions: db_stats.decisions,
            events: db_stats.events,
        },
        current,
        active_decision,
        stale_prices,
        recent_events,
        catalog,
    };

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(&output)?);
        }
        OutputFormat::Text => {
            print_text_output(&output);
        }
    }

    Ok(())
}

fn print_text_output(output: &PrimeOutput) {
    use console::style;

    println!("{}", style("Storage Planner Context").bold().cyan());
    println!("{}", style("═".repeat(40)).dim());
    println!();

    // Stats
    println!("{}", style("Database Stats:").bold());
    println!("  Items:          {}", output.stats.items);
    println!("  Prices:         {}", output.stats.prices);
    println!("  Configurations: {}", output.stats.configurations);
    println!("  Decisions:      {}", output.stats.decisions);
    println!("  Events:         {}", output.stats.events);
    println!();

    // Current configuration
    if let Some(ref current) = output.current {
        println!("{}", style("Current Configuration:").bold());
        println!("  Name:   {}", current.name);
        println!("  Domain: {}", current.domain);
        println!("  Items:  {}", current.items.len());
        if current.total_cost() > 0.0 {
            println!("  Cost:   ${:.2}", current.total_cost());
        }
        println!();
    } else {
        println!("{}", style("No current configuration deployed").dim());
        println!();
    }

    // Active decision
    if let Some(ref decision) = output.active_decision {
        println!("{}", style("Active Decision:").bold().yellow());
        println!("  Purpose: {}", decision.purpose);
        println!("  Options: {}", decision.options.len());
        for (name, config_id) in &decision.options {
            println!("    - {} ({})", name, config_id);
        }
        println!();
    }

    // Stale prices
    if !output.stale_prices.is_empty() {
        println!(
            "{} {} items with stale prices (>7 days):",
            style("!").yellow(),
            output.stale_prices.len()
        );
        for id in output.stale_prices.iter().take(5) {
            println!("    {}", id);
        }
        if output.stale_prices.len() > 5 {
            println!("    ... and {} more", output.stale_prices.len() - 5);
        }
        println!();
    }

    // Recent events
    if !output.recent_events.is_empty() {
        println!("{}", style("Recent Events:").bold());
        for event in output.recent_events.iter().take(5) {
            println!(
                "  {} {} {} ({})",
                style(&event.timestamp.format("%Y-%m-%d %H:%M")).dim(),
                event.event_type.as_str(),
                event.entity_id,
                event.actor
            );
        }
        println!();
    }

    // Catalog summary
    if let Some(ref catalog) = output.catalog {
        println!("{}", style("Catalog:").bold());
        let mut by_category: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for item in catalog {
            *by_category.entry(&item.category).or_insert(0) += 1;
        }
        for (cat, count) in by_category {
            println!("  {}: {} items", cat, count);
        }
    }
}
