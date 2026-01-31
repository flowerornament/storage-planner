//! sp config - Manage configurations

use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use clap::{Args, Subcommand};
use console::style;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::events::{current_actor, EventLog};
use crate::core::models::{ConfigItem, Configuration, EntityType, EventType};

use super::OutputFormat;

#[derive(Subcommand)]
#[command(after_help = r#"EXAMPLES:
    # Create a new configuration
    sp config create "My Setup" --domain=storage

    # Clone from current deployment
    sp config create "Variant" --from-current

    # Add items to a configuration
    sp config add-item "My Setup" samsung-870-evo-4tb --qty=2
    sp config add-item "My Setup" owc-dual-mini --qty=1

    # View configuration details
    sp config show "My Setup"
    sp config current              # View deployed configuration

    # List all configurations
    sp config list
    sp config list --archived      # Include archived

    # Deploy a configuration
    sp config set-current "My Setup"

    # Remove and archive
    sp config remove-item "My Setup" samsung-870-evo-4tb
    sp config archive "Old Setup"
"#)]
pub enum ConfigCommands {
    /// Show current deployed configuration
    Current(CurrentArgs),

    /// Create a new configuration
    Create(CreateArgs),

    /// Clone an existing configuration
    Clone(CloneArgs),

    /// Show configuration details
    Show(ShowArgs),

    /// List all configurations
    List(ListArgs),

    /// Add an item to a configuration
    AddItem(AddItemArgs),

    /// Remove an item from a configuration
    RemoveItem(RemoveItemArgs),

    /// Archive a configuration
    Archive(ArchiveArgs),

    /// Set a configuration as current (deploy)
    SetCurrent(SetCurrentArgs),
}

#[derive(Args)]
pub struct CurrentArgs {}

#[derive(Args)]
pub struct CreateArgs {
    /// Configuration name
    pub name: String,

    /// Domain (storage, computing, etc.)
    #[arg(long, default_value = "storage")]
    pub domain: String,

    /// Create from current configuration
    #[arg(long)]
    pub from_current: bool,
}

#[derive(Args)]
pub struct CloneArgs {
    /// Source configuration ID or name
    pub source: String,

    /// New configuration name
    #[arg(long)]
    pub name: String,
}

#[derive(Args)]
pub struct ShowArgs {
    /// Configuration ID or name
    pub id: String,
}

#[derive(Args)]
pub struct ListArgs {
    /// Include archived configurations
    #[arg(long)]
    pub archived: bool,

    /// Filter by domain
    #[arg(long)]
    pub domain: Option<String>,
}

#[derive(Args)]
pub struct AddItemArgs {
    /// Configuration ID or name
    pub config: String,

    /// Item ID to add
    pub item_id: String,

    /// Quantity
    #[arg(long, default_value = "1")]
    pub qty: u32,

    /// Unit price override
    #[arg(long)]
    pub price: Option<f64>,

    /// Notes
    #[arg(long)]
    pub notes: Option<String>,
}

#[derive(Args)]
pub struct RemoveItemArgs {
    /// Configuration ID or name
    pub config: String,

    /// Item ID to remove
    pub item_id: String,
}

#[derive(Args)]
pub struct ArchiveArgs {
    /// Configuration ID or name
    pub id: String,
}

#[derive(Args)]
pub struct SetCurrentArgs {
    /// Configuration ID or name
    pub id: String,
}

pub fn run(db_path: Utf8PathBuf, cmd: ConfigCommands, format: OutputFormat) -> Result<()> {
    if !db_path.exists() {
        bail!("Database not found at {}. Run `sp init` first.", db_path);
    }

    match cmd {
        ConfigCommands::Current(args) => current(db_path, args, format),
        ConfigCommands::Create(args) => create(db_path, args, format),
        ConfigCommands::Clone(args) => clone(db_path, args, format),
        ConfigCommands::Show(args) => show(db_path, args, format),
        ConfigCommands::List(args) => list(db_path, args, format),
        ConfigCommands::AddItem(args) => add_item(db_path, args),
        ConfigCommands::RemoveItem(args) => remove_item(db_path, args),
        ConfigCommands::Archive(args) => archive(db_path, args),
        ConfigCommands::SetCurrent(args) => set_current(db_path, args),
    }
}

