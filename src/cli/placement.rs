//! sp placement -- Manage dataset placements on volumes
//!
//! Placeholder for Phase 2 Plan 03 implementation.

use anyhow::Result;
use clap::Subcommand;

use crate::core::db::Database;

use super::OutputFormat;

#[derive(Subcommand)]
pub enum PlacementCommands {
    /// Place a dataset on a volume
    Add {
        /// Dataset name or ID
        dataset: String,

        /// Volume name or ID
        volume: String,

        /// Node to disambiguate volume (if name is shared across nodes)
        #[arg(long)]
        node: Option<String>,

        /// Placement role (primary, replica, backup, archive)
        #[arg(long, default_value = "primary")]
        role: String,

        /// Priority (higher = preferred for reads)
        #[arg(long, default_value_t = 0)]
        priority: i32,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// List placements in the active topology
    List {
        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Remove a dataset placement from a volume
    Remove {
        /// Dataset name or ID
        dataset: String,

        /// Volume name or ID
        volume: String,

        /// Node to disambiguate volume
        #[arg(long)]
        node: Option<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },
}

pub fn run(cmd: PlacementCommands, _db: &mut Database, _format: OutputFormat) -> Result<()> {
    match cmd {
        PlacementCommands::Add { .. } => {
            println!("Placement commands coming in Phase 2 Plan 03.");
        }
        PlacementCommands::List { .. } => {
            println!("Placement commands coming in Phase 2 Plan 03.");
        }
        PlacementCommands::Remove { .. } => {
            println!("Placement commands coming in Phase 2 Plan 03.");
        }
    }
    Ok(())
}
