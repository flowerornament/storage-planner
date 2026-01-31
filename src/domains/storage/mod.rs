//! Storage domain models and analysis
//!
//! Provides storage-specific concepts:
//! - Nodes (compute devices)
//! - Volumes (storage units)
//! - Datasets (logical data groups)
//! - Sync regimes (data movement)

pub mod analysis;
pub mod models;

pub use models::{Dataset, Node, SyncRegime, Volume};