fn current(_db_path: Utf8PathBuf, _args: CurrentArgs, format: OutputFormat) -> Result<()> {
    let db = Database::open(&_db_path)?;

    let config: Option<Configuration> = db
        .conn()
        .query_row(
            "SELECT id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at
             FROM configurations WHERE is_current = 1 LIMIT 1",
            [],
            Configuration::from_row,
        )
        .ok();

    match format {
        OutputFormat::Json => {
            if let Some(ref c) = config {
                println!("{}", serde_json::to_string_pretty(c)?);
            } else {
                println!("null");
            }
        }
        OutputFormat::Yaml => {
            if let Some(ref c) = config {
                println!("{}", serde_yaml::to_string(c)?);
            } else {
                println!("~");
            }
        }
        OutputFormat::Text => {
            if let Some(c) = config {
                print_config_detail(&c);
            } else {
                println!("{}", style("No current configuration deployed").dim());
                println!("Use `sp config set-current <id>` to deploy a configuration");
            }
        }
    }

    Ok(())
}

fn create(db_path: Utf8PathBuf, args: CreateArgs, format: OutputFormat) -> Result<()> {
    let mut db = Database::open(&db_path)?;
    let actor = current_actor();

    let config_id = uuid::Uuid::new_v4().to_string();

    let config = if args.from_current {
        // Clone from current
        let current: Configuration = db
            .conn()
            .query_row(
                "SELECT id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at
                 FROM configurations WHERE is_current = 1 LIMIT 1",
                [],
                Configuration::from_row,
            )
            .map_err(|_| anyhow::anyhow!("No current configuration to clone from"))?;

        let mut new_config = Configuration::new(&config_id, &args.name, &current.domain);
        new_config.items = current.items;
        new_config.domain_data = current.domain_data;
        new_config
    } else {
        Configuration::new(&config_id, &args.name, &args.domain)
    };

    db.transaction(|tx| {
        config.insert(tx)?;

        EventLog::record(
            tx,
            EventType::Created,
            EntityType::Configuration,
            &config_id,
            serde_json::json!({
                "name": args.name,
                "domain": config.domain,
            }),
            &actor,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&config)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&config)?),
        OutputFormat::Text => {
            println!(
                "{} Created configuration: {} ({})",
                style("✓").green(),
                args.name,
                &config_id[..8]
            );
        }
    }

    Ok(())
}

fn clone(db_path: Utf8PathBuf, args: CloneArgs, format: OutputFormat) -> Result<()> {
    let mut db = Database::open(&db_path)?;
    let actor = current_actor();

    // Find source configuration
    let source: Configuration = find_config(&db, &args.source)?;

    let new_id = uuid::Uuid::new_v4().to_string();
    let mut new_config = Configuration::new(&new_id, &args.name, &source.domain);
    new_config.items = source.items;
    new_config.domain_data = source.domain_data;
    new_config.metadata = serde_json::json!({"cloned_from": source.id});

    db.transaction(|tx| {
        new_config.insert(tx)?;

        EventLog::record(
            tx,
            EventType::Created,
            EntityType::Configuration,
            &new_id,
            serde_json::json!({
                "name": args.name,
                "cloned_from": source.id,
            }),
            &actor,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&new_config)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&new_config)?),
        OutputFormat::Text => {
            println!(
                "{} Cloned configuration: {} ({})",
                style("✓").green(),
                args.name,
                &new_id[..8]
            );
        }
    }

    Ok(())
}

fn show(db_path: Utf8PathBuf, args: ShowArgs, format: OutputFormat) -> Result<()> {
    let db = Database::open(&db_path)?;
    let config = find_config(&db, &args.id)?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&config)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&config)?),
        OutputFormat::Text => print_config_detail(&config),
    }

    Ok(())
}

fn list(db_path: Utf8PathBuf, args: ListArgs, format: OutputFormat) -> Result<()> {
    let db = Database::open(&db_path)?;

    let mut sql = String::from(
        "SELECT id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at
         FROM configurations WHERE 1=1",
    );

    if !args.archived {
        sql.push_str(" AND archived = 0");
    }

    if let Some(ref domain) = args.domain {
        sql.push_str(&format!(" AND domain = '{}'", domain));
    }

    sql.push_str(" ORDER BY is_current DESC, created_at DESC");

    let mut stmt = db.conn().prepare(&sql)?;
    let configs: Vec<Configuration> = stmt
        .query_map([], Configuration::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&configs)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&configs)?),
        OutputFormat::Text => print_config_list(&configs),
    }

    Ok(())
}

