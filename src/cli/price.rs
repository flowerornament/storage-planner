//! sp price - Manage price observations

use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use chrono::{Duration, Utc};
use clap::{Args, Subcommand};
use console::style;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::events::{current_actor, EventLog};
use crate::core::models::{EntityType, EventType, ItemCondition, Price, PriceSource};

use super::OutputFormat;

#[derive(Subcommand)]
pub enum PriceCommands {
    /// Add a manual price observation
    Add(AddArgs),

    /// Fetch prices from APIs (requires API keys)
    Fetch(FetchArgs),

    /// Show current prices for an item
    Show(ShowArgs),

    /// Show price history for an item
    History(HistoryArgs),

    /// Compare prices across items
    Compare(CompareArgs),
}

#[derive(Args)]
pub struct AddArgs {
    /// Item ID
    pub item_id: String,

    /// Price in USD
    #[arg(long)]
    pub price: f64,

    /// Condition (new, used, refurbished, open_box)
    #[arg(long, default_value = "new")]
    pub condition: String,

    /// Source name (for manual entries)
    #[arg(long, default_value = "manual")]
    pub source: String,

    /// URL where price was found
    #[arg(long)]
    pub url: Option<String>,
}

#[derive(Args)]
pub struct FetchArgs {
    /// Item ID to fetch prices for (omit for all stale items)
    pub item_id: Option<String>,

    /// Sources to fetch from (ebay, bestbuy, keepa)
    #[arg(long, short = 's', value_delimiter = ',')]
    pub sources: Option<Vec<String>>,

    /// Fetch all items, even with fresh prices
    #[arg(long)]
    pub all: bool,

    /// Only fetch items with prices older than N days
    #[arg(long, default_value = "7")]
    pub stale_days: i64,
}

#[derive(Args)]
pub struct ShowArgs {
    /// Item ID
    pub item_id: String,
}

#[derive(Args)]
pub struct HistoryArgs {
    /// Item ID
    pub item_id: String,

    /// Maximum entries to show
    #[arg(long, short = 'n', default_value = "20")]
    pub limit: usize,
}

#[derive(Args)]
pub struct CompareArgs {
    /// Item IDs to compare
    #[arg(required = true)]
    pub ids: Vec<String>,
}

pub fn run(db_path: Utf8PathBuf, cmd: PriceCommands, format: OutputFormat) -> Result<()> {
    if !db_path.exists() {
        bail!("Database not found at {}. Run `sp init` first.", db_path);
    }

    match cmd {
        PriceCommands::Add(args) => add(db_path, args, format),
        PriceCommands::Fetch(args) => fetch(db_path, args, format),
        PriceCommands::Show(args) => show(db_path, args, format),
        PriceCommands::History(args) => history(db_path, args, format),
        PriceCommands::Compare(args) => compare(db_path, args, format),
    }
}

fn add(db_path: Utf8PathBuf, args: AddArgs, format: OutputFormat) -> Result<()> {
    let mut db = Database::open(&db_path)?;
    let actor = current_actor();

    // Verify item exists
    let item_exists: bool = db
        .conn()
        .query_row(
            "SELECT 1 FROM items WHERE id = ?1 AND archived = 0",
            [&args.item_id],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if !item_exists {
        bail!("Item '{}' not found or archived", args.item_id);
    }

    let source = PriceSource::from_str(&args.source);
    let condition = ItemCondition::from_str(&args.condition);

    let mut price = Price::new(&args.item_id, source, args.price, condition);
    price.url = args.url;

    db.transaction(|tx| {
        price.insert(tx)?;

        EventLog::record(
            tx,
            EventType::PriceObserved,
            EntityType::Price,
            &price.id,
            serde_json::json!({
                "item_id": args.item_id,
                "price": args.price,
                "source": source.as_str(),
                "condition": condition.as_str(),
            }),
            &actor,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&price)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&price)?),
        OutputFormat::Text => {
            println!(
                "{} Added price: ${:.2} for {} ({}, {})",
                style("✓").green(),
                args.price,
                args.item_id,
                condition.as_str(),
                source.as_str()
            );
        }
    }

    Ok(())
}

