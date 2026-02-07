//! sp dataset -- Manage logical datasets with replication requirements
//!
//! Placeholder for Phase 2 implementation.

use anyhow::Result;
use clap::Subcommand;

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
    },

    /// List datasets in the active topology
    List,

    /// Show details of a specific dataset
    Show {
        /// Dataset name
        name: String,
    },

    /// Remove a dataset from the active topology
    Remove {
        /// Dataset name to remove
        name: String,
    },
}

pub fn run(cmd: DatasetCommands) -> Result<()> {
    match cmd {
        DatasetCommands::Add { .. } => {
            println!("Dataset commands coming in Phase 2.");
        }
        DatasetCommands::List => {
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
