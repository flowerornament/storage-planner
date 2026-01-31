//! sp decide - Manage decision sessions

use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use chrono::Utc;
use clap::{Args, Subcommand};
use console::style;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::events::{current_actor, EventLog};
use crate::core::models::{Configuration, Decision, DecisionStatus, EntityType, EventType};

use super::OutputFormat;

#[derive(Subcommand)]
#[command(after_help = r#"EXAMPLES:
    # Start a decision session
    sp decide create --purpose="Storage upgrade for home office"

    # Add options (each maps to a configuration)
    sp decide add-option sata --config="SATA Setup"
    sp decide add-option nvme --config="NVMe Setup"

    # Compare options side-by-side
    sp decide compare

    # Make and document the decision
    sp decide choose sata --rationale="Best value per TB, sufficient speed"

    # Deploy the chosen configuration
    sp decide deploy

    # View decision history
    sp decide history
    sp decide show <decision-id>

    # Abandon if requirements change
    sp decide abandon --reason="Budget reduced"
"#)]
pub enum DecideCommands {
    /// Create a new decision session
    Create(CreateArgs),

    /// Add an option to the active decision
    AddOption(AddOptionArgs),

    /// Compare all options in the active decision
    Compare(CompareArgs),

    /// Choose an option and record rationale
    Choose(ChooseArgs),

    /// Deploy the chosen option as current configuration
    Deploy(DeployArgs),

    /// View decision history
    History(HistoryArgs),

    /// Show details of a specific decision
    Show(ShowArgs),

    /// Abandon the active decision
    Abandon(AbandonArgs),
}

#[derive(Args)]
pub struct CreateArgs {
    /// Purpose of this decision
    #[arg(long)]
    pub purpose: String,
}

#[derive(Args)]
pub struct AddOptionArgs {
    /// Option name (e.g., "sata", "nvme", "option-a")
    pub name: String,

    /// Configuration ID or name to use for this option
    #[arg(long)]
    pub config: String,
}

#[derive(Args)]
pub struct CompareArgs {}

#[derive(Args)]
pub struct ChooseArgs {
    /// Option name to choose
    pub option: String,

    /// Rationale for the decision
    #[arg(long)]
    pub rationale: String,
}

#[derive(Args)]
pub struct DeployArgs {}

#[derive(Args)]
pub struct HistoryArgs {
    /// Maximum decisions to show
    #[arg(long, short = 'n', default_value = "10")]
    pub limit: usize,
}

#[derive(Args)]
pub struct ShowArgs {
    /// Decision ID
    pub id: String,
}

#[derive(Args)]
pub struct AbandonArgs {
    /// Reason for abandoning
    #[arg(long)]
    pub reason: Option<String>,
}

pub fn run(db_path: Utf8PathBuf, cmd: DecideCommands, format: OutputFormat) -> Result<()> {
    if !db_path.exists() {
        bail!("Database not found at {}. Run `sp init` first.", db_path);
    }

    match cmd {
        DecideCommands::Create(args) => create(db_path, args, format),
        DecideCommands::AddOption(args) => add_option(db_path, args),
        DecideCommands::Compare(args) => compare(db_path, args, format),
        DecideCommands::Choose(args) => choose(db_path, args),
        DecideCommands::Deploy(args) => deploy(db_path, args),
        DecideCommands::History(args) => history(db_path, args, format),
        DecideCommands::Show(args) => show(db_path, args, format),
        DecideCommands::Abandon(args) => abandon(db_path, args),
    }
}

fn create(db_path: Utf8PathBuf, args: CreateArgs, format: OutputFormat) -> Result<()> {
    let mut db = Database::open(&db_path)?;
    let actor = current_actor();

    // Check for existing active decision
    let existing: Option<Decision> = db
        .conn()
        .query_row(
            "SELECT id, purpose, status, options, chosen_option, chosen_config_id, rationale, decided_at, decided_by, created_at, metadata
             FROM decisions WHERE status = 'active' LIMIT 1",
            [],
            Decision::from_row,
        )
        .ok();

    if let Some(existing) = existing {
        bail!(
            "Active decision already exists: '{}' ({}). Abandon it first with `sp decide abandon`.",
            existing.purpose,
            &existing.id[..8]
        );
    }

    let decision = Decision::new(&args.purpose);

    db.transaction(|tx| {
        decision.insert(tx)?;

        EventLog::record(
            tx,
            EventType::Created,
            EntityType::Decision,
            &decision.id,
            serde_json::json!({"purpose": args.purpose}),
            &actor,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&decision)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&decision)?),
        OutputFormat::Text => {
            println!(
                "{} Created decision session: {}",
                style("✓").green(),
                args.purpose
            );
            println!("  ID: {}", &decision.id[..8]);
            println!();
            println!("Next steps:");
            println!("  sp decide add-option <name> --config=<config>");
            println!("  sp decide compare");
            println!("  sp decide choose <option> --rationale=\"...\"");
        }
    }

    Ok(())
}

