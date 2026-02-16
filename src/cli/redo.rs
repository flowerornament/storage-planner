//! sp redo -- Redo the last undone action

use anyhow::Result;

use crate::core::db::Database;
use crate::core::events;

use super::OutputFormat;

pub fn run(db: &mut Database, format: OutputFormat, skip: bool) -> Result<()> {
    let summary = events::redo(db, skip)?;
    match format {
        OutputFormat::Text => {
            if skip {
                println!("Skipped: {}", summary);
            } else {
                println!("Redone: {}", summary);
            }
        }
        OutputFormat::Json => {
            let action = if skip { "skip_redo" } else { "redo" };
            let json = serde_json::json!({ "action": action, "summary": summary });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }
    Ok(())
}
