//! sp redo -- Redo the last undone action

use anyhow::Result;

use crate::core::db::Database;
use crate::core::events;

pub fn run(db: &mut Database) -> Result<()> {
    let summary = events::redo(db)?;
    println!("Redone: {}", summary);
    Ok(())
}
