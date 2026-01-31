//! sp item - Manage items in the catalog

use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use clap::{Args, Subcommand};
use console::style;
use rusqlite::params;
use serde_json::Value as JsonValue;

use crate::core::db::Database;
use crate::core::events::{current_actor, EventLog};
use crate::core::models::{EntityType, EventType, Item};

use super::OutputFormat;

#[derive(Subcommand)]
pub enum ItemCommands {
    /// Add a new item to the catalog
    Add(AddArgs),

    /// Update an existing item
    Update(UpdateArgs),

    /// Archive an item (soft delete)
    Archive(ArchiveArgs),

    /// Show item details
    Show(ShowArgs),

    /// List items with filtering
    List(ListArgs),

    /// Search items by text
    Search(SearchArgs),

    /// Compare multiple items
    Compare(CompareArgs),
}

#[derive(Args)]
pub struct AddArgs {
    /// Unique item ID (e.g., samsung-870-evo-4tb)
    pub id: String,

    /// Item name
    #[arg(long)]
    pub name: String,

    /// Category (e.g., ssd, enclosure, software)
    #[arg(long)]
    pub category: String,

    /// Brand name
    #[arg(long)]
    pub brand: Option<String>,

    /// Specs as JSON (e.g., '{"capacity":"4TB","read_speed":"560MB/s"}')
    #[arg(long, default_value = "{}")]
    pub specs: String,

    /// Tags (comma-separated)
    #[arg(long)]
    pub tags: Option<String>,
}

#[derive(Args)]
pub struct UpdateArgs {
    /// Item ID to update
    pub id: String,

    /// New name
    #[arg(long)]
    pub name: Option<String>,

    /// New category
    #[arg(long)]
    pub category: Option<String>,

    /// New brand
    #[arg(long)]
    pub brand: Option<String>,

    /// Specs to merge (JSON)
    #[arg(long)]
    pub specs: Option<String>,

    /// Tags to set (comma-separated, replaces existing)
    #[arg(long)]
    pub tags: Option<String>,
}

#[derive(Args)]
pub struct ArchiveArgs {
    /// Item ID to archive
    pub id: String,
}

#[derive(Args)]
pub struct ShowArgs {
    /// Item ID to show
    pub id: String,

    /// Include price history
    #[arg(long)]
    pub prices: bool,
}

#[derive(Args)]
pub struct ListArgs {
    /// Filter by category
    #[arg(long, short = 'c')]
    pub category: Option<String>,

    /// Filter by tags (comma-separated, matches any)
    #[arg(long, short = 't')]
    pub tags: Option<String>,

    /// Include archived items
    #[arg(long)]
    pub archived: bool,

    /// Maximum items to show
    #[arg(long, short = 'n', default_value = "50")]
    pub limit: usize,
}

#[derive(Args)]
pub struct SearchArgs {
    /// Search query
    pub query: String,

    /// Maximum results
    #[arg(long, short = 'n', default_value = "20")]
    pub limit: usize,
}

#[derive(Args)]
pub struct CompareArgs {
    /// Item IDs to compare
    #[arg(required = true)]
    pub ids: Vec<String>,
}

pub fn run(db_path: Utf8PathBuf, cmd: ItemCommands, format: OutputFormat) -> Result<()> {
    if !db_path.exists() {
        bail!("Database not found at {}. Run `sp init` first.", db_path);
    }

    match cmd {
        ItemCommands::Add(args) => add(db_path, args, format),
        ItemCommands::Update(args) => update(db_path, args, format),
        ItemCommands::Archive(args) => archive(db_path, args),
        ItemCommands::Show(args) => show(db_path, args, format),
        ItemCommands::List(args) => list(db_path, args, format),
        ItemCommands::Search(args) => search(db_path, args, format),
        ItemCommands::Compare(args) => compare(db_path, args, format),
    }
}

fn add(db_path: Utf8PathBuf, args: AddArgs, format: OutputFormat) -> Result<()> {
    let mut db = Database::open(&db_path)?;

    // Parse specs
    let specs: JsonValue = serde_json::from_str(&args.specs)
        .map_err(|e| anyhow::anyhow!("Invalid specs JSON: {}", e))?;

    // Parse tags
    let tags: Vec<String> = args
        .tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let mut item = Item::new(&args.id, &args.name, &args.category);
    item.brand = args.brand;
    item.specs = specs;
    item.tags = tags;

    let actor = current_actor();

    db.transaction(|tx| {
        // Check if ID already exists
        let exists: bool = tx.query_row(
            "SELECT 1 FROM items WHERE id = ?1",
            [&args.id],
            |_| Ok(true),
        ).unwrap_or(false);

        if exists {
            bail!("Item with ID '{}' already exists", args.id);
        }

        item.insert(tx)?;

        EventLog::record(
            tx,
            EventType::Created,
            EntityType::Item,
            &args.id,
            serde_json::json!({
                "name": item.name,
                "category": item.category,
            }),
            &actor,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&item)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&item)?),
        OutputFormat::Text => {
            println!("{} Added item: {} ({})", style("✓").green(), args.id, args.name);
        }
    }

    Ok(())
}