fn add_item(db_path: Utf8PathBuf, args: AddItemArgs) -> Result<()> {
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

    // Get price if not specified
    let unit_price = if let Some(p) = args.price {
        Some(p)
    } else {
        db.conn()
            .query_row(
                "SELECT price FROM prices WHERE item_id = ?1 AND condition = 'new'
                 ORDER BY observed_at DESC LIMIT 1",
                [&args.item_id],
                |row| row.get(0),
            )
            .ok()
    };

    db.transaction(|tx| {
        let mut config = find_config_tx(tx, &args.config)?;

        // Check if item already exists
        if let Some(existing) = config.items.iter_mut().find(|i| i.item_id == args.item_id) {
            existing.quantity += args.qty;
            if args.price.is_some() {
                existing.unit_price = args.price;
            }
            if args.notes.is_some() {
                existing.notes = args.notes.clone();
            }
        } else {
            config.items.push(ConfigItem {
                item_id: args.item_id.clone(),
                quantity: args.qty,
                unit_price,
                notes: args.notes.clone(),
            });
        }

        config.updated_at = chrono::Utc::now();

        tx.execute(
            "UPDATE configurations SET items = ?1, updated_at = ?2 WHERE id = ?3",
            params![
                serde_json::to_string(&config.items)?,
                config.updated_at.to_rfc3339(),
                config.id,
            ],
        )?;

        EventLog::record(
            tx,
            EventType::Updated,
            EntityType::Configuration,
            &config.id,
            serde_json::json!({
                "action": "add_item",
                "item_id": args.item_id,
                "quantity": args.qty,
            }),
            &actor,
        )?;

        Ok(())
    })?;

    println!(
        "{} Added {} x {} to configuration",
        style("✓").green(),
        args.qty,
        args.item_id
    );

    Ok(())
}

fn remove_item(db_path: Utf8PathBuf, args: RemoveItemArgs) -> Result<()> {
    let mut db = Database::open(&db_path)?;
    let actor = current_actor();

    db.transaction(|tx| {
        let mut config = find_config_tx(tx, &args.config)?;

        let initial_len = config.items.len();
        config.items.retain(|i| i.item_id != args.item_id);

        if config.items.len() == initial_len {
            bail!("Item '{}' not found in configuration", args.item_id);
        }

        config.updated_at = chrono::Utc::now();

        tx.execute(
            "UPDATE configurations SET items = ?1, updated_at = ?2 WHERE id = ?3",
            params![
                serde_json::to_string(&config.items)?,
                config.updated_at.to_rfc3339(),
                config.id,
            ],
        )?;

        EventLog::record(
            tx,
            EventType::Updated,
            EntityType::Configuration,
            &config.id,
            serde_json::json!({
                "action": "remove_item",
                "item_id": args.item_id,
            }),
            &actor,
        )?;

        Ok(())
    })?;

    println!(
        "{} Removed {} from configuration",
        style("✓").green(),
        args.item_id
    );

    Ok(())
}

fn archive(db_path: Utf8PathBuf, args: ArchiveArgs) -> Result<()> {
    let mut db = Database::open(&db_path)?;
    let actor = current_actor();

    let config = find_config(&db, &args.id)?;

    if config.is_current {
        bail!("Cannot archive current configuration. Set a different configuration as current first.");
    }

    db.transaction(|tx| {
        tx.execute(
            "UPDATE configurations SET archived = 1, updated_at = datetime('now') WHERE id = ?1",
            [&config.id],
        )?;

        EventLog::record(
            tx,
            EventType::Archived,
            EntityType::Configuration,
            &config.id,
            serde_json::json!({}),
            &actor,
        )?;

        Ok(())
    })?;

    println!(
        "{} Archived configuration: {}",
        style("✓").green(),
        config.name
    );

    Ok(())
}

