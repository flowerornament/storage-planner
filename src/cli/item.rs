//! sp item - Manage items in the catalog

use std::io::{self, Read};

use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use clap::{Args, Subcommand};
use console::style;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::core::db::Database;
use crate::core::events::{current_actor, EventLog};
use crate::core::models::{EntityType, EventType, Item, ItemCondition, Price, PriceSource};
use crate::pricing::{
    self, generate_agent_response, parse_url, print_fallback_instructions, FallbackReason,
    Identifiers, ParsedUrl, ProductFetcher, Retailer,
};

use super::OutputFormat;

#[derive(Subcommand)]
#[command(after_help = r#"EXAMPLES:
    # Add by URL (auto-fetch specs and price)
    sp item add --url="https://www.bestbuy.com/site/samsung-870-evo-4tb/6405087.p"

    # Add by URL without fetching (just store identifier)
    sp item add --url="https://amazon.com/dp/B089C5P5SX" --no-fetch

    # Add manually with full details
    sp item add samsung-870-evo-4tb --name="Samsung 870 EVO 4TB" --category=ssd

    # Import from JSON (agent workflow)
    sp item import --json='{"name":"Samsung 870 EVO 4TB","category":"ssd"}'

    # List and search
    sp item list --category=ssd --tags=quiet
    sp item show samsung-870-evo-4tb --prices
    sp item compare samsung-870-evo-4tb lexar-nm790-4tb
    sp item search "samsung evo"
"#)]
pub enum ItemCommands {
    /// Add a new item to the catalog
    Add(AddArgs),

    /// Import an item from JSON (agent workflow)
    Import(ImportArgs),

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
#[command(after_help = r#"EXAMPLES:
    # Add by URL (auto-fetch specs and price if API keys available)
    sp item add --url="https://www.bestbuy.com/site/samsung-870-evo-4tb/6405087.p"

    # Add by URL without auto-fetch
    sp item add --url="https://amazon.com/dp/B089C5P5SX" --no-fetch

    # Add by identifier
    sp item add --asin=B089C5P5SX

    # Add manually
    sp item add samsung-870-evo-4tb \
      --name="Samsung 870 EVO 4TB" \
      --category=ssd \
      --brand=Samsung \
      --specs='{"capacity":"4TB","read_speed":"560MB/s","interface":"SATA"}' \
      --tags=sata,ssd,quiet

    # Show fallback JSON for agents
    sp item add --url="https://amazon.com/dp/B089C5P5SX" --agent-mode
"#)]
pub struct AddArgs {
    /// Unique item ID (e.g., samsung-870-evo-4tb). Auto-generated from URL if not provided.
    #[arg(default_value = "")]
    pub id: String,

    /// Item name (auto-fetched from URL if not provided)
    #[arg(long)]
    pub name: Option<String>,

    /// Category (e.g., ssd, enclosure, software). Auto-detected from URL if possible.
    #[arg(long)]
    pub category: Option<String>,

    /// Brand name (auto-fetched from URL if not provided)
    #[arg(long)]
    pub brand: Option<String>,

    /// Specs as JSON (e.g., '{"capacity":"4TB","read_speed":"560MB/s"}')
    #[arg(long)]
    pub specs: Option<String>,

    /// Tags (comma-separated)
    #[arg(long)]
    pub tags: Option<String>,

    // URL-based entry options
    /// Product URL to parse and fetch (Amazon, Best Buy, eBay)
    #[arg(long, conflicts_with_all = ["asin", "upc"])]
    pub url: Option<String>,

    /// Amazon ASIN
    #[arg(long, conflicts_with_all = ["url", "upc"])]
    pub asin: Option<String>,

    /// Universal Product Code
    #[arg(long, conflicts_with_all = ["url", "asin"])]
    pub upc: Option<String>,

    /// Skip auto price/spec fetching
    #[arg(long)]
    pub no_fetch: bool,

    /// Output JSON for agent consumption on fallback
    #[arg(long)]
    pub agent_mode: bool,
}

#[derive(Args)]
#[command(after_help = r#"EXAMPLE:
    # Import from JSON string
    sp item import --json='{"name":"Samsung 870 EVO 4TB","brand":"Samsung","category":"ssd","specs":{"capacity":"4TB"},"price":289}'

