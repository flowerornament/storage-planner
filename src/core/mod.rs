//! Core abstractions (domain-agnostic)
//!
//! - Database: SQLite connection and transactions
//! - Models: Item, Price, Configuration, Event
//! - Events: Append-only audit log
//! - Specs: Parsing typed attributes (capacity, speed, noise)

pub mod db;
pub mod events;
pub mod models;
pub mod specs;
