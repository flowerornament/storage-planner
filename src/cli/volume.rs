//! sp volume -- Manage storage volumes attached to nodes
//!
//! Placeholder for Phase 2 implementation.

use anyhow::Result;
use clap::Subcommand;

use crate::core::db::Database;

use super::OutputFormat;

#[derive(Subcommand)]
pub enum VolumeCommands {
    /// Add a storage volume to a node
    Add {
        /// Volume name (must be unique within node)
        name: String,

        /// Node to attach this volume to
        #[arg(long)]
        node: String,

        /// Total capacity (e.g., "4TB", "500GB")
        #[arg(long)]
        capacity: String,

        /// Filesystem type (e.g., apfs, ext4, zfs, btrfs)
        #[arg(long)]
        filesystem: Option<String>,

        /// RAID level if applicable (e.g., raid1, raid5, raidz2)
        #[arg(long)]
        raid: Option<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// List volumes in the active topology
    List {
        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Show details of a specific volume
    Show {
        /// Volume name or ID
        name: String,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Remove a volume from a node
    Remove {
        /// Volume name or ID to remove
        name: String,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },
}

pub fn run(cmd: VolumeCommands, _db: &mut Database, _format: OutputFormat) -> Result<()> {
    match cmd {
        VolumeCommands::Add { .. } => {
            println!("Volume commands coming in Phase 2.");
        }
        VolumeCommands::List { .. } => {
            println!("Volume commands coming in Phase 2.");
        }
        VolumeCommands::Show { .. } => {
            println!("Volume commands coming in Phase 2.");
        }
        VolumeCommands::Remove { .. } => {
            println!("Volume commands coming in Phase 2.");
        }
    }
    Ok(())
}
