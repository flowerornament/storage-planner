//! sp dataset -- Manage logical datasets with replication requirements
//!
//! Placeholder for Phase 2 implementation.

use anyhow::Result;
use clap::Subcommand;

use crate::core::db::Database;

use super::OutputFormat;

#[derive(Subcommand)]
pub enum DatasetCommands {
    /// Add a dataset to the active topology
    Add {
        /// Dataset name (must be unique within topology)
        name: String,

        /// Current size (e.g., "500GB", "2TB")
        #[arg(long)]
        size: String,

        /// Criticality level (normal, important, critical)
        #[arg(long, default_value = "normal")]
        criticality: String,

        /// Minimum number of copies required
        #[arg(long, default_value = "1")]
        min_copies: i32,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// List datasets in the active topology
    List {
        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Show details of a specific dataset
    Show {
        /// Dataset name or ID
        name: String,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Remove a dataset from the active topology
    Remove {
        /// Dataset name or ID to remove
        name: String,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },
}

pub fn run(cmd: DatasetCommands, _db: &mut Database, _format: OutputFormat) -> Result<()> {
    match cmd {
        DatasetCommands::Add { .. } => {
            println!("Dataset commands coming in Phase 2.");
        }
        DatasetCommands::List { .. } => {
            println!("Dataset commands coming in Phase 2.");
        }
        DatasetCommands::Show { .. } => {
            println!("Dataset commands coming in Phase 2.");
        }
        DatasetCommands::Remove { .. } => {
            println!("Dataset commands coming in Phase 2.");
        }
    }
    Ok(())
}