fn fetch(db_path: Utf8PathBuf, args: FetchArgs, format: OutputFormat) -> Result<()> {
    let db = Database::open(&db_path)?;

    // Determine which items need price fetching
    let items_to_fetch: Vec<String> = if let Some(ref item_id) = args.item_id {
        vec![item_id.clone()]
    } else if args.all {
        let mut stmt = db
            .conn()
            .prepare("SELECT id FROM items WHERE archived = 0")?;
        let result = stmt.query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        result
    } else {
        // Get items with stale or missing prices
        let cutoff = Utc::now() - Duration::days(args.stale_days);
        let mut stmt = db.conn().prepare(
            "SELECT i.id FROM items i
             LEFT JOIN (
                 SELECT item_id, MAX(observed_at) as latest
                 FROM prices GROUP BY item_id
             ) p ON i.id = p.item_id
             WHERE i.archived = 0
               AND (p.latest IS NULL OR p.latest < ?1)",
        )?;
        let result = stmt.query_map([cutoff.to_rfc3339()], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    if items_to_fetch.is_empty() {
        match format {
            OutputFormat::Json => println!("{{\"message\": \"No items need price updates\"}}"),
            OutputFormat::Yaml => println!("message: No items need price updates"),
            OutputFormat::Text => println!("{}", style("No items need price updates").dim()),
        }
        return Ok(());
    }

    // Check for API keys
    let sources = args.sources.unwrap_or_else(|| vec!["ebay".into(), "bestbuy".into()]);
    let available_sources: Vec<&str> = sources
        .iter()
        .filter(|s| has_api_key(s))
        .map(|s| s.as_str())
        .collect();

    if available_sources.is_empty() {
        println!(
            "{} No API keys configured. Set environment variables:",
            style("!").yellow()
        );
        println!("  SP_EBAY_APP_ID, SP_EBAY_CERT_ID   - for eBay prices");
        println!("  SP_BESTBUY_API_KEY                - for Best Buy prices");
        println!("  SP_KEEPA_API_KEY                  - for Amazon (Keepa) prices");
        println!();
        println!("Use `sp price add` to manually add prices.");
        return Ok(());
    }

    match format {
        OutputFormat::Text => {
            println!(
                "Would fetch prices for {} item(s) from: {}",
                items_to_fetch.len(),
                available_sources.join(", ")
            );
            println!("{}", style("(API integration not yet implemented)").dim());
        }
        _ => {
            let output = serde_json::json!({
                "items_to_fetch": items_to_fetch,
                "sources": available_sources,
                "status": "not_implemented"
            });
            match format {
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&output)?),
                OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&output)?),
                _ => {}
            }
        }
    }

    Ok(())
}

fn show(db_path: Utf8PathBuf, args: ShowArgs, format: OutputFormat) -> Result<()> {
    let db = Database::open(&db_path)?;

    // Get item name
    let item_name: String = db
        .conn()
        .query_row(
            "SELECT name FROM items WHERE id = ?1",
            [&args.item_id],
            |row| row.get(0),
        )
        .map_err(|_| anyhow::anyhow!("Item '{}' not found", args.item_id))?;

    // Get latest price for each condition
    let mut stmt = db.conn().prepare(
        "SELECT p.id, p.item_id, p.source, p.price, p.currency, p.condition, p.url, p.observed_at, p.metadata
         FROM prices p
         INNER JOIN (
             SELECT condition, MAX(observed_at) as max_date
             FROM prices WHERE item_id = ?1
             GROUP BY condition
         ) latest ON p.condition = latest.condition AND p.observed_at = latest.max_date
         WHERE p.item_id = ?1
         ORDER BY p.condition",
    )?;

    let prices: Vec<Price> = stmt
        .query_map([&args.item_id], Price::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "item_id": args.item_id,
                "item_name": item_name,
                "prices": prices,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Yaml => {
            let output = serde_json::json!({
                "item_id": args.item_id,
                "item_name": item_name,
                "prices": prices,
            });
            println!("{}", serde_yaml::to_string(&output)?);
        }
        OutputFormat::Text => {
            print_price_summary(&args.item_id, &item_name, &prices);
        }
    }

    Ok(())
}

fn history(db_path: Utf8PathBuf, args: HistoryArgs, format: OutputFormat) -> Result<()> {
    let db = Database::open(&db_path)?;

    let mut stmt = db.conn().prepare(
        "SELECT id, item_id, source, price, currency, condition, url, observed_at, metadata
         FROM prices WHERE item_id = ?1
         ORDER BY observed_at DESC LIMIT ?2",
    )?;

    let prices: Vec<Price> = stmt
        .query_map(params![args.item_id, args.limit], Price::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&prices)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&prices)?),
        OutputFormat::Text => {
            print_price_history(&args.item_id, &prices);
        }
    }

    Ok(())
}