fn set_current(db_path: Utf8PathBuf, args: SetCurrentArgs) -> Result<()> {
    let mut db = Database::open(&db_path)?;
    let actor = current_actor();

    let config = find_config(&db, &args.id)?;

    if config.archived {
        bail!("Cannot set archived configuration as current");
    }

    db.transaction(|tx| {
        // Unset current on all configurations
        tx.execute("UPDATE configurations SET is_current = 0", [])?;

        // Set this one as current
        tx.execute(
            "UPDATE configurations SET is_current = 1, updated_at = datetime('now') WHERE id = ?1",
            [&config.id],
        )?;

        EventLog::record(
            tx,
            EventType::ConfigDeployed,
            EntityType::Configuration,
            &config.id,
            serde_json::json!({"name": config.name}),
            &actor,
        )?;

        Ok(())
    })?;

    println!(
        "{} Set current configuration: {}",
        style("✓").green(),
        config.name
    );

    Ok(())
}

/// Find a configuration by ID or name
fn find_config(db: &Database, id_or_name: &str) -> Result<Configuration> {
    // Try by ID first
    if let Ok(config) = db.conn().query_row(
        "SELECT id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at
         FROM configurations WHERE id = ?1",
        [id_or_name],
        Configuration::from_row,
    ) {
        return Ok(config);
    }

    // Try by name
    db.conn()
        .query_row(
            "SELECT id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at
             FROM configurations WHERE name = ?1",
            [id_or_name],
            Configuration::from_row,
        )
        .map_err(|_| anyhow::anyhow!("Configuration '{}' not found", id_or_name))
}

/// Find a configuration within a transaction
fn find_config_tx(tx: &rusqlite::Transaction, id_or_name: &str) -> Result<Configuration> {
    // Try by ID first
    if let Ok(config) = tx.query_row(
        "SELECT id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at
         FROM configurations WHERE id = ?1",
        [id_or_name],
        Configuration::from_row,
    ) {
        return Ok(config);
    }

    // Try by name
    tx.query_row(
        "SELECT id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at
         FROM configurations WHERE name = ?1",
        [id_or_name],
        Configuration::from_row,
    )
    .map_err(|_| anyhow::anyhow!("Configuration '{}' not found", id_or_name))
}

fn print_config_list(configs: &[Configuration]) {
    if configs.is_empty() {
        println!("{}", style("No configurations found").dim());
        return;
    }

    println!(
        "{:<8} {:<30} {:<10} {:>6} {:>10}",
        style("ID").bold(),
        style("NAME").bold(),
        style("DOMAIN").bold(),
        style("ITEMS").bold(),
        style("STATUS").bold()
    );
    println!("{}", style("─".repeat(70)).dim());

    for config in configs {
        let status = if config.is_current {
            style("CURRENT").green().to_string()
        } else if config.archived {
            style("archived").dim().to_string()
        } else {
            "-".to_string()
        };

        println!(
            "{:<8} {:<30} {:<10} {:>6} {:>10}",
            &config.id[..8.min(config.id.len())],
            truncate(&config.name, 29),
            config.domain,
            config.items.len(),
            status
        );
    }
}

fn print_config_detail(config: &Configuration) {
    println!("{}", style(&config.name).bold().cyan());
    println!("{}", style("═".repeat(40)).dim());
    println!();

    println!("ID:       {}", config.id);
    println!("Domain:   {}", config.domain);
    println!(
        "Status:   {}",
        if config.is_current {
            style("CURRENT").green()
        } else {
            style("not deployed").dim()
        }
    );
    println!("Created:  {}", config.created_at.format("%Y-%m-%d %H:%M"));
    println!();

    if config.items.is_empty() {
        println!("{}", style("No items in configuration").dim());
    } else {
        println!("{}", style("Items:").bold());
        let mut total = 0.0;
        for item in &config.items {
            let price_str = item
                .unit_price
                .map(|p| {
                    total += p * item.quantity as f64;
                    format!("${:.2}", p)
                })
                .unwrap_or_else(|| "-".to_string());

            println!(
                "  {}x {} @ {}",
                item.quantity, item.item_id, price_str
            );
            if let Some(ref notes) = item.notes {
                println!("     {}", style(notes).dim());
            }
        }

        if total > 0.0 {
            println!();
            println!("{}: ${:.2}", style("Total Cost").bold(), total);
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
