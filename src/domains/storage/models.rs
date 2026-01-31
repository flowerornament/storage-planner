//! Storage domain models

use serde::{Deserialize, Serialize};

/// A compute device that can host storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub node_type: NodeType,
    pub location: String,
    pub volumes: Vec<String>, // Volume IDs
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Desktop,
    Nas,
    Server,
    Cloud,
    External,
}

/// A storage unit attached to a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub id: String,
    pub name: String,
    pub node_id: String,
    pub item_id: Option<String>, // Reference to catalog item
    pub capacity_bytes: u64,
    pub raid_level: Option<String>,
    pub datasets: Vec<String>, // Dataset IDs
}

/// A logical data group with requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dataset {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub growth_rate: Option<f64>, // Bytes per month
    pub criticality: Criticality,
    pub rpo_hours: Option<u32>, // Recovery Point Objective
    pub rto_hours: Option<u32>, // Recovery Time Objective
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Criticality {
    Critical,
    Important,
    Normal,
    Archive,
}

/// Data movement definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRegime {
    pub id: String,
    pub name: String,
    pub source_volume: String,
    pub target_volume: String,
    pub sync_type: SyncType,
    pub schedule: Option<String>, // Cron expression
    pub datasets: Vec<String>,    // Which datasets this sync covers
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SyncType {
    Rsync,
    Rclone,
    Zfs,
    TimeMachine,
    Manual,
}
