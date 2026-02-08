//! sp prime -- AI agent bootstrap document (CTX-02)
//!
//! Outputs a static instructional document with workflow guide and concrete
//! example commands, followed by a dynamically generated state summary.
//! Designed as the first command an AI agent runs to understand the system.

use anyhow::Result;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::models::Topology;

/// Run the prime command: print agent bootstrap document.
pub fn run_prime(db: &mut Database) -> Result<()> {
    // Placeholder -- will be implemented in Task 2
    println!("sp prime: not yet implemented");
    Ok(())
}
