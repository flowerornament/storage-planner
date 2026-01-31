//! sp doctor - Health check and diagnostics

use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use clap::Args;
use console::style;

use crate::core::db::Database;

#[derive(Args)]
pub struct DoctorArgs {
    /// Run integrity checks
    #[arg(long)]
    pub integrity: bool,
}

pub fn run(db_path: Utf8PathBuf, args: DoctorArgs) -> Result<()> {
    let mut issues: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // Check database exists
    if !db_path.exists() {
        println!("{} Database not found at {}", style("✗").red(), db_path);
        println!("  Run `sp init` to create a new database");
        return Ok(());
    }

    let db = Database::open(&db_path)?;

    // Check database is initialized
    if !db.is_initialized()? {
        issues.push("Database exists but schema not initialized. Run `sp init --force`".into());
    }

    // Get stats
    let stats = db.stats()?;

    // Check for orphaned prices (prices without items)
    let orphaned_prices: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM prices WHERE item_id NOT IN (SELECT id FROM items)",
        [],
        |row| row.get(0),
    )?;
    if orphaned_prices > 0 {
        warnings.push(format!("{} orphaned price records", orphaned_prices));
    }

    // Check for items without prices
    let items_without_prices: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM items WHERE archived = 0 AND id NOT IN (SELECT DISTINCT item_id FROM prices)",
        [],
        |row| row.get(0),
    )?;
    if items_without_prices > 0 && stats.items > 0 {
        warnings.push(format!("{} items have no price data", items_without_prices));
    }

    // Check for stale prices (>30 days)
    let stale_prices: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(DISTINCT item_id) FROM prices
         WHERE item_id IN (SELECT id FROM items WHERE archived = 0)
         GROUP BY item_id
         HAVING MAX(observed_at) < datetime('now', '-30 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if stale_prices > 0 {
        warnings.push(format!(
            "{} items have very stale prices (>30 days)",
            stale_prices
        ));
    }

    // Check for multiple current configurations
    let current_configs: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM configurations WHERE is_current = 1",
        [],
        |row| row.get(0),
    )?;
    if current_configs > 1 {
        issues.push(format!(
            "{} configurations marked as current (should be 1)",
            current_configs
        ));
    }

    // Run SQLite integrity check if requested
    if args.integrity {
        let integrity: String = db
            .conn()
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            issues.push(format!("SQLite integrity check failed: {}", integrity));
        }
    }

    // Print results
    println!("{}", style("Storage Planner Health Check").bold().cyan());
    println!("{}", style("═".repeat(40)).dim());
    println!();

    println!("{}", style("Database:").bold());
    println!("  Path:           {}", db_path);
    println!("  Items:          {}", stats.items);
    println!("  Prices:         {}", stats.prices);
    println!("  Configurations: {}", stats.configurations);
    println!("  Decisions:      {}", stats.decisions);
    println!("  Events:         {}", stats.events);
    println!();

    if issues.is_empty() && warnings.is_empty() {
        println!("{} All checks passed", style("✓").green());
    } else {
        if !issues.is_empty() {
            println!("{}", style("Issues:").bold().red());
            for issue in &issues {
                println!("  {} {}", style("✗").red(), issue);
            }
            println!();
        }

        if !warnings.is_empty() {
            println!("{}", style("Warnings:").bold().yellow());
            for warning in &warnings {
                println!("  {} {}", style("!").yellow(), warning);
            }
            println!();
        }
    }

    if !issues.is_empty() {
        bail!("Health check failed with {} issue(s)", issues.len());
    }

    Ok(())
}
