//! Core abstractions (domain-agnostic)
//!
//! - Database: SQLite connection, migrations, transactions
//! - Models: Topology entity structs (Topology, Node, Volume, Dataset, Placement, Link, SyncRegime)
//! - Events: Undo/redo event system with before/after state
//! - Specs: Parsing typed attributes (capacity, speed, noise)

pub mod db;
pub mod events;
pub mod models;
pub mod resolve;
pub mod specs;