fn add_option(db_path: Utf8PathBuf, args: AddOptionArgs) -> Result<()> {
    let mut db = Database::open(&db_path)?;
    let actor = current_actor();

    // Get active decision
    let mut decision = get_active_decision(&db)?;

    // Find the configuration
    let config: Configuration = find_config(&db, &args.config)?;

    // Add option
    decision
        .options
        .insert(args.name.clone(), config.id.clone());

    db.transaction(|tx| {
        tx.execute(
            "UPDATE decisions SET options = ?1 WHERE id = ?2",
            params![serde_json::to_string(&decision.options)?, decision.id],
        )?;

        EventLog::record(
            tx,
            EventType::Updated,
            EntityType::Decision,
            &decision.id,
            serde_json::json!({
                "action": "add_option",
                "option_name": args.name,
                "config_id": config.id,
            }),
            &actor,
        )?;

        Ok(())
    })?;

    println!(
        "{} Added option '{}' -> configuration '{}'",
        style("✓").green(),
        args.name,
        config.name
    );

    Ok(())
}

fn compare(_db_path: Utf8PathBuf, _args: CompareArgs, format: OutputFormat) -> Result<()> {
    let db = Database::open(&_db_path)?;

    let decision = get_active_decision(&db)?;

    if decision.options.is_empty() {
        bail!("No options to compare. Add options with `sp decide add-option`.");
    }

    // Collect option details
    #[derive(serde::Serialize)]
    struct OptionDetail {
        name: String,
        config_id: String,
        config_name: String,
        item_count: usize,
        total_cost: f64,
    }

    let mut options: Vec<OptionDetail> = Vec::new();

    for (name, config_id) in &decision.options {
        let config: Configuration = db
            .conn()
            .query_row(
                "SELECT id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at
                 FROM configurations WHERE id = ?1",
                [config_id],
                Configuration::from_row,
            )
            .map_err(|_| anyhow::anyhow!("Configuration '{}' not found", config_id))?;

        let item_count = config.items.len();
        let total_cost = config.total_cost();

        options.push(OptionDetail {
            name: name.clone(),
            config_id: config_id.clone(),
            config_name: config.name,
            item_count,
            total_cost,
        });
    }

    match format {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "decision_id": decision.id,
                "purpose": decision.purpose,
                "options": options,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Yaml => {
            let output = serde_json::json!({
                "decision_id": decision.id,
                "purpose": decision.purpose,
                "options": options,
            });
            println!("{}", serde_yaml::to_string(&output)?);
        }
        OutputFormat::Text => {
            println!("{}", style("Decision: ").bold().cyan());
            println!("{}", decision.purpose);
            println!("{}", style("─".repeat(60)).dim());
            println!();

            println!(
                "{:<15} {:<25} {:>6} {:>12}",
                style("OPTION").bold(),
                style("CONFIGURATION").bold(),
                style("ITEMS").bold(),
                style("COST").bold()
            );
            println!("{}", style("─".repeat(60)).dim());

            for opt in &options {
                println!(
                    "{:<15} {:<25} {:>6} {:>12}",
                    opt.name,
                    truncate(&opt.config_name, 24),
                    opt.item_count,
                    if opt.total_cost > 0.0 {
                        format!("${:.2}", opt.total_cost)
                    } else {
                        "-".to_string()
                    }
                );
            }
        }
    }

    Ok(())
}

