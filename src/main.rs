//! Storage Planner CLI - Purchase decision support tool
//!
//! A bd-style CLI for evaluating storage (and other purchase) decisions.
//! All mutations go through this CLI; YAML exports are read-only snapshots.

use anyhow::Result;
use clap::Parser;

mod cli;
mod core;
mod domains;
mod pricing;

use cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.run()
}
