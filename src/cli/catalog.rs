//! sp catalog -- Manage product catalog and price observations
//!
//! Subcommands: add, show, list, search, price (add, list)
//! Catalog items are global (not topology-scoped).
//! All mutating commands log events for undo/redo support.

use anyhow::{bail, Result};
use clap::Subcommand;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::events::{record_event, EventSource};
use crate::core::models::{CatalogItem, Price};
use crate::core::resolve::resolve_catalog_item;

use super::OutputFormat;

#[derive(Subcommand)]
pub enum CatalogCommands {
    /// Add a product to the catalog
    Add {
        /// Product name
        name: String,

        /// Product category (e.g., ssd, hdd, nas, enclosure)
        #[arg(long)]
        category: String,

        /// Product specifications as JSON (e.g., '{"capacity_gb": 4000}')
        #[arg(long, default_value = "{}")]
        specs: String,

        /// Product URL
        #[arg(long)]
        url: Option<String>,

        /// Notes about the product
        #[arg(long)]
        notes: Option<String>,
    },

    /// Show details of a catalog item
    Show {
        /// Item name or UUID prefix
        item: String,
    },

    /// List catalog items
    List {
        /// Filter by category
        #[arg(long)]
        category: Option<String>,
    },

    /// Search catalog items by name, category, or notes
    Search {
        /// Search query
        query: String,
    },

    /// Manage price observations
    #[command(subcommand)]
    Price(PriceCommands),
}

#[derive(Subcommand)]
pub enum PriceCommands {
    /// Record a price observation for an item
    Add {
        /// Item name or UUID prefix
        item: String,

        /// Price amount in dollars (e.g., 289.99)
        #[arg(long, value_name = "DOLLARS")]
        amount: f64,

        /// Price source (e.g., amazon, bestbuy, ebay)
        #[arg(long, default_value = "manual")]
        source: String,

        /// Item condition (e.g., new, used, refurbished)
        #[arg(long, default_value = "new")]
        condition: String,

        /// Price type: one-time, monthly, or annual
        #[arg(long, value_name = "TYPE", default_value = "one-time", value_parser = clap::builder::PossibleValuesParser::new(["one-time", "monthly", "annual"]))]
        r#type: String,

        /// Currency code
        #[arg(long, default_value = "USD")]
        currency: String,
    },

    /// List price observations for an item
    List {
        /// Item name or UUID prefix
        item: String,
    },
}

pub fn run(cmd: CatalogCommands, db: &mut Database, format: OutputFormat) -> Result<()> {
    match cmd {
        CatalogCommands::Add {
            name,
            category,
            specs,
            url,
            notes,
        } => add(
            db,
            &name,
            &category,
            &specs,
            url.as_deref(),
            notes.as_deref(),
            format,
        ),
        CatalogCommands::Show { item } => show(db, &item, format),
        CatalogCommands::List { category } => list(db, category.as_deref(), format),
        CatalogCommands::Search { query } => search(db, &query, format),
        CatalogCommands::Price(price_cmd) => match price_cmd {
            PriceCommands::Add {
                item,
                amount,
                source,
                condition,
                r#type,
                currency,
            } => {
                if amount < 0.0 {
                    anyhow::bail!("Price amount cannot be negative (got ${:.2})", amount);
                }
                let amount_cents = (amount * 100.0).round() as i64;
                price_add(
                    db,
                    &item,
                    amount_cents,
                    &source,
                    &condition,
                    &r#type,
                    &currency,
                    format,
                )
            }
            PriceCommands::List { item } => price_list(db, &item, format),
        },
    }
}

// ---------------------------------------------------------------------------
// Item commands
// ---------------------------------------------------------------------------