fn choose(db_path: Utf8PathBuf, args: ChooseArgs) -> Result<()> {
    let mut db = Database::open(&db_path)?;
    let actor = current_actor();

    let mut decision = get_active_decision(&db)?;

    // Verify option exists
    let config_id = decision
        .options
        .get(&args.option)
        .ok_or_else(|| anyhow::anyhow!("Option '{}' not found in decision", args.option))?
        .clone();

    decision.chosen_option = Some(args.option.clone());
    decision.chosen_config_id = Some(config_id.clone());
    decision.rationale = Some(args.rationale.clone());
    decision.decided_at = Some(Utc::now());
    decision.decided_by = Some(actor.clone());
    decision.status = DecisionStatus::Decided;

    db.transaction(|tx| {
        tx.execute(
            "UPDATE decisions SET status = ?1, chosen_option = ?2, chosen_config_id = ?3, rationale = ?4, decided_at = ?5, decided_by = ?6
             WHERE id = ?7",
            params![
                decision.status.as_str(),
                decision.chosen_option,
                decision.chosen_config_id,
                decision.rationale,
                decision.decided_at.map(|dt| dt.to_rfc3339()),
                decision.decided_by,
                decision.id,
            ],
        )?;

        EventLog::record(
            tx,
            EventType::DecisionMade,
            EntityType::Decision,
            &decision.id,
            serde_json::json!({
                "chosen_option": args.option,
                "config_id": config_id,
                "rationale": args.rationale,
            }),
            &actor,
        )?;

        Ok(())
    })?;

    println!(
        "{} Decision made: chose '{}'",
        style("✓").green(),
        args.option
    );
    println!("  Rationale: {}", args.rationale);
    println!();
    println!("Next: run `sp decide deploy` to set as current configuration");

    Ok(())
}

fn deploy(_db_path: Utf8PathBuf, _args: DeployArgs) -> Result<()> {
    let mut db = Database::open(&_db_path)?;
    let actor = current_actor();

    // Get the most recent decided decision
    let decision: Decision = db
        .conn()
        .query_row(
            "SELECT id, purpose, status, options, chosen_option, chosen_config_id, rationale, decided_at, decided_by, created_at, metadata
             FROM decisions WHERE status = 'decided' ORDER BY decided_at DESC LIMIT 1",
            [],
            Decision::from_row,
        )
        .map_err(|_| anyhow::anyhow!("No decided decision to deploy. Run `sp decide choose` first."))?;

    let config_id = decision
        .chosen_config_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Decision has no chosen configuration"))?;

    db.transaction(|tx| {
        // Unset current on all configurations
        tx.execute("UPDATE configurations SET is_current = 0", [])?;

        // Set chosen configuration as current
        tx.execute(
            "UPDATE configurations SET is_current = 1, updated_at = datetime('now') WHERE id = ?1",
            [config_id],
        )?;

        EventLog::record(
            tx,
            EventType::ConfigDeployed,
            EntityType::Configuration,
            config_id,
            serde_json::json!({
                "decision_id": decision.id,
                "chosen_option": decision.chosen_option,
            }),
            &actor,
        )?;

        Ok(())
    })?;

    println!(
        "{} Deployed configuration from decision: {}",
        style("✓").green(),
        decision.chosen_option.unwrap_or_default()
    );

    Ok(())
}

fn history(db_path: Utf8PathBuf, args: HistoryArgs, format: OutputFormat) -> Result<()> {
    let db = Database::open(&db_path)?;

    let mut stmt = db.conn().prepare(
        "SELECT id, purpose, status, options, chosen_option, chosen_config_id, rationale, decided_at, decided_by, created_at, metadata
         FROM decisions ORDER BY created_at DESC LIMIT ?1",
    )?;

    let decisions: Vec<Decision> = stmt
        .query_map([args.limit], Decision::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&decisions)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&decisions)?),
        OutputFormat::Text => print_decision_list(&decisions),
    }

    Ok(())
}

fn show(db_path: Utf8PathBuf, args: ShowArgs, format: OutputFormat) -> Result<()> {
    let db = Database::open(&db_path)?;

    let decision: Decision = db
        .conn()
        .query_row(
            "SELECT id, purpose, status, options, chosen_option, chosen_config_id, rationale, decided_at, decided_by, created_at, metadata
             FROM decisions WHERE id LIKE ?1 || '%'",
            [&args.id],
            Decision::from_row,
        )
        .map_err(|_| anyhow::anyhow!("Decision '{}' not found", args.id))?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&decision)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&decision)?),
        OutputFormat::Text => print_decision_detail(&decision),
    }

    Ok(())
}

