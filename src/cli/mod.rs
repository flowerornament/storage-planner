//! CLI command implementations
//!
//! All user interactions go through these commands.
//! Commands enforce workflow and invariants - agents can't bypass them.

mod analyze;
mod config;
mod decide;
mod doctor;
mod events;
mod init;
mod item;
mod price;
mod prime;
mod sync;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

/// Storage Planner - Purchase decision support tool
///
/// A bd-style CLI for evaluating storage (and other purchase) decisions.
/// All mutations go through this CLI; YAML exports are read-only snapshots.
#[derive(Parser)]
#[command(name = "sp", version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Database directory (default: .sp in current directory)
    #[arg(long, short = 'd', global = true, env = "SP_DIR")]
    pub dir: Option<Utf8PathBuf>,

    /// Output format for commands that support it
    #[arg(long, global = true, default_value = "text")]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new database
    Init(init::InitArgs),

    /// Output context for agents (like bd prime)
    Prime(prime::PrimeArgs),

    /// Health check and diagnostics
    Doctor(doctor::DoctorArgs),

    /// Export database to YAML (read-only snapshot)
    Sync(sync::SyncArgs),

    /// View event audit log
    Events(events::EventsArgs),

    /// Manage items in the catalog
    #[command(subcommand)]
    Item(item::ItemCommands),

    /// Manage price observations
    #[command(subcommand)]
    Price(price::PriceCommands),

    /// Manage configurations
    #[command(subcommand)]
    Config(config::ConfigCommands),

    /// Manage decision sessions
    #[command(subcommand)]
    Decide(decide::DecideCommands),

    /// Run analysis on configurations
    Analyze(analyze::AnalyzeArgs),
}

/// Output format for commands
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    Yaml,
}

impl Cli {
    /// Get the database directory path
    pub fn db_dir(&self) -> Utf8PathBuf {
        self.dir
            .clone()
            .unwrap_or_else(|| Utf8PathBuf::from(".sp"))
    }

    /// Get the database file path
    pub fn db_path(&self) -> Utf8PathBuf {
        self.db_dir().join("decisions.db")
    }

    /// Run the CLI command
    pub fn run(self) -> Result<()> {
        let db_path = self.db_path();
        let format = self.format;

        match self.command {
            Commands::Init(args) => init::run(db_path, args),
            Commands::Prime(args) => prime::run(db_path, args, format),
            Commands::Doctor(args) => doctor::run(db_path, args),
            Commands::Sync(args) => sync::run(db_path, args),
            Commands::Events(args) => events::run(db_path, args, format),
            Commands::Item(cmd) => item::run(db_path, cmd, format),
            Commands::Price(cmd) => price::run(db_path, cmd, format),
            Commands::Config(cmd) => config::run(db_path, cmd, format),
            Commands::Decide(cmd) => decide::run(db_path, cmd, format),
            Commands::Analyze(args) => analyze::run(db_path, args, format),
        }
    }
}