    # Import from stdin
    echo '{"name":"...","category":"ssd"}' | sp item import --stdin

    # Import with custom ID
    sp item import --json='{"name":"Test Item","category":"ssd"}' --id=test-item
"#)]
pub struct ImportArgs {
    /// JSON data to import
    #[arg(long, conflicts_with = "stdin")]
    pub json: Option<String>,

    /// Read JSON from stdin
    #[arg(long, conflicts_with = "json")]
    pub stdin: bool,

    /// Override the generated item ID
    #[arg(long)]
    pub id: Option<String>,
}

/// JSON schema for item import
#[derive(Debug, Deserialize)]
struct ImportData {
    name: String,
    #[serde(default)]
    brand: Option<String>,
    category: String,
    #[serde(default)]
    specs: JsonValue,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    price: Option<f64>,
    #[serde(default)]
    condition: Option<String>,
    #[serde(default)]
    source: Option<String>,
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
    // Special case: Add with URL can work before DB exists to show fallback
    if let ItemCommands::Add(ref args) = cmd {
        if args.url.is_some() && args.agent_mode && !db_path.exists() {
            // Show fallback without requiring DB
            return handle_url_add_fallback(args, format);
        }
    }

    if !db_path.exists() {
        bail!("Database not found at {}. Run `sp init` first.", db_path);
    }

    match cmd {
        ItemCommands::Add(args) => add(db_path, args, format),
        ItemCommands::Import(args) => import(db_path, args, format),
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

    // Determine if we're doing URL-based or manual entry
    let (item, price_to_add) = if let Some(ref url) = args.url {
        add_from_url(&args, url, format)?
    } else if let Some(ref asin) = args.asin {
        add_from_identifier(&args, "asin", asin, format)?
    } else if let Some(ref upc) = args.upc {
        add_from_identifier(&args, "upc", upc, format)?
    } else {
        // Manual entry - require name and category
        if args.name.is_none() {
            bail!("--name is required for manual item entry");
        }
        if args.category.is_none() {
            bail!("--category is required for manual item entry");
        }
        if args.id.is_empty() {
            bail!("Item ID is required for manual entry");
        }
        (build_manual_item(&args)?, None)
    };

    let actor = current_actor();

    db.transaction(|tx| {
        // Check if ID already exists
        let exists: bool = tx
            .query_row("SELECT 1 FROM items WHERE id = ?1", [&item.id], |_| {
                Ok(true)
            })
            .unwrap_or(false);

        if exists {
            bail!("Item with ID '{}' already exists", item.id);
        }

        item.insert(tx)?;

        EventLog::record(
            tx,
            EventType::Created,
            EntityType::Item,
            &item.id,
            serde_json::json!({
                "name": item.name,
                "category": item.category,
            }),
            &actor,
        )?;

        // Add price if we got one from API
        if let Some(price) = price_to_add {
            price.insert(tx)?;
            EventLog::record(
                tx,
                EventType::PriceObserved,
                EntityType::Price,
                &price.id,
                serde_json::json!({
                    "item_id": item.id,
                    "price": price.price,
                    "source": price.source.as_str(),
                }),
                &actor,
            )?;
        }

        Ok(())
    })?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&item)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&item)?),
        OutputFormat::Text => {
            println!(
                "{} Added item: {} ({})",
                style("✓").green(),
                item.id,
                item.name
            );
        }
    }

    Ok(())
}

