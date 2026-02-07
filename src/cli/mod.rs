//! CLI command implementations
//!
//! All user interactions go through these commands.
//! Commands enforce workflow and invariants - agents can't bypass them.
//!
//! Command hierarchy:
//!   sp init              - Initialize database
//!   sp topology ...      - Topology CRUD
//!   sp node ...          - Node management (Phase 2)
//!   sp volume ...        - Volume management (Phase 2)
//!   sp dataset ...       - Dataset management (Phase 2)
//!   sp placement ...     - Placement management (Phase 2)
//!   sp link ...          - Link management (Phase 2)
//!   sp sync ...          - Sync regime management (Phase 2)
//!   sp undo              - Undo last action
//!   sp redo              - Redo last undone action

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

use crate::core::db::Database;

mod dataset;
mod init;
mod link;
mod node;
mod placement;
mod redo;
mod sync_regime;
mod topology;
mod undo;
mod volume;

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
    /// Initialize a new storage planner database
    Init,

    /// Manage storage topologies (named configurations)
    #[command(subcommand)]
    Topology(topology::TopologyCommands),

    /// Manage compute nodes within a topology
    #[command(subcommand)]
    Node(node::NodeCommands),

    /// Manage storage volumes attached to nodes
    #[command(subcommand)]
    Volume(volume::VolumeCommands),

    /// Manage logical datasets with replication requirements
    #[command(subcommand)]
    Dataset(dataset::DatasetCommands),

    /// Manage dataset placements on volumes
    #[command(subcommand)]
    Placement(placement::PlacementCommands),

    /// Manage network links between nodes
    #[command(subcommand)]
    Link(link::LinkCommands),

    /// Manage data sync regimes between volumes
    #[command(subcommand)]
    Sync(sync_regime::SyncCommands),

    /// Undo the last action
    Undo,

    /// Redo the last undone action
    Redo,
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
        let db_path = self.db_path();
        let format = self.format;

        match self.command {
            Commands::Init => init::run(&db_path),
            Commands::Topology(cmd) => {
                let mut db = open_db(&db_path)?;
                topology::run(cmd, &mut db, format)
            }
            Commands::Node(cmd) => {
                let mut db = open_db(&db_path)?;
                node::run(cmd, &mut db, format)
            }
            Commands::Volume(cmd) => {
                let mut db = open_db(&db_path)?;
                volume::run(cmd, &mut db, format)
            }
            Commands::Dataset(cmd) => {
                let mut db = open_db(&db_path)?;
                dataset::run(cmd, &mut db, format)
            }
            Commands::Placement(cmd) => {
                let mut db = open_db(&db_path)?;
                placement::run(cmd, &mut db, format)
            }
            Commands::Link(cmd) => {
                let mut db = open_db(&db_path)?;
                link::run(cmd, &mut db, format)
            }
            Commands::Sync(cmd) => {
                let mut db = open_db(&db_path)?;
                sync_regime::run(cmd, &mut db, format)
            }
            Commands::Undo => {
                let mut db = open_db(&db_path)?;
                undo::run(&mut db)
            }
            Commands::Redo => {
                let mut db = open_db(&db_path)?;
                redo::run(&mut db)
            }
        }
    }
}

/// Open an existing database. Fails if the database doesn't exist.
fn open_db(path: &Path) -> Result<Database> {
    if !path.exists() {
        anyhow::bail!(
            "Database not found at {}. Run 'sp init' first.",
            path.display()
        );
    }
    Database::open(path).context("Failed to open database")
}
