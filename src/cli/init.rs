//! sp init -- Initialize a new storage planner database

use anyhow::Result;
use std::path::Path;

use crate::core::db::Database;

pub fn run(db_path: &Path) -> Result<()> {
    if db_path.exists() {
        println!("Database already exists at {}", db_path.display());
        return Ok(());
    }

    Database::open(db_path)?;
    println!(
        "Initialized storage planner database at {}",
        db_path.display()
    );
    Ok(())
}