fn abandon(db_path: Utf8PathBuf, args: AbandonArgs) -> Result<()> {
    let mut db = Database::open(&db_path)?;
    let actor = current_actor();

    let decision = get_active_decision(&db)?;

    db.transaction(|tx| {
        tx.execute(
            "UPDATE decisions SET status = 'abandoned' WHERE id = ?1",
            [&decision.id],
        )?;

        EventLog::record(
            tx,
            EventType::Updated,
            EntityType::Decision,
            &decision.id,
            serde_json::json!({
                "action": "abandoned",
                "reason": args.reason,
            }),
            &actor,
        )?;

        Ok(())
    })?;

    println!(
        "{} Abandoned decision: {}",
        style("✓").green(),
        decision.purpose
    );

    Ok(())
}

fn get_active_decision(db: &Database) -> Result<Decision> {
    db.conn()
        .query_row(
            "SELECT id, purpose, status, options, chosen_option, chosen_config_id, rationale, decided_at, decided_by, created_at, metadata
             FROM decisions WHERE status = 'active' LIMIT 1",
            [],
            Decision::from_row,
        )
        .map_err(|_| anyhow::anyhow!("No active decision. Create one with `sp decide create`"))
}

fn find_config(db: &Database, id_or_name: &str) -> Result<Configuration> {
    if let Ok(config) = db.conn().query_row(
        "SELECT id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at
         FROM configurations WHERE id = ?1",
        [id_or_name],
        Configuration::from_row,
    ) {
        return Ok(config);
    }

    db.conn()
        .query_row(
            "SELECT id, name, domain, items, domain_data, metadata, is_current, archived, created_at, updated_at
             FROM configurations WHERE name = ?1",
            [id_or_name],
            Configuration::from_row,
        )
        .map_err(|_| anyhow::anyhow!("Configuration '{}' not found", id_or_name))
}

fn print_decision_list(decisions: &[Decision]) {
    if decisions.is_empty() {
        println!("{}", style("No decisions found").dim());
        return;
    }

    println!(
        "{:<8} {:<12} {:<35} {}",
        style("ID").bold(),
        style("STATUS").bold(),
        style("PURPOSE").bold(),
        style("CHOSEN").bold()
    );
    println!("{}", style("─".repeat(70)).dim());

    for decision in decisions {
        let status_style = match decision.status {
            DecisionStatus::Active => style(decision.status.as_str()).yellow(),
            DecisionStatus::Decided => style(decision.status.as_str()).green(),
            DecisionStatus::Abandoned => style(decision.status.as_str()).dim(),
        };

        println!(
            "{:<8} {:<12} {:<35} {}",
            &decision.id[..8.min(decision.id.len())],
            status_style,
            truncate(&decision.purpose, 34),
            decision.chosen_option.as_deref().unwrap_or("-")
        );
    }
}

fn print_decision_detail(decision: &Decision) {
    println!("{}", style(&decision.purpose).bold().cyan());
    println!("{}", style("═".repeat(40)).dim());
    println!();

    println!("ID:      {}", decision.id);
    println!(
        "Status:  {}",
        match decision.status {
            DecisionStatus::Active => style(decision.status.as_str()).yellow(),
            DecisionStatus::Decided => style(decision.status.as_str()).green(),
            DecisionStatus::Abandoned => style(decision.status.as_str()).dim(),
        }
    );
    println!("Created: {}", decision.created_at.format("%Y-%m-%d %H:%M"));
    println!();

    if !decision.options.is_empty() {
        println!("{}", style("Options:").bold());
        for (name, config_id) in &decision.options {
            let marker = if decision.chosen_option.as_ref() == Some(name) {
                style("→ ").green()
            } else {
                style("  ").dim()
            };
            println!("{}{} ({})", marker, name, &config_id[..8]);
        }
        println!();
    }

    if let Some(ref chosen) = decision.chosen_option {
        println!("{}: {}", style("Chosen").bold().green(), chosen);
    }

    if let Some(ref rationale) = decision.rationale {
        println!("{}: {}", style("Rationale").bold(), rationale);
    }

    if let Some(ref decided_at) = decision.decided_at {
        println!(
            "Decided: {} by {}",
            decided_at.format("%Y-%m-%d %H:%M"),
            decision.decided_by.as_deref().unwrap_or("unknown")
        );
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
