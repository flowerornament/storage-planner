//! sp undo -- Undo the last action

use anyhow::Result;

use crate::core::db::Database;
use crate::core::events;

pub fn run(db: &mut Database) -> Result<()> {
    let summary = events::undo(db)?;
    println!("Undone: {}", summary);
    Ok(())
}