fn compare(db_path: Utf8PathBuf, args: CompareArgs, format: OutputFormat) -> Result<()> {
    let db = Database::open(&db_path)?;

    #[derive(serde::Serialize)]
    struct ItemPrice {
        item_id: String,
        item_name: String,
        new_price: Option<f64>,
        used_price: Option<f64>,
        new_staleness: Option<i64>,
        used_staleness: Option<i64>,
    }

    let mut results: Vec<ItemPrice> = Vec::new();

    for item_id in &args.ids {
        let item_name: String = db
            .conn()
            .query_row(
                "SELECT name FROM items WHERE id = ?1",
                [item_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| item_id.clone());

        // Get latest new price
        let new_price: Option<(f64, String)> = db
            .conn()
            .query_row(
                "SELECT price, observed_at FROM prices
                 WHERE item_id = ?1 AND condition = 'new'
                 ORDER BY observed_at DESC LIMIT 1",
                [item_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        // Get latest used price
        let used_price: Option<(f64, String)> = db
            .conn()
            .query_row(
                "SELECT price, observed_at FROM prices
                 WHERE item_id = ?1 AND condition = 'used'
                 ORDER BY observed_at DESC LIMIT 1",
                [item_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        let now = Utc::now();
        results.push(ItemPrice {
            item_id: item_id.clone(),
            item_name,
            new_price: new_price.as_ref().map(|(p, _)| *p),
            used_price: used_price.as_ref().map(|(p, _)| *p),
            new_staleness: new_price.and_then(|(_, d)| {
                chrono::DateTime::parse_from_rfc3339(&d)
                    .ok()
                    .map(|dt| (now - dt.with_timezone(&Utc)).num_days())
            }),
            used_staleness: used_price.and_then(|(_, d)| {
                chrono::DateTime::parse_from_rfc3339(&d)
                    .ok()
                    .map(|dt| (now - dt.with_timezone(&Utc)).num_days())
            }),
        });
    }

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&results)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&results)?),
        OutputFormat::Text => {
            print_price_comparison(&results);
        }
    }

    Ok(())
}

fn has_api_key(source: &str) -> bool {
    match source {
        "ebay" => std::env::var("SP_EBAY_APP_ID").is_ok(),
        "bestbuy" => std::env::var("SP_BESTBUY_API_KEY").is_ok(),
        "keepa" => std::env::var("SP_KEEPA_API_KEY").is_ok(),
        _ => false,
    }
}

fn print_price_summary(item_id: &str, item_name: &str, prices: &[Price]) {
    println!("{}", style(item_name).bold().cyan());
    println!("ID: {}", item_id);
    println!("{}", style("─".repeat(40)).dim());

    if prices.is_empty() {
        println!("{}", style("No price data available").dim());
        return;
    }

    for price in prices {
        let staleness = (Utc::now() - price.observed_at).num_days();
        let stale_indicator = if staleness > 7 {
            style(format!(" ({} days old)", staleness)).yellow()
        } else {
            style(format!(" ({} days old)", staleness)).dim()
        };

        println!(
            "  {:<12} ${:>8.2}  {}{}",
            price.condition.as_str(),
            price.price,
            style(price.source.as_str()).dim(),
            stale_indicator
        );
    }
}

fn print_price_history(item_id: &str, prices: &[Price]) {
    if prices.is_empty() {
        println!("{}", style("No price history available").dim());
        return;
    }

    println!("{}", style(format!("Price History: {}", item_id)).bold());
    println!("{}", style("─".repeat(70)).dim());

    println!(
        "{:<12} {:<10} {:>10} {:<12} {}",
        style("DATE").bold(),
        style("CONDITION").bold(),
        style("PRICE").bold(),
        style("SOURCE").bold(),
        style("URL").bold()
    );

    for price in prices {
        let date = price.observed_at.format("%Y-%m-%d");
        let url = price.url.as_deref().unwrap_or("-");
        let url_display = if url.len() > 30 {
            format!("{}...", &url[..27])
        } else {
            url.to_string()
        };

        println!(
            "{:<12} {:<10} ${:>9.2} {:<12} {}",
            date,
            price.condition.as_str(),
            price.price,
            price.source.as_str(),
            style(url_display).dim()
        );
    }
}

fn print_price_comparison(results: &[impl serde::Serialize]) {
    // This is a bit hacky but works for our ItemPrice struct
    let json = serde_json::to_value(results).unwrap();
    let items: Vec<serde_json::Value> = json.as_array().unwrap().clone();

    println!(
        "{:<25} {:>12} {:>12}",
        style("ITEM").bold(),
        style("NEW").bold(),
        style("USED").bold()
    );
    println!("{}", style("─".repeat(52)).dim());

    for item in items {
        let name = item["item_name"].as_str().unwrap_or("?");
        let name_display = if name.len() > 24 {
            format!("{}...", &name[..21])
        } else {
            name.to_string()
        };

        let new_str = item["new_price"]
            .as_f64()
            .map(|p| format!("${:.2}", p))
            .unwrap_or_else(|| "-".to_string());

        let used_str = item["used_price"]
            .as_f64()
            .map(|p| format!("${:.2}", p))
            .unwrap_or_else(|| "-".to_string());

        println!("{:<25} {:>12} {:>12}", name_display, new_str, used_str);
    }
}
