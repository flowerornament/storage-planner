//! sp init - Initialize database

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use clap::Args;
use console::style;

use crate::core::db::Database;

#[derive(Args)]
pub struct InitArgs {
    /// Force re-initialization (drops existing data)
    #[arg(long)]
    pub force: bool,
}

pub fn run(db_path: Utf8PathBuf, args: InitArgs) -> Result<()> {
    // Check if database already exists
    if db_path.exists() && !args.force {
        println!(
            "{} Database already exists at {}",
            style("!").yellow(),
            db_path
        );
        println!("  Use --force to reinitialize (this will drop all data)");
        return Ok(());
    }

    // Remove existing database if force
    if args.force && db_path.exists() {
        std::fs::remove_file(&db_path)
            .with_context(|| format!("Failed to remove existing database: {db_path}"))?;
        println!("{} Removed existing database", style("✓").green());
    }

    // Create and initialize database
    let mut db = Database::open(&db_path)?;
    db.migrate()?;

    println!("{} Initialized database at {}", style("✓").green(), db_path);
    println!();
    println!("Next steps:");
    println!("  sp item add <id> --name=... --category=...   # Add items to catalog");
    println!("  sp prime                                      # Get context for agents");
    println!("  sp doctor                                     # Check database health");

    Ok(())
}
