//! sp sync -- Manage data sync regimes between volumes
//!
//! Placeholder for Phase 2 implementation.

use anyhow::Result;
use clap::Subcommand;

use crate::core::db::Database;

use super::OutputFormat;

#[derive(Subcommand)]
pub enum SyncCommands {
    /// Add a sync regime for a dataset between two volumes
    Add {
        /// Sync regime name (must be unique within topology)
        name: String,

        /// Dataset to sync
        #[arg(long)]
        dataset: String,

        /// Source volume
        #[arg(long)]
        from: String,

        /// Target volume
        #[arg(long)]
        to: String,

        /// Sync type (e.g., rsync, zfs-send, rclone, time-machine)
        #[arg(long, name = "type")]
        sync_type: String,

        /// Sync schedule (cron expression)
        #[arg(long)]
        schedule: Option<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// List sync regimes in the active topology
    List {
        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Show details of a specific sync regime
    Show {
        /// Sync regime name or ID
        name: String,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Remove a sync regime
    Remove {
        /// Sync regime name or ID to remove
        name: String,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },
}

pub fn run(cmd: SyncCommands, _db: &mut Database, _format: OutputFormat) -> Result<()> {
    match cmd {
        SyncCommands::Add { .. } => {
            println!("Sync commands coming in Phase 2.");
        }
        SyncCommands::List { .. } => {
            println!("Sync commands coming in Phase 2.");
        }
        SyncCommands::Show { .. } => {
            println!("Sync commands coming in Phase 2.");
        }
        SyncCommands::Remove { .. } => {
            println!("Sync commands coming in Phase 2.");
        }
    }
    Ok(())
}
