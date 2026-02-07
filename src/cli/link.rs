//! sp link -- Manage network links between nodes
//!
//! Placeholder for Phase 2 implementation.

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum LinkCommands {
    /// Add a network link between two nodes
    Add {
        /// Source node name
        #[arg(long)]
        from: String,

        /// Target node name
        #[arg(long)]
        to: String,

        /// Connection type (e.g., lan, wan, usb, thunderbolt)
        #[arg(long, name = "type")]
        connection_type: String,

        /// Bandwidth (e.g., "1GB/s", "100MB/s")
        #[arg(long)]
        bandwidth: Option<String>,

        /// Whether the connection is metered
        #[arg(long)]
        metered: bool,
    },

    /// List links in the active topology
    List,

    /// Show details of a specific link
    Show {
        /// Link identifier (source--target)
        name: String,
    },

    /// Remove a link between nodes
    Remove {
        /// Link identifier (source--target)
        name: String,
    },
}

pub fn run(cmd: LinkCommands) -> Result<()> {
    match cmd {
        LinkCommands::Add { .. } => {
            println!("Link commands coming in Phase 2.");
        }
        LinkCommands::List => {
            println!("Link commands coming in Phase 2.");
        }
        LinkCommands::Show { .. } => {
            println!("Link commands coming in Phase 2.");
        }
        LinkCommands::Remove { .. } => {
            println!("Link commands coming in Phase 2.");
        }
    }
    Ok(())
}
