//! CLI command implementations
//!
//! All user interactions go through these commands.
//! Commands enforce workflow and invariants - agents can't bypass them.
//!
//! Stub module - full CLI implementation comes in Plan 02.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Storage Planner - Purchase decision support tool
///
/// A CLI for evaluating storage (and other purchase) decisions.
/// All mutations go through this CLI; the database is the source of truth.
#[derive(Parser)]
#[command(name = "sp", version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Database directory (default: .sp in current directory)
    #[arg(long, short = 'd', global = true, env = "SP_DIR")]
    pub dir: Option<PathBuf>,

    /// Output format for commands that support it
    #[arg(long, global = true, default_value = "text")]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new database
    Init,
}

/// Output format for commands
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

impl Cli {
    /// Get the database directory path
    pub fn db_dir(&self) -> PathBuf {
        self.dir.clone().unwrap_or_else(|| PathBuf::from(".sp"))
    }

    /// Get the database file path
    pub fn db_path(&self) -> PathBuf {
        self.db_dir().join("decisions.db")
    }

    /// Run the CLI command
    pub fn run(self) -> Result<()> {
        match self.command {
            Commands::Init => {
                let db_path = self.db_path();
                let mut db = crate::core::db::Database::open(&db_path)?;
                db.migrate()?;
                println!("Initialized database at {}", db_path.display());
                Ok(())
            }
        }
    }
}
