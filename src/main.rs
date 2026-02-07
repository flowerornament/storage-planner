//! Storage Planner CLI - Purchase decision support tool
//!
//! A CLI for evaluating storage (and other purchase) decisions.
//! All mutations go through this CLI; the database is the source of truth.

use anyhow::Result;
use clap::Parser;

mod cli;
mod core;
mod domains;

use cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.run()
}