fn add(
    db: &mut Database,
    name: &str,
    category: &str,
    specs_json: &str,
    url: Option<&str>,
    notes: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    // Parse specs JSON
    let specs: serde_json::Value = serde_json::from_str(specs_json).map_err(|e| {
        anyhow::anyhow!(
            "Invalid JSON for --specs: {}. Example: '{{\"capacity_gb\": 4000}}'",
            e
        )
    })?;

    // Pre-insert uniqueness check
    let existing: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM catalog_items WHERE name = ?1",
        params![name],
        |row| row.get(0),
    )?;
    if existing > 0 {
        bail!("Catalog item '{}' already exists", name);
    }

    let mut item = CatalogItem::new(name, category);
    item.specs = specs;
    if let Some(u) = url {
        item.url = Some(u.to_string());
    }
    if let Some(n) = notes {
        item.notes = n.to_string();
    }

    let after_json = item.to_json()?;
    let item_id = item.id.clone();
    let item_name = item.name.clone();

    db.transaction(|tx| {
        item.insert(tx)?;

        record_event(
            tx,
            "catalog_item.created",
            "catalog_item",
            &item_id,
            &format!("Added catalog item '{}'", item_name),
            None,
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    let id_prefix = &item_id[..8];
    match format {
        OutputFormat::Text => {
            println!("Added item: {} ({})", item_name, id_prefix);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "created",
                "item": item_name,
                "id": item_id,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn show(db: &mut Database, name_or_id: &str, format: OutputFormat) -> Result<()> {
    let item = resolve_catalog_item(db, name_or_id)?;

    // Query latest price (block-scoped per D023)
    let latest_price: Option<Price> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, item_id, amount_cents, currency, source, condition, price_type, observed_at \
             FROM prices WHERE item_id = ?1 ORDER BY observed_at DESC LIMIT 1",
        )?;
        let result = stmt
            .query_map(params![item.id], Price::from_row)?
            .next()
            .transpose()?;
        result
    };

    // Query price count
    let price_count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM prices WHERE item_id = ?1",
        params![item.id],
        |row| row.get(0),
    )?;

    match format {
        OutputFormat::Text => {
            println!("Item: {} [{}]", item.name, item.category);
            if item.specs != serde_json::json!({}) {
                println!("  Specs:           {}", item.specs);
            }
            if let Some(ref u) = item.url {
                println!("  URL:             {}", u);
            }
            if !item.notes.is_empty() {
                println!("  Notes:           {}", item.notes);
            }
            if let Some(ref price) = latest_price {
                println!(
                    "  Latest price:    ${:.2} ({}, {}, {})",
                    price.amount_dollars(),
                    price.source,
                    price.condition,
                    price.price_type
                );
            }
            println!("  Price records:   {}", price_count);
            println!("  ID:              {}", item.id);
            println!(
                "  Created:         {}",
                item.created_at.format("%Y-%m-%d %H:%M:%S")
            );
        }
        OutputFormat::Json => {
            let mut item_val = serde_json::to_value(&item)?;
            if let serde_json::Value::Object(ref mut map) = item_val {
                if let Some(ref price) = latest_price {
                    map.insert("latest_price".to_string(), serde_json::to_value(price)?);
                } else {
                    map.insert("latest_price".to_string(), serde_json::Value::Null);
                }
                map.insert(
                    "price_count".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(price_count)),
                );
            }
            println!("{}", serde_json::to_string_pretty(&item_val)?);
        }
    }

    Ok(())
}

fn list(db: &mut Database, category: Option<&str>, format: OutputFormat) -> Result<()> {
    let items: Vec<CatalogItem> = if let Some(cat) = category {
        let mut stmt = db.conn().prepare(
            "SELECT id, name, category, specs, url, notes, created_at, updated_at \
             FROM catalog_items WHERE category = ?1 ORDER BY name",
        )?;
        let result = stmt
            .query_map(params![cat], CatalogItem::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    } else {
        let mut stmt = db.conn().prepare(
            "SELECT id, name, category, specs, url, notes, created_at, updated_at \
             FROM catalog_items ORDER BY name",
        )?;
        let result = stmt
            .query_map([], CatalogItem::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    match format {
        OutputFormat::Text => {
            if items.is_empty() {
                println!(
                    "No catalog items found. Add one with 'sp catalog add <name> --category=<cat>'"
                );
            } else {
                println!(
                    "  {:<30} {:<12} {:<42} Latest Price",
                    "Name", "Category", "URL"
                );
                println!("  {}", "-".repeat(90));
                for item in &items {
                    // Query latest price for this item
                    let latest_price: Option<i64> = db
                        .conn()
                        .query_row(
                            "SELECT amount_cents FROM prices WHERE item_id = ?1 ORDER BY observed_at DESC LIMIT 1",
                            params![item.id],
                            |row| row.get(0),
                        )
                        .ok();

                    let price_str = match latest_price {
                        Some(cents) => format!("${:.2}", cents as f64 / 100.0),
                        None => "-".to_string(),
                    };

                    let url_str = item
                        .url
                        .as_deref()
                        .map(|u| {
                            if u.len() > 40 {
                                format!("{}...", &u[..37])
                            } else {
                                u.to_string()
                            }
                        })
                        .unwrap_or_else(|| "-".to_string());

                    println!(
                        "  {:<30} {:<12} {:<42} {}",
                        item.name, item.category, url_str, price_str
                    );
                }
            }
        }
        OutputFormat::Json => {
            let json: Vec<serde_json::Value> = items
                .iter()
                .map(|i| serde_json::to_value(i).unwrap_or_default())
                .collect();
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn search(db: &mut Database, query: &str, format: OutputFormat) -> Result<()> {
    let pattern = format!("%{}%", query);
    let mut stmt = db.conn().prepare(
        "SELECT id, name, category, specs, url, notes, created_at, updated_at \
         FROM catalog_items WHERE name LIKE ?1 OR category LIKE ?1 OR notes LIKE ?1 \
         ORDER BY name",
    )?;

    let items: Vec<CatalogItem> = stmt
        .query_map(params![pattern], CatalogItem::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Text => {
            if items.is_empty() {
                println!("No items matching '{}'", query);
            } else {
                for item in &items {
                    let latest_price: Option<i64> = db
                        .conn()
                        .query_row(
                            "SELECT amount_cents FROM prices WHERE item_id = ?1 ORDER BY observed_at DESC LIMIT 1",
                            params![item.id],
                            |row| row.get(0),
                        )
                        .ok();

                    let price_str = match latest_price {
                        Some(cents) => format!("${:.2}", cents as f64 / 100.0),
                        None => "-".to_string(),
                    };

                    let url_str = item
                        .url
                        .as_deref()
                        .map(|u| {
                            if u.len() > 40 {
                                format!("{}...", &u[..37])
                            } else {
                                u.to_string()
                            }
                        })
                        .unwrap_or_else(|| "-".to_string());

                    println!(
                        "  {:<30} {:<12} {:<42} {}",
                        item.name, item.category, url_str, price_str
                    );
                }
            }
        }
        OutputFormat::Json => {
            let json: Vec<serde_json::Value> = items
                .iter()
                .map(|i| serde_json::to_value(i).unwrap_or_default())
                .collect();
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Price commands
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn price_add(
    db: &mut Database,
    item_name_or_id: &str,
    amount_cents: i64,
    source: &str,
    condition: &str,
    price_type: &str,
    currency: &str,
    format: OutputFormat,
) -> Result<()> {
    // Validate price_type
    match price_type {
        "one-time" | "monthly" | "annual" => {}
        _ => bail!(
            "Invalid price type '{}'. Must be one of: one-time, monthly, annual",
            price_type
        ),
    }

    let item = resolve_catalog_item(db, item_name_or_id)?;

    let mut price = Price::new(&item.id, amount_cents);
    price.source = source.to_string();
    price.condition = condition.to_string();
    price.price_type = price_type.to_string();
    price.currency = currency.to_string();

    let after_json = price.to_json()?;
    let price_id = price.id.clone();
    let item_name = item.name.clone();

    db.transaction(|tx| {
        price.insert(tx)?;

        record_event(
            tx,
            "price.created",
            "price",
            &price_id,
            &format!(
                "Recorded price ${:.2} for '{}'",
                amount_cents as f64 / 100.0,
                item_name
            ),
            None,
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!(
                "Recorded price: ${:.2} for {} ({}, {}, {})",
                amount_cents as f64 / 100.0,
                item_name,
                source,
                condition,
                price_type
            );
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "created",
                "price_id": price_id,
                "item": item_name,
                "amount_cents": amount_cents,
                "source": source,
                "condition": condition,
                "price_type": price_type,
                "currency": currency,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn price_list(db: &mut Database, item_name_or_id: &str, format: OutputFormat) -> Result<()> {
    let item = resolve_catalog_item(db, item_name_or_id)?;

    let mut stmt = db.conn().prepare(
        "SELECT id, item_id, amount_cents, currency, source, condition, price_type, observed_at \
         FROM prices WHERE item_id = ?1 ORDER BY observed_at DESC",
    )?;

    let prices: Vec<Price> = stmt
        .query_map(params![item.id], Price::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Text => {
            if prices.is_empty() {
                println!("No price observations recorded for {}", item.name);
            } else {
                println!("Prices for {}:", item.name);
                println!(
                    "  {:<12} {:>10} {:<12} {:<12} Type",
                    "Date", "Amount", "Source", "Condition"
                );
                for price in &prices {
                    println!(
                        "  {:<12} {:>10} {:<12} {:<12} {}",
                        price.observed_at.format("%Y-%m-%d"),
                        format!("${:.2}", price.amount_dollars()),
                        price.source,
                        price.condition,
                        price.price_type
                    );
                }
            }
        }
        OutputFormat::Json => {
            let json: Vec<serde_json::Value> = prices
                .iter()
                .map(|p| serde_json::to_value(p).unwrap_or_default())
                .collect();
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}
