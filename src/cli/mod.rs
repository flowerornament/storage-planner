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
//!   sp analyze ...       - Run analysis reports (Phase 4)
//!   sp decision ...      - Decision lifecycle management (Phase 5)
//!   sp diagram           - ASCII topology visualization (Phase 6)
//!   sp export            - YAML topology export (Phase 6)
//!   sp import            - YAML topology import (Phase 6)
//!   sp status            - System health overview (Phase 6)
//!   sp prime             - AI agent bootstrap document (Phase 6)
//!   sp current           - Show/set current topology (Phase 6)
//!   sp undo              - Undo last action
//!   sp redo              - Redo last undone action

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

use crate::core::db::Database;

mod analyze;
mod catalog;
mod dataset;
mod decision;
mod diagram;
pub(crate) mod export;
mod init;
mod link;
mod node;
mod placement;
mod prime;
mod redo;
mod status;
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

    /// Manage product catalog and price observations
    #[command(subcommand)]
    Catalog(catalog::CatalogCommands),

    /// Track purchase decisions with lifecycle management
    #[command(subcommand)]
    Decision(decision::DecisionCommands),

    /// Run analysis reports against topology data
    #[command(subcommand)]
    Analyze(analyze::AnalyzeCommands),

    /// Show ASCII diagram of topology structure
    Diagram {
        /// Target topology (defaults to current)
        #[arg(long)]
        topology: Option<String>,
        /// Show node-volume-dataset hierarchy
        #[arg(long)]
        tree: bool,
        /// Show network link topology between nodes
        #[arg(long)]
        network: bool,
    },

    /// Export topology to YAML file
    Export {
        /// Topology name or ID prefix
        topology: String,
        /// Export as template (strip all IDs for reuse)
        #[arg(long)]
        template: bool,
        /// Only export specific entity types (comma-separated: nodes,volumes,datasets,placements,links,sync_regimes)
        #[arg(long)]
        only: Option<String>,
        /// Write to file instead of stdout
        #[arg(long, short)]
        output: Option<PathBuf>,
    },

    /// Import topology from YAML file
    Import {
        /// Path to YAML file
        file: PathBuf,
        /// Name for the imported topology (auto-detected from YAML if omitted)
        #[arg(long)]
        name: Option<String>,
    },

    /// Show system health overview -- problems, topology, decisions, catalog, activity
    Status,

    /// Output AI agent bootstrap document with workflow guide and dynamic state
    Prime,

    /// Show or set the current topology
    Current {
        /// Topology name or ID to set as current (omit to show current)
        topology: Option<String>,
    },

    /// Undo the last action
    Undo {
        /// Skip the current undo event (move pointer without reversing)
        #[arg(long)]
        skip: bool,
    },

    /// Redo the last undone action
    Redo {
        /// Skip the current redo event (move pointer without re-applying)
        #[arg(long)]
        skip: bool,
    },
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
            Commands::Catalog(cmd) => {
                let mut db = open_db(&db_path)?;
                catalog::run(cmd, &mut db, format)
            }
            Commands::Decision(cmd) => {
                let mut db = open_db(&db_path)?;
                decision::run(cmd, &mut db, format)
            }
            Commands::Analyze(cmd) => {
                let mut db = open_db(&db_path)?;
                analyze::run(cmd, &mut db, format)
            }
            Commands::Diagram {
                topology,
                tree,
                network,
            } => {
                let mut db = open_db(&db_path)?;
                diagram::run(&mut db, topology.as_deref(), tree, network)
            }
            Commands::Export {
                topology,
                template,
                only,
                output,
            } => {
                let mut db = open_db(&db_path)?;
                export::run_export(
                    &mut db,
                    &topology,
                    template,
                    only.as_deref(),
                    output.as_ref(),
                )
            }
            Commands::Import { file, name } => {
                let mut db = open_db(&db_path)?;
                export::run_import(&mut db, &file, name.as_deref())
            }
            Commands::Status => {
                let mut db = open_db(&db_path)?;
                status::run_status(&mut db, format)
            }
            Commands::Prime => {
                let mut db = open_db(&db_path)?;
                prime::run(&mut db)
            }
            Commands::Current { topology } => {
                let mut db = open_db(&db_path)?;
                status::run_current(&mut db, topology.as_deref(), format)
            }
            Commands::Undo { skip } => {
                let mut db = open_db(&db_path)?;
                undo::run(&mut db, format, skip)
            }
            Commands::Redo { skip } => {
                let mut db = open_db(&db_path)?;
                redo::run(&mut db, format, skip)
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