fn add_from_url(args: &AddArgs, url: &str, format: OutputFormat) -> Result<(Item, Option<Price>)> {
    // Parse the URL
    let parsed = parse_url(url)?;

    // Check if we should try to fetch
    if !args.no_fetch && !pricing::available_sources().is_empty() {
        // Try to fetch product info
        if let Ok(Some(product)) = fetch_from_parsed_url(&parsed) {
            return build_item_from_product(args, product, Some(parsed));
        }
    }

    // Fallback: no API available or fetch failed
    if args.agent_mode {
        let search_query = build_search_query_from_url(url, &parsed);
        let response = generate_agent_response(
            FallbackReason::NoApiKeys,
            &search_query,
            None,
            Some(serde_json::json!({
                "identifiers": {
                    parsed.identifier_key(): parsed.identifier
                },
                "source_url": url
            })),
        );
        println!("{}", response);
        bail!("Fallback required - agent should use sp item import");
    }

    // If we have required fields from CLI, create item with identifier stored
    if let (Some(name), Some(category)) = (&args.name, &args.category) {
        let id = if args.id.is_empty() {
            slugify(name)
        } else {
            args.id.clone()
        };

        let mut item = Item::new(&id, name, category);
        item.brand = args.brand.clone();

        if let Some(ref specs_str) = args.specs {
            item.specs = serde_json::from_str(specs_str)?;
        }

        if let Some(ref tags_str) = args.tags {
            item.tags = tags_str.split(',').map(|s| s.trim().to_string()).collect();
        }

        // Store identifier in metadata
        let mut metadata = serde_json::Map::new();
        let mut identifiers = serde_json::Map::new();
        identifiers.insert(
            parsed.identifier_key().to_string(),
            JsonValue::String(parsed.identifier.clone()),
        );
        metadata.insert("identifiers".to_string(), JsonValue::Object(identifiers));
        metadata.insert("source_url".to_string(), JsonValue::String(url.to_string()));
        item.metadata = JsonValue::Object(metadata);

        return Ok((item, None));
    }

    // Print human-readable fallback
    let search_query = build_search_query_from_url(url, &parsed);
    print_fallback_instructions(FallbackReason::NoApiKeys, &search_query);
    bail!("Could not auto-fetch product info. Use --name and --category, or use sp item import");
}

