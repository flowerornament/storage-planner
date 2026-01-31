//! sp events - View event audit log

use anyhow::{bail, Result};
use camino::Utf8PathBuf;
use clap::Args;
use console::style;

use crate::core::db::Database;
use crate::core::events::EventLog;
use crate::core::models::{EntityType, Event};

use super::OutputFormat;

#[derive(Args)]
pub struct EventsArgs {
    /// Filter by entity type (item, price, configuration, decision)
    #[arg(long, short = 't')]
    pub entity_type: Option<String>,

    /// Filter by entity ID
    #[arg(long, short = 'e')]
    pub entity: Option<String>,

    /// Maximum number of events to show
    #[arg(long, short = 'n', default_value = "20")]
    pub limit: usize,
}

pub fn run(db_path: Utf8PathBuf, args: EventsArgs, format: OutputFormat) -> Result<()> {
    if !db_path.exists() {
        bail!("Database not found at {}. Run `sp init` first.", db_path);
    }

    let db = Database::open(&db_path)?;

    let events: Vec<Event> = if let (Some(entity_type), Some(entity_id)) =
        (&args.entity_type, &args.entity)
    {
        let et = EntityType::from_str(entity_type);
        EventLog::for_entity(db.conn(), et, entity_id)?
            .into_iter()
            .take(args.limit)
            .collect()
    } else if let Some(entity_id) = &args.entity {
        // Search across all entity types
        let mut all_events = Vec::new();
        for et in [
            EntityType::Item,
            EntityType::Price,
            EntityType::Configuration,
            EntityType::Decision,
        ] {
            all_events.extend(EventLog::for_entity(db.conn(), et, entity_id)?);
        }
        all_events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        all_events.into_iter().take(args.limit).collect()
    } else {
        EventLog::recent(db.conn(), args.limit)?
    };

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&events)?);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(&events)?);
        }
        OutputFormat::Text => {
            print_events(&events);
        }
    }

    Ok(())
}

fn print_events(events: &[Event]) {
    if events.is_empty() {
        println!("{}", style("No events found").dim());
        return;
    }

    println!(
        "{:<20} {:<15} {:<15} {:<25} {}",
        style("TIMESTAMP").bold(),
        style("TYPE").bold(),
        style("ENTITY").bold(),
        style("ID").bold(),
        style("ACTOR").bold()
    );
    println!("{}", style("─".repeat(90)).dim());

    for event in events {
        let timestamp = event.timestamp.format("%Y-%m-%d %H:%M:%S");
        println!(
            "{:<20} {:<15} {:<15} {:<25} {}",
            style(timestamp).dim(),
            event.event_type.as_str(),
            event.entity_type.as_str(),
            truncate(&event.entity_id, 24),
            style(&event.actor).dim()
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
