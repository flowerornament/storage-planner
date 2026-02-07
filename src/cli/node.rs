//! sp node -- Manage compute nodes within a topology
//!
//! Placeholder for Phase 2 implementation.

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum NodeCommands {
    /// Add a compute node to the active topology
    Add {
        /// Node name (must be unique within topology)
        name: String,

        /// Node role (e.g., desktop, nas, server, cloud)
        #[arg(long)]
        role: String,

        /// Physical location (e.g., office, closet, datacenter)
        #[arg(long)]
        location: Option<String>,

        /// Number of available drive bays
        #[arg(long)]
        bays: Option<i32>,
    },

    /// List nodes in the active topology
    List,

    /// Show details of a specific node
    Show {
        /// Node name
        name: String,
    },

    /// Remove a node (and its volumes) from the active topology
    Remove {
        /// Node name to remove
        name: String,
    },
}

pub fn run(cmd: NodeCommands) -> Result<()> {
    match cmd {
        NodeCommands::Add { .. } => {
            println!("Node commands coming in Phase 2.");
        }
        NodeCommands::List => {
            println!("Node commands coming in Phase 2.");
        }
        NodeCommands::Show { .. } => {
            println!("Node commands coming in Phase 2.");
        }
        NodeCommands::Remove { .. } => {
            println!("Node commands coming in Phase 2.");
        }
    }
    Ok(())
}