fn update(db_path: Utf8PathBuf, args: UpdateArgs, format: OutputFormat) -> Result<()> {
    let mut db = Database::open(&db_path)?;
    let actor = current_actor();

    let item = db.transaction(|tx| {
        // Get existing item
        let mut item: Item = tx.query_row(
            "SELECT id, name, category, brand, specs, tags, metadata, archived, created_at, updated_at
             FROM items WHERE id = ?1",
            [&args.id],
            Item::from_row,
        ).map_err(|_| anyhow::anyhow!("Item '{}' not found", args.id))?;

        // Apply updates
        if let Some(name) = args.name {
            item.name = name;
        }
        if let Some(category) = args.category {
            item.category = category;
        }
        if let Some(brand) = args.brand {
            item.brand = Some(brand);
        }
        if let Some(specs_str) = args.specs {
            let new_specs: JsonValue = serde_json::from_str(&specs_str)?;
            // Merge specs
            if let (JsonValue::Object(existing), JsonValue::Object(new)) =
                (&mut item.specs, new_specs)
            {
                for (k, v) in new {
                    existing.insert(k, v);
                }
            }
        }
        if let Some(tags_str) = args.tags {
            item.tags = tags_str.split(',').map(|s| s.trim().to_string()).collect();
        }

        item.updated_at = chrono::Utc::now();

        // Update in database
        tx.execute(
            "UPDATE items SET name = ?1, category = ?2, brand = ?3, specs = ?4, tags = ?5, updated_at = ?6
             WHERE id = ?7",
            params![
                item.name,
                item.category,
                item.brand,
                serde_json::to_string(&item.specs)?,
                serde_json::to_string(&item.tags)?,
                item.updated_at.to_rfc3339(),
                item.id,
            ],
        )?;

        EventLog::record(
            tx,
            EventType::Updated,
            EntityType::Item,
            &item.id,
            serde_json::json!({"updated_fields": "multiple"}),
            &actor,
        )?;

        Ok(item)
    })?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&item)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&item)?),
        OutputFormat::Text => {
            println!("{} Updated item: {}", style("✓").green(), args.id);
        }
    }

    Ok(())
}

fn archive(db_path: Utf8PathBuf, args: ArchiveArgs) -> Result<()> {
    let mut db = Database::open(&db_path)?;
    let actor = current_actor();

    db.transaction(|tx| {
        let affected = tx.execute(
            "UPDATE items SET archived = 1, updated_at = datetime('now') WHERE id = ?1 AND archived = 0",
            [&args.id],
        )?;

        if affected == 0 {
            bail!("Item '{}' not found or already archived", args.id);
        }

        EventLog::record(
            tx,
            EventType::Archived,
            EntityType::Item,
            &args.id,
            serde_json::json!({}),
            &actor,
        )?;

        Ok(())
    })?;

    println!("{} Archived item: {}", style("✓").green(), args.id);
    Ok(())
}

fn show(db_path: Utf8PathBuf, args: ShowArgs, format: OutputFormat) -> Result<()> {
    let db = Database::open(&db_path)?;

    let item: Item = db
        .conn()
        .query_row(
            "SELECT id, name, category, brand, specs, tags, metadata, archived, created_at, updated_at
             FROM items WHERE id = ?1",
            [&args.id],
            Item::from_row,
        )
        .map_err(|_| anyhow::anyhow!("Item '{}' not found", args.id))?;

    // Get prices if requested
    let prices: Vec<crate::core::models::Price> = if args.prices {
        let mut stmt = db.conn().prepare(
            "SELECT id, item_id, source, price, currency, condition, url, observed_at, metadata
             FROM prices WHERE item_id = ?1 ORDER BY observed_at DESC LIMIT 10",
        )?;
        let result = stmt.query_map([&args.id], crate::core::models::Price::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    } else {
        Vec::new()
    };

    match format {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "item": item,
                "prices": prices,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Yaml => {
            let output = serde_json::json!({
                "item": item,
                "prices": prices,
            });
            println!("{}", serde_yaml::to_string(&output)?);
        }
        OutputFormat::Text => {
            print_item_detail(&item, &prices);
        }
    }

    Ok(())
}

fn list(db_path: Utf8PathBuf, args: ListArgs, format: OutputFormat) -> Result<()> {
    let db = Database::open(&db_path)?;

    let mut sql = String::from(
        "SELECT id, name, category, brand, specs, tags, metadata, archived, created_at, updated_at FROM items WHERE 1=1",
    );
    let mut params: Vec<String> = Vec::new();

    if !args.archived {
        sql.push_str(" AND archived = 0");
    }

    if let Some(ref category) = args.category {
        sql.push_str(&format!(" AND category = ?{}", params.len() + 1));
        params.push(category.clone());
    }

    sql.push_str(" ORDER BY category, name");
    sql.push_str(&format!(" LIMIT {}", args.limit));

    let mut stmt = db.conn().prepare(&sql)?;
    let items: Vec<Item> = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), Item::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    // Filter by tags if specified
    let items: Vec<Item> = if let Some(ref tags_filter) = args.tags {
        let filter_tags: Vec<&str> = tags_filter.split(',').map(|s| s.trim()).collect();
        items
            .into_iter()
            .filter(|item| item.tags.iter().any(|t| filter_tags.contains(&t.as_str())))
            .collect()
    } else {
        items
    };

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&items)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&items)?),
        OutputFormat::Text => print_item_list(&items),
    }

    Ok(())
}

