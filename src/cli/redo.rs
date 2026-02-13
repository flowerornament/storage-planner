//! sp redo -- Redo the last undone action

use anyhow::Result;

use crate::core::db::Database;
use crate::core::events;

use super::OutputFormat;

pub fn run(db: &mut Database, format: OutputFormat) -> Result<()> {
    let summary = events::redo(db)?;
    match format {
        OutputFormat::Text => {
            println!("Redone: {}", summary);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({ "action": "redo", "summary": summary });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }
    Ok(())
}
