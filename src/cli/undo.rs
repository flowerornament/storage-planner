//! sp undo -- Undo the last action

use anyhow::Result;

use crate::core::db::Database;
use crate::core::events;

use super::OutputFormat;

pub fn run(db: &mut Database, format: OutputFormat) -> Result<()> {
    let summary = events::undo(db)?;
    match format {
        OutputFormat::Text => {
            println!("Undone: {}", summary);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({ "action": "undo", "summary": summary });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }
    Ok(())
}