fn add_from_identifier(
    args: &AddArgs,
    id_type: &str,
    id_value: &str,
    format: OutputFormat,
) -> Result<(Item, Option<Price>)> {
    // Try to fetch by identifier
    if !args.no_fetch {
        let product = match id_type {
            "asin" => {
                // Amazon doesn't have a free API, just store the identifier
                None
            }
            "upc" => {
                // Try Best Buy by UPC
                let fetcher = pricing::bestbuy::BestBuyFetcher::new();
                if fetcher.is_available() {
                    fetcher.fetch_by_upc(id_value).ok().flatten()
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(product) = product {
            // Create fake ParsedUrl to store identifier
            let mut identifiers = Identifiers::default();
            match id_type {
                "asin" => identifiers.asin = Some(id_value.to_string()),
                "upc" => identifiers.upc = Some(id_value.to_string()),
                _ => {}
            }
            let mut product = product;
            product.identifiers = identifiers;
            return build_item_from_product(args, product, None);
        }
    }

    // Fallback
    if args.agent_mode {
        let search_query = format!("product {} {}", id_type.to_uppercase(), id_value);
        let response = generate_agent_response(
            FallbackReason::NoApiKeys,
            &search_query,
            None,
            Some(serde_json::json!({
                "identifiers": { id_type: id_value }
            })),
        );
        println!("{}", response);
        bail!("Fallback required - agent should use sp item import");
    }

    // If we have required fields from CLI
    if let (Some(name), Some(category)) = (&args.name, &args.category) {
        let item_id = if args.id.is_empty() {
            slugify(name)
        } else {
            args.id.clone()
        };

        let mut item = Item::new(&item_id, name, category);
        item.brand = args.brand.clone();

        if let Some(ref specs_str) = args.specs {
            item.specs = serde_json::from_str(specs_str)?;
        }

        if let Some(ref tags_str) = args.tags {
            item.tags = tags_str.split(',').map(|s| s.trim().to_string()).collect();
        }

        // Store identifier in metadata
        let mut metadata = serde_json::Map::new();
        let mut identifiers = serde_json::Map::new();
        identifiers.insert(id_type.to_string(), JsonValue::String(id_value.to_string()));
        metadata.insert("identifiers".to_string(), JsonValue::Object(identifiers));
        item.metadata = JsonValue::Object(metadata);

        return Ok((item, None));
    }

    let search_query = format!("product {} {}", id_type.to_uppercase(), id_value);
    print_fallback_instructions(FallbackReason::NoApiKeys, &search_query);
    bail!("Could not auto-fetch product info. Use --name and --category, or use sp item import");
}

fn fetch_from_parsed_url(parsed: &ParsedUrl) -> Result<Option<pricing::ProductInfo>> {
    match parsed.retailer {
        Retailer::BestBuy => {
            let fetcher = pricing::bestbuy::BestBuyFetcher::new();
            if fetcher.is_available() {
                return fetcher.fetch_by_id(&parsed.identifier);
            }
        }
        Retailer::Ebay => {
            let fetcher = pricing::ebay::EbayFetcher::new();
            if fetcher.is_available() {
                return fetcher.fetch_by_id(&parsed.identifier);
            }
        }
        Retailer::Amazon => {
            // Amazon doesn't have a free API
            // Could search Best Buy by UPC if we had it
        }
    }
    Ok(None)
}

fn build_item_from_product(
    args: &AddArgs,
    product: pricing::ProductInfo,
    parsed_url: Option<ParsedUrl>,
) -> Result<(Item, Option<Price>)> {
    // Use CLI args as overrides, product data as defaults
    let name = args.name.clone().unwrap_or(product.name.clone());
    let category = args
        .category
        .clone()
        .or(product.category.clone())
        .unwrap_or("other".to_string());
    let id = if args.id.is_empty() {
        product.suggested_item_id()
    } else {
        args.id.clone()
    };

    let mut item = Item::new(&id, &name, &category);
    item.brand = args.brand.clone().or(product.brand.clone());
    item.specs = if let Some(ref specs_str) = args.specs {
        serde_json::from_str(specs_str)?
    } else {
        product.specs.clone()
    };

    if let Some(ref tags_str) = args.tags {
        item.tags = tags_str.split(',').map(|s| s.trim().to_string()).collect();
    }

    // Build metadata with identifiers
    let mut metadata = serde_json::Map::new();
    metadata.insert("identifiers".to_string(), product.identifiers.to_json());
    if let Some(ref url) = product.source_url {
        metadata.insert("source_url".to_string(), JsonValue::String(url.clone()));
    } else if let Some(ref parsed) = parsed_url {
        metadata.insert(
            "source_url".to_string(),
            JsonValue::String(parsed.original_url.clone()),
        );
    }
    item.metadata = JsonValue::Object(metadata);

    // Build price if available
    let price = product.price.map(|p| {
        let source = if let Some(ref parsed) = parsed_url {
            match parsed.retailer {
                Retailer::Amazon => PriceSource::Amazon,
                Retailer::BestBuy => PriceSource::BestBuy,
                Retailer::Ebay => PriceSource::Ebay,
            }
        } else {
            PriceSource::Manual
        };
        let mut price_obj = Price::new(&id, source, p.amount, p.condition);
        price_obj.url = product.source_url.clone();
        price_obj
    });

    Ok((item, price))
}

fn build_manual_item(args: &AddArgs) -> Result<Item> {
    let name = args.name.as_ref().unwrap();
    let category = args.category.as_ref().unwrap();

    let mut item = Item::new(&args.id, name, category);
    item.brand = args.brand.clone();

    if let Some(ref specs_str) = args.specs {
        item.specs = serde_json::from_str(specs_str)?;
    }

    if let Some(ref tags_str) = args.tags {
        item.tags = tags_str.split(',').map(|s| s.trim().to_string()).collect();
    }

    Ok(item)
}

fn build_search_query_from_url(url: &str, parsed: &ParsedUrl) -> String {
    // Try to extract product name from URL path
    // e.g., https://www.bestbuy.com/site/samsung-870-evo-4tb/6405087.p
    if let Some(path_start) = url.find("/site/") {
        let path = &url[path_start + 6..];
        if let Some(end) = path.find('/') {
            let slug = &path[..end];
            return slug.replace('-', " ");
        }
    }

    // Fallback to just retailer + identifier
    format!("{} product {}", parsed.retailer.as_str(), parsed.identifier)
}

fn handle_url_add_fallback(args: &AddArgs, format: OutputFormat) -> Result<()> {
    let url = args.url.as_ref().unwrap();
    let parsed = parse_url(url)?;
    let search_query = build_search_query_from_url(url, &parsed);

    if args.agent_mode {
        let response = generate_agent_response(
            FallbackReason::NoApiKeys,
            &search_query,
            None,
            Some(serde_json::json!({
                "identifiers": {
                    parsed.identifier_key(): parsed.identifier
                },
                "source_url": url
            })),
        );
        println!("{}", response);
    } else {
        print_fallback_instructions(FallbackReason::NoApiKeys, &search_query);
    }

    Ok(())
}

fn import(db_path: Utf8PathBuf, args: ImportArgs, format: OutputFormat) -> Result<()> {
    // Read JSON from --json or stdin
    let json_str = if let Some(ref json) = args.json {
        json.clone()
    } else if args.stdin {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer
    } else {
        bail!("Either --json or --stdin is required");
    };

    let data: ImportData =
        serde_json::from_str(&json_str).map_err(|e| anyhow::anyhow!("Invalid JSON: {}", e))?;

    // Generate or use provided ID
    let id = args.id.unwrap_or_else(|| slugify(&data.name));

    let mut db = Database::open(&db_path)?;
    let actor = current_actor();

    let mut item = Item::new(&id, &data.name, &data.category);
    item.brand = data.brand;
    item.specs = if data.specs.is_null() {
        JsonValue::Object(Default::default())
    } else {
        data.specs
    };
    item.tags = data.tags.unwrap_or_default();

    let price_to_add = data.price.map(|amount| {
        let condition = data
            .condition
            .as_deref()
            .map(ItemCondition::parse)
            .unwrap_or(ItemCondition::New);
        let source = data
            .source
            .as_deref()
            .map(PriceSource::parse)
            .unwrap_or(PriceSource::Manual);
        Price::new(&id, source, amount, condition)
    });

    db.transaction(|tx| {
        // Check if ID already exists
        let exists: bool = tx
            .query_row("SELECT 1 FROM items WHERE id = ?1", [&item.id], |_| {
                Ok(true)
            })
            .unwrap_or(false);

        if exists {
            bail!("Item with ID '{}' already exists", item.id);
        }

        item.insert(tx)?;

        EventLog::record(
            tx,
            EventType::Created,
            EntityType::Item,
            &item.id,
            serde_json::json!({
                "name": item.name,
                "category": item.category,
                "source": "import",
            }),
            &actor,
        )?;

        // Add price if provided
        if let Some(ref price) = price_to_add {
            price.insert(tx)?;
            EventLog::record(
                tx,
                EventType::PriceObserved,
                EntityType::Price,
                &price.id,
                serde_json::json!({
                    "item_id": item.id,
                    "price": price.price,
                    "source": price.source.as_str(),
                }),
                &actor,
            )?;
        }

        Ok(())
    })?;

    match format {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "item": item,
                "price": price_to_add,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Yaml => {
            let output = serde_json::json!({
                "item": item,
                "price": price_to_add,
            });
            println!("{}", serde_yaml::to_string(&output)?);
        }
        OutputFormat::Text => {
            println!(
                "{} Imported item: {} ({})",
                style("✓").green(),
                item.id,
                item.name
            );
            if let Some(ref price) = price_to_add {
                println!("  {} Added price: ${:.2}", style("✓").green(), price.price);
            }
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
    let prices: Vec<Price> = if args.prices {
        let mut stmt = db.conn().prepare(
            "SELECT id, item_id, source, price, currency, condition, url, observed_at, metadata
             FROM prices WHERE item_id = ?1 ORDER BY observed_at DESC LIMIT 10",
        )?;
        let result = stmt
            .query_map([&args.id], Price::from_row)?
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

fn print_item_detail(item: &Item, prices: &[Price]) {
    println!("{}", style(&item.name).bold().cyan());
    println!("{}", style("═".repeat(40)).dim());
    println!();

    println!("ID:       {}", item.id);
    println!("Category: {}", item.category);
    if let Some(ref brand) = item.brand {
        println!("Brand:    {}", brand);
    }
    println!("Tags:     {}", item.tags.join(", "));

    // Show identifiers if present
    if let Some(identifiers) = item.metadata.get("identifiers") {
        if let Some(obj) = identifiers.as_object() {
            if !obj.is_empty() {
                println!();
                println!("{}", style("Identifiers:").bold());
                for (key, value) in obj {
                    if let Some(v) = value.as_str() {
                        println!("  {}: {}", key, v);
                    }
                }
            }
        }
    }

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

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
