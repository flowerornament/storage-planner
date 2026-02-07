//! sp sync -- Manage data sync regimes between volumes
//!
//! Placeholder for Phase 2 implementation.

use anyhow::Result;
use clap::Subcommand;

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
    },

    /// List sync regimes in the active topology
    List,

    /// Show details of a specific sync regime
    Show {
        /// Sync regime name
        name: String,
    },

    /// Remove a sync regime
    Remove {
        /// Sync regime name to remove
        name: String,
    },
}

pub fn run(cmd: SyncCommands) -> Result<()> {
    match cmd {
        SyncCommands::Add { .. } => {
            println!("Sync commands coming in Phase 2.");
        }
        SyncCommands::List => {
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