fn search(db_path: Utf8PathBuf, args: SearchArgs, format: OutputFormat) -> Result<()> {
    let db = Database::open(&db_path)?;

    let mut stmt = db.conn().prepare(
        "SELECT i.id, i.name, i.category, i.brand, i.specs, i.tags, i.metadata, i.archived, i.created_at, i.updated_at
         FROM items i
         JOIN items_fts fts ON i.rowid = fts.rowid
         WHERE items_fts MATCH ?1 AND i.archived = 0
         LIMIT ?2",
    )?;

    let items: Vec<Item> = stmt
        .query_map(params![args.query, args.limit], Item::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&items)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&items)?),
        OutputFormat::Text => {
            if items.is_empty() {
                println!("{}", style("No items found").dim());
            } else {
                print_item_list(&items);
            }
        }
    }

    Ok(())
}

fn compare(db_path: Utf8PathBuf, args: CompareArgs, format: OutputFormat) -> Result<()> {
    let db = Database::open(&db_path)?;

    let mut items: Vec<Item> = Vec::new();
    for id in &args.ids {
        let item: Item = db
            .conn()
            .query_row(
                "SELECT id, name, category, brand, specs, tags, metadata, archived, created_at, updated_at
                 FROM items WHERE id = ?1",
                [id],
                Item::from_row,
            )
            .map_err(|_| anyhow::anyhow!("Item '{}' not found", id))?;
        items.push(item);
    }

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&items)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&items)?),
        OutputFormat::Text => print_item_comparison(&items),
    }

    Ok(())
}

fn print_item_list(items: &[Item]) {
    if items.is_empty() {
        println!("{}", style("No items found").dim());
        return;
    }

    println!(
        "{:<30} {:<15} {:<20} {}",
        style("ID").bold(),
        style("CATEGORY").bold(),
        style("BRAND").bold(),
        style("NAME").bold()
    );
    println!("{}", style("─".repeat(85)).dim());

    for item in items {
        println!(
            "{:<30} {:<15} {:<20} {}",
            truncate(&item.id, 29),
            item.category,
            item.brand.as_deref().unwrap_or("-"),
            item.name
        );
    }
}

fn print_item_detail(item: &Item, prices: &[crate::core::models::Price]) {
    println!("{}", style(&item.name).bold().cyan());
    println!("{}", style("═".repeat(40)).dim());
    println!();

    println!("ID:       {}", item.id);
    println!("Category: {}", item.category);
    if let Some(ref brand) = item.brand {
        println!("Brand:    {}", brand);
    }
    println!("Tags:     {}", item.tags.join(", "));
    println!();

    println!("{}", style("Specs:").bold());
    if let Some(obj) = item.specs.as_object() {
        for (key, value) in obj {
            let value_str = match value {
                serde_json::Value::String(s) => s.clone(),
                v => v.to_string(),
            };
            println!("  {}: {}", key, value_str);
        }
    }

    if !prices.is_empty() {
        println!();
        println!("{}", style("Recent Prices:").bold());
        for price in prices {
            println!(
                "  ${:.2} ({}, {}) - {}",
                price.price,
                price.condition.as_str(),
                price.source.as_str(),
                price.observed_at.format("%Y-%m-%d")
            );
        }
    }
}

fn print_item_comparison(items: &[Item]) {
    if items.is_empty() {
        return;
    }

    // Collect all spec keys
    let mut all_keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for item in items {
        if let Some(obj) = item.specs.as_object() {
            all_keys.extend(obj.keys().cloned());
        }
    }

    // Print header
    print!("{:<20}", style("SPEC").bold());
    for item in items {
        print!(" {:<25}", style(truncate(&item.id, 24)).bold());
    }
    println!();
    println!("{}", style("─".repeat(20 + items.len() * 26)).dim());

    // Print name
    print!("{:<20}", "Name");
    for item in items {
        print!(" {:<25}", truncate(&item.name, 24));
    }
    println!();

    // Print each spec
    for key in all_keys {
        print!("{:<20}", key);
        for item in items {
            let value = item
                .specs
                .get(&key)
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    v => v.to_string(),
                })
                .unwrap_or_else(|| "-".to_string());
            print!(" {:<25}", truncate(&value, 24));
        }
        println!();
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
