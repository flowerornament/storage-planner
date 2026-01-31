//! Storage Planner Library
//!
//! Core abstractions for purchase decision support.
//! Domain-agnostic models with pluggable domain-specific modules.

pub mod core;
pub mod domains;
pub mod pricing;

pub use core::db::Database;
pub use core::events::EventLog;
pub use core::models::{Configuration, Event, Item, Price};
