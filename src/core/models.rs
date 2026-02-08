//! Topology entity model structs
//!
//! All 7 topology entities plus the Event struct.
//! Each struct follows the pattern: new/insert/from_row/to_json.
//!
//! Entities: Topology, Node, Volume, Dataset, Placement, Link, SyncRegime, Event

use chrono::{DateTime, Utc};
use rusqlite::{params, Row, Transaction};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Topology
// ---------------------------------------------------------------------------

/// A named storage topology configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topology {
    pub id: String,
    pub name: String,
    pub description: String,
    pub parent_id: Option<String>,
    pub tag: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Topology {
    /// Create a new topology. Defaults: tag=None, no parent.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            parent_id: None,
            tag: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO topologies (id, name, description, parent_id, tag, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                self.id,
                self.name,
                self.description,
                self.parent_id,
                self.tag,
                self.created_at.to_rfc3339(),
                self.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_str: String = row.get("created_at")?;
        let updated_str: String = row.get("updated_at")?;
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            description: row.get("description")?,
            parent_id: row.get("parent_id")?,
            tag: row.get("tag")?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

// ---------------------------------------------------------------------------
// Node
// ---------------------------------------------------------------------------

/// A compute device that hosts storage volumes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub topology_id: String,
    pub name: String,
    pub role: String,
    pub location: String,
    pub available_bays: Option<i32>,
    pub interface_types: String,
    pub power_draw_watts: Option<f64>,
    pub cost_estimate: Option<f64>,
    pub noise_db: Option<f64>,
    pub rack_units: Option<f64>,
    pub item_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Node {
    /// Create a new node. Defaults: location="", interface_types="".
    pub fn new(
        topology_id: impl Into<String>,
        name: impl Into<String>,
        role: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            topology_id: topology_id.into(),
            name: name.into(),
            role: role.into(),
            location: String::new(),
            available_bays: None,
            interface_types: String::new(),
            power_draw_watts: None,
            cost_estimate: None,
            noise_db: None,
            rack_units: None,
            item_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO nodes (id, topology_id, name, role, location, available_bays, interface_types, power_draw_watts, cost_estimate, noise_db, rack_units, item_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                self.id,
                self.topology_id,
                self.name,
                self.role,
                self.location,
                self.available_bays,
                self.interface_types,
                self.power_draw_watts,
                self.cost_estimate,
                self.noise_db,
                self.rack_units,
                self.item_id,
                self.created_at.to_rfc3339(),
                self.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_str: String = row.get("created_at")?;
        let updated_str: String = row.get("updated_at")?;
        Ok(Self {
            id: row.get("id")?,
            topology_id: row.get("topology_id")?,
            name: row.get("name")?,
            role: row.get("role")?,
            location: row.get("location")?,
            available_bays: row.get("available_bays")?,
            interface_types: row.get("interface_types")?,
            power_draw_watts: row.get("power_draw_watts")?,
            cost_estimate: row.get("cost_estimate")?,
            noise_db: row.get("noise_db")?,
            rack_units: row.get("rack_units")?,
            item_id: row.get("item_id")?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

// ---------------------------------------------------------------------------
// Volume
// ---------------------------------------------------------------------------

/// A storage unit attached to a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub id: String,
    pub topology_id: String,
    pub node_id: String,
    pub name: String,
    pub capacity_bytes: i64,
    pub usable_bytes: Option<i64>,
    pub filesystem: Option<String>,
    pub raid_level: Option<String>,
    pub pool_type: Option<String>,
    pub item_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Volume {
    /// Create a new volume with required fields.
    pub fn new(
        topology_id: impl Into<String>,
        node_id: impl Into<String>,
        name: impl Into<String>,
        capacity_bytes: i64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            topology_id: topology_id.into(),
            node_id: node_id.into(),
            name: name.into(),
            capacity_bytes,
            usable_bytes: None,
            filesystem: None,
            raid_level: None,
            pool_type: None,
            item_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO volumes (id, topology_id, node_id, name, capacity_bytes, usable_bytes, filesystem, raid_level, pool_type, item_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                self.id,
                self.topology_id,
                self.node_id,
                self.name,
                self.capacity_bytes,
                self.usable_bytes,
                self.filesystem,
                self.raid_level,
                self.pool_type,
                self.item_id,
                self.created_at.to_rfc3339(),
                self.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_str: String = row.get("created_at")?;
        let updated_str: String = row.get("updated_at")?;
        Ok(Self {
            id: row.get("id")?,
            topology_id: row.get("topology_id")?,
            node_id: row.get("node_id")?,
            name: row.get("name")?,
            capacity_bytes: row.get("capacity_bytes")?,
            usable_bytes: row.get("usable_bytes")?,
            filesystem: row.get("filesystem")?,
            raid_level: row.get("raid_level")?,
            pool_type: row.get("pool_type")?,
            item_id: row.get("item_id")?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

// ---------------------------------------------------------------------------
// Dataset
// ---------------------------------------------------------------------------

/// A logical data group with retention/replication requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dataset {
    pub id: String,
    pub topology_id: String,
    pub name: String,
    pub size_bytes: i64,
    pub growth_rate_bytes_month: Option<f64>,
    pub criticality: String,
    pub min_copies: i32,
    pub min_locations: i32,
    pub max_rpo_hours: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Dataset {
    /// Create a new dataset. Defaults: criticality="normal", min_copies=1, min_locations=1.
    pub fn new(topology_id: impl Into<String>, name: impl Into<String>, size_bytes: i64) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            topology_id: topology_id.into(),
            name: name.into(),
            size_bytes,
            growth_rate_bytes_month: None,
            criticality: "normal".to_string(),
            min_copies: 1,
            min_locations: 1,
            max_rpo_hours: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO datasets (id, topology_id, name, size_bytes, growth_rate_bytes_month, criticality, min_copies, min_locations, max_rpo_hours, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                self.id,
                self.topology_id,
                self.name,
                self.size_bytes,
                self.growth_rate_bytes_month,
                self.criticality,
                self.min_copies,
                self.min_locations,
                self.max_rpo_hours,
                self.created_at.to_rfc3339(),
                self.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_str: String = row.get("created_at")?;
        let updated_str: String = row.get("updated_at")?;
        Ok(Self {
            id: row.get("id")?,
            topology_id: row.get("topology_id")?,
            name: row.get("name")?,
            size_bytes: row.get("size_bytes")?,
            growth_rate_bytes_month: row.get("growth_rate_bytes_month")?,
            criticality: row.get("criticality")?,
            min_copies: row.get("min_copies")?,
            min_locations: row.get("min_locations")?,
            max_rpo_hours: row.get("max_rpo_hours")?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

// ---------------------------------------------------------------------------
// Placement
// ---------------------------------------------------------------------------

/// Junction table: maps a dataset to a volume with role and priority.
/// Placements are immutable -- delete and recreate to change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Placement {
    pub id: String,
    pub topology_id: String,
    pub dataset_id: String,
    pub volume_id: String,
    pub role: String,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
}

impl Placement {
    /// Create a new placement. Defaults: role="primary", priority=0.
    pub fn new(
        topology_id: impl Into<String>,
        dataset_id: impl Into<String>,
        volume_id: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            topology_id: topology_id.into(),
            dataset_id: dataset_id.into(),
            volume_id: volume_id.into(),
            role: "primary".to_string(),
            priority: 0,
            created_at: Utc::now(),
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO placements (id, topology_id, dataset_id, volume_id, role, priority, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                self.id,
                self.topology_id,
                self.dataset_id,
                self.volume_id,
                self.role,
                self.priority,
                self.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_str: String = row.get("created_at")?;
        Ok(Self {
            id: row.get("id")?,
            topology_id: row.get("topology_id")?,
            dataset_id: row.get("dataset_id")?,
            volume_id: row.get("volume_id")?,
            role: row.get("role")?,
            priority: row.get("priority")?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

// ---------------------------------------------------------------------------
// Link
// ---------------------------------------------------------------------------

/// A network connection between two nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub id: String,
    pub topology_id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub bandwidth_bytes_sec: Option<i64>,
    pub connection_type: String,
    pub latency_ms: Option<f64>,
    pub is_metered: bool,
    pub cost_per_gb_cents: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Link {
    /// Create a new link between two nodes.
    pub fn new(
        topology_id: impl Into<String>,
        source_node_id: impl Into<String>,
        target_node_id: impl Into<String>,
        connection_type: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            topology_id: topology_id.into(),
            source_node_id: source_node_id.into(),
            target_node_id: target_node_id.into(),
            bandwidth_bytes_sec: None,
            connection_type: connection_type.into(),
            latency_ms: None,
            is_metered: false,
            cost_per_gb_cents: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO links (id, topology_id, source_node_id, target_node_id, bandwidth_bytes_sec, connection_type, latency_ms, is_metered, cost_per_gb_cents, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                self.id,
                self.topology_id,
                self.source_node_id,
                self.target_node_id,
                self.bandwidth_bytes_sec,
                self.connection_type,
                self.latency_ms,
                self.is_metered as i32,
                self.cost_per_gb_cents,
                self.created_at.to_rfc3339(),
                self.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_str: String = row.get("created_at")?;
        let updated_str: String = row.get("updated_at")?;
        Ok(Self {
            id: row.get("id")?,
            topology_id: row.get("topology_id")?,
            source_node_id: row.get("source_node_id")?,
            target_node_id: row.get("target_node_id")?,
            bandwidth_bytes_sec: row.get("bandwidth_bytes_sec")?,
            connection_type: row.get("connection_type")?,
            latency_ms: row.get("latency_ms")?,
            is_metered: row.get::<_, i32>("is_metered")? != 0,
            cost_per_gb_cents: row.get("cost_per_gb_cents")?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

// ---------------------------------------------------------------------------
// SyncRegime
// ---------------------------------------------------------------------------

/// A data movement definition between two volumes for a dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRegime {
    pub id: String,
    pub topology_id: String,
    pub name: String,
    pub dataset_id: String,
    pub source_volume_id: String,
    pub target_volume_id: String,
    pub sync_type: String,
    pub schedule: Option<String>,
    pub direction: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SyncRegime {
    /// Create a new sync regime. Defaults: direction="push".
    pub fn new(
        topology_id: impl Into<String>,
        name: impl Into<String>,
        dataset_id: impl Into<String>,
        source_volume_id: impl Into<String>,
        target_volume_id: impl Into<String>,
        sync_type: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            topology_id: topology_id.into(),
            name: name.into(),
            dataset_id: dataset_id.into(),
            source_volume_id: source_volume_id.into(),
            target_volume_id: target_volume_id.into(),
            sync_type: sync_type.into(),
            schedule: None,
            direction: "push".to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO sync_regimes (id, topology_id, name, dataset_id, source_volume_id, target_volume_id, sync_type, schedule, direction, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                self.id,
                self.topology_id,
                self.name,
                self.dataset_id,
                self.source_volume_id,
                self.target_volume_id,
                self.sync_type,
                self.schedule,
                self.direction,
                self.created_at.to_rfc3339(),
                self.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_str: String = row.get("created_at")?;
        let updated_str: String = row.get("updated_at")?;
        Ok(Self {
            id: row.get("id")?,
            topology_id: row.get("topology_id")?,
            name: row.get("name")?,
            dataset_id: row.get("dataset_id")?,
            source_volume_id: row.get("source_volume_id")?,
            target_volume_id: row.get("target_volume_id")?,
            sync_type: row.get("sync_type")?,
            schedule: row.get("schedule")?,
            direction: row.get("direction")?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

// ---------------------------------------------------------------------------
// Decision
// ---------------------------------------------------------------------------

/// A purchase/configuration decision tracking entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub parent_id: Option<String>,
    pub chosen_topology_id: Option<String>,
    pub rationale: Option<String>,
    pub snapshot: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

impl Decision {
    /// Create a new decision with status "draft".
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            description: String::new(),
            status: "draft".to_string(),
            parent_id: None,
            chosen_topology_id: None,
            rationale: None,
            snapshot: None,
            created_at: now,
            updated_at: now,
            closed_at: None,
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO decisions (id, title, description, status, parent_id, chosen_topology_id, rationale, snapshot, created_at, updated_at, closed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                self.id,
                self.title,
                self.description,
                self.status,
                self.parent_id,
                self.chosen_topology_id,
                self.rationale,
                self.snapshot,
                self.created_at.to_rfc3339(),
                self.updated_at.to_rfc3339(),
                self.closed_at.map(|dt| dt.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_str: String = row.get("created_at")?;
        let updated_str: String = row.get("updated_at")?;
        let closed_str: Option<String> = row.get("closed_at")?;
        Ok(Self {
            id: row.get("id")?,
            title: row.get("title")?,
            description: row.get("description")?,
            status: row.get("status")?,
            parent_id: row.get("parent_id")?,
            chosen_topology_id: row.get("chosen_topology_id")?,
            rationale: row.get("rationale")?,
            snapshot: row.get("snapshot")?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            closed_at: closed_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            }),
        })
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

// ---------------------------------------------------------------------------
// DecisionConstraint
// ---------------------------------------------------------------------------

/// A constraint on a decision (e.g., budget, noise, power, rack units)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionConstraint {
    pub id: String,
    pub decision_id: String,
    pub constraint_type: String,
    pub max_value: f64,
    pub created_at: DateTime<Utc>,
}

impl DecisionConstraint {
    /// Create a new decision constraint.
    pub fn new(
        decision_id: impl Into<String>,
        constraint_type: impl Into<String>,
        max_value: f64,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            decision_id: decision_id.into(),
            constraint_type: constraint_type.into(),
            max_value,
            created_at: Utc::now(),
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO decision_constraints (id, decision_id, constraint_type, max_value, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                self.id,
                self.decision_id,
                self.constraint_type,
                self.max_value,
                self.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_str: String = row.get("created_at")?;
        Ok(Self {
            id: row.get("id")?,
            decision_id: row.get("decision_id")?,
            constraint_type: row.get("constraint_type")?,
            max_value: row.get("max_value")?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

// ---------------------------------------------------------------------------
// DecisionTopology
// ---------------------------------------------------------------------------

/// A junction linking a decision to a topology under consideration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionTopology {
    pub id: String,
    pub decision_id: String,
    pub topology_id: String,
    pub added_at: DateTime<Utc>,
}

impl DecisionTopology {
    /// Create a new decision-topology link.
    pub fn new(decision_id: impl Into<String>, topology_id: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            decision_id: decision_id.into(),
            topology_id: topology_id.into(),
            added_at: Utc::now(),
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO decision_topologies (id, decision_id, topology_id, added_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                self.id,
                self.decision_id,
                self.topology_id,
                self.added_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let added_str: String = row.get("added_at")?;
        Ok(Self {
            id: row.get("id")?,
            decision_id: row.get("decision_id")?,
            topology_id: row.get("topology_id")?,
            added_at: DateTime::parse_from_rfc3339(&added_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

// ---------------------------------------------------------------------------
// Event
// ---------------------------------------------------------------------------

/// An event in the undo/redo log with before/after state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub sequence: i64,
    pub event_type: String,
    pub entity_type: String,
    pub entity_id: String,
    pub summary: String,
    pub before_state: Option<String>,
    pub after_state: Option<String>,
    pub source: String,
    pub actor: String,
    pub timestamp: DateTime<Utc>,
}

impl Event {
    /// Create a new event. Sequence must be provided by the caller
    /// (typically max(sequence)+1 from the events table).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sequence: i64,
        event_type: impl Into<String>,
        entity_type: impl Into<String>,
        entity_id: impl Into<String>,
        summary: impl Into<String>,
        before_state: Option<String>,
        after_state: Option<String>,
        source: impl Into<String>,
        actor: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            sequence,
            event_type: event_type.into(),
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            summary: summary.into(),
            before_state,
            after_state,
            source: source.into(),
            actor: actor.into(),
            timestamp: Utc::now(),
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO events (id, sequence, event_type, entity_type, entity_id, summary, before_state, after_state, source, actor, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                self.id,
                self.sequence,
                self.event_type,
                self.entity_type,
                self.entity_id,
                self.summary,
                self.before_state,
                self.after_state,
                self.source,
                self.actor,
                self.timestamp.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let timestamp_str: String = row.get("timestamp")?;
        Ok(Self {
            id: row.get("id")?,
            sequence: row.get("sequence")?,
            event_type: row.get("event_type")?,
            entity_type: row.get("entity_type")?,
            entity_id: row.get("entity_id")?,
            summary: row.get("summary")?,
            before_state: row.get("before_state")?,
            after_state: row.get("after_state")?,
            source: row.get("source")?,
            actor: row.get("actor")?,
            timestamp: DateTime::parse_from_rfc3339(&timestamp_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

// ---------------------------------------------------------------------------
// CatalogItem
// ---------------------------------------------------------------------------

/// A product in the catalog that the user is considering for purchase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogItem {
    pub id: String,
    pub name: String,
    pub category: String,
    pub specs: serde_json::Value,
    pub url: Option<String>,
    pub notes: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CatalogItem {
    /// Create a new catalog item. Defaults: empty specs {}, no url, empty notes.
    pub fn new(name: impl Into<String>, category: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            category: category.into(),
            specs: serde_json::json!({}),
            url: None,
            notes: String::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO catalog_items (id, name, category, specs, url, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                self.id,
                self.name,
                self.category,
                self.specs.to_string(),
                self.url,
                self.notes,
                self.created_at.to_rfc3339(),
                self.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let created_str: String = row.get("created_at")?;
        let updated_str: String = row.get("updated_at")?;
        let specs_str: String = row.get("specs")?;
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            category: row.get("category")?,
            specs: serde_json::from_str(&specs_str).unwrap_or_default(),
            url: row.get("url")?,
            notes: row.get("notes")?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

// ---------------------------------------------------------------------------
// Price
// ---------------------------------------------------------------------------

/// A price observation for a catalog item at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Price {
    pub id: String,
    pub item_id: String,
    pub amount_cents: i64,
    pub currency: String,
    pub source: String,
    pub condition: String,
    pub price_type: String,
    pub observed_at: DateTime<Utc>,
}

impl Price {
    /// Create a new price. Defaults: USD, manual, new, one-time, now.
    pub fn new(item_id: impl Into<String>, amount_cents: i64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            item_id: item_id.into(),
            amount_cents,
            currency: "USD".to_string(),
            source: "manual".to_string(),
            condition: "new".to_string(),
            price_type: "one-time".to_string(),
            observed_at: Utc::now(),
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO prices (id, item_id, amount_cents, currency, source, condition, price_type, observed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                self.id,
                self.item_id,
                self.amount_cents,
                self.currency,
                self.source,
                self.condition,
                self.price_type,
                self.observed_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let observed_str: String = row.get("observed_at")?;
        Ok(Self {
            id: row.get("id")?,
            item_id: row.get("item_id")?,
            amount_cents: row.get("amount_cents")?,
            currency: row.get("currency")?,
            source: row.get("source")?,
            condition: row.get("condition")?,
            price_type: row.get("price_type")?,
            observed_at: DateTime::parse_from_rfc3339(&observed_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Return the price amount in dollars (amount_cents / 100).
    pub fn amount_dollars(&self) -> f64 {
        self.amount_cents as f64 / 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::Database;

    #[test]
    fn test_topology_new() {
        let topo = Topology::new("my-setup", "Main storage topology");
        assert_eq!(topo.name, "my-setup");
        assert_eq!(topo.description, "Main storage topology");
        assert!(topo.tag.is_none());
        // UUID format: 8-4-4-4-12 hex chars
        assert_eq!(topo.id.len(), 36);
        assert_eq!(topo.id.chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn test_topology_roundtrip() {
        let mut db = Database::open_memory().unwrap();
        let topo = Topology::new("test-topo", "A test topology");

        db.transaction(|tx| {
            topo.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let loaded: Topology = db
            .conn()
            .query_row(
                "SELECT id, name, description, parent_id, tag, created_at, updated_at FROM topologies WHERE id = ?1",
                [&topo.id],
                Topology::from_row,
            )
            .unwrap();

        assert_eq!(loaded.id, topo.id);
        assert_eq!(loaded.name, "test-topo");
        assert_eq!(loaded.description, "A test topology");
        assert!(loaded.tag.is_none());
        assert!(loaded.parent_id.is_none());
    }

    #[test]
    fn test_node_roundtrip() {
        let mut db = Database::open_memory().unwrap();
        let topo = Topology::new("test-topo", "desc");

        db.transaction(|tx| {
            topo.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let mut node = Node::new(&topo.id, "mac-mini", "desktop");
        node.location = "office".to_string();
        node.available_bays = Some(0);
        node.interface_types = "usb3,thunderbolt4".to_string();
        node.power_draw_watts = Some(39.0);
        node.cost_estimate = Some(599.99);
        node.noise_db = Some(20.5);
        node.rack_units = Some(1.0);

        db.transaction(|tx| {
            node.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let loaded: Node = db
            .conn()
            .query_row(
                "SELECT id, topology_id, name, role, location, available_bays, interface_types, power_draw_watts, cost_estimate, noise_db, rack_units, item_id, created_at, updated_at FROM nodes WHERE id = ?1",
                [&node.id],
                Node::from_row,
            )
            .unwrap();

        assert_eq!(loaded.id, node.id);
        assert_eq!(loaded.name, "mac-mini");
        assert_eq!(loaded.role, "desktop");
        assert_eq!(loaded.location, "office");
        assert_eq!(loaded.available_bays, Some(0));
        assert_eq!(loaded.interface_types, "usb3,thunderbolt4");
        assert_eq!(loaded.power_draw_watts, Some(39.0));
        assert_eq!(loaded.cost_estimate, Some(599.99));
        assert_eq!(loaded.noise_db, Some(20.5));
        assert_eq!(loaded.rack_units, Some(1.0));
    }

    #[test]
    fn test_topology_to_json() {
        let topo = Topology::new("json-test", "Testing JSON");
        let json_str = topo.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["name"], "json-test");
        assert_eq!(parsed["description"], "Testing JSON");
        assert!(parsed["tag"].is_null());
        assert!(parsed["id"].is_string());
    }

    #[test]
    fn test_cascade_node_volumes() {
        let mut db = Database::open_memory().unwrap();
        let topo = Topology::new("test-topo", "desc");
        let node = Node::new(&topo.id, "node-1", "nas");
        let vol = Volume::new(&topo.id, &node.id, "main-pool", 4_000_000_000_000);

        db.transaction(|tx| {
            topo.insert(tx)?;
            node.insert(tx)?;
            vol.insert(tx)?;
            Ok(())
        })
        .unwrap();

        // Verify volume exists
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM volumes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // Delete the node (should cascade to volume)
        db.transaction(|tx| {
            tx.execute("DELETE FROM nodes WHERE id = ?1", [&node.id])?;
            Ok(())
        })
        .unwrap();

        // Volume should be gone
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM volumes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_volume_roundtrip() {
        let mut db = Database::open_memory().unwrap();
        let topo = Topology::new("t", "d");
        let node = Node::new(&topo.id, "n", "desktop");
        let mut vol = Volume::new(&topo.id, &node.id, "ssd-1", 1_000_000_000_000);
        vol.usable_bytes = Some(930_000_000_000);
        vol.filesystem = Some("apfs".to_string());
        vol.item_id = Some("samsung-870-evo-4tb".to_string());

        db.transaction(|tx| {
            topo.insert(tx)?;
            node.insert(tx)?;
            vol.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let loaded: Volume = db
            .conn()
            .query_row(
                "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, filesystem, raid_level, pool_type, item_id, created_at, updated_at FROM volumes WHERE id = ?1",
                [&vol.id],
                Volume::from_row,
            )
            .unwrap();

        assert_eq!(loaded.capacity_bytes, 1_000_000_000_000);
        assert_eq!(loaded.usable_bytes, Some(930_000_000_000));
        assert_eq!(loaded.filesystem, Some("apfs".to_string()));
        assert_eq!(loaded.item_id, Some("samsung-870-evo-4tb".to_string()));
        assert!(loaded.raid_level.is_none());
    }

    #[test]
    fn test_dataset_roundtrip() {
        let mut db = Database::open_memory().unwrap();
        let topo = Topology::new("t", "d");
        let mut ds = Dataset::new(&topo.id, "photos", 500_000_000_000);
        ds.criticality = "critical".to_string();
        ds.min_copies = 3;
        ds.max_rpo_hours = Some(24);

        db.transaction(|tx| {
            topo.insert(tx)?;
            ds.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let loaded: Dataset = db
            .conn()
            .query_row(
                "SELECT id, topology_id, name, size_bytes, growth_rate_bytes_month, criticality, min_copies, min_locations, max_rpo_hours, created_at, updated_at FROM datasets WHERE id = ?1",
                [&ds.id],
                Dataset::from_row,
            )
            .unwrap();

        assert_eq!(loaded.name, "photos");
        assert_eq!(loaded.size_bytes, 500_000_000_000);
        assert_eq!(loaded.criticality, "critical");
        assert_eq!(loaded.min_copies, 3);
        assert_eq!(loaded.max_rpo_hours, Some(24));
    }

    #[test]
    fn test_placement_roundtrip() {
        let mut db = Database::open_memory().unwrap();
        let topo = Topology::new("t", "d");
        let node = Node::new(&topo.id, "n", "nas");
        let vol = Volume::new(&topo.id, &node.id, "pool", 4_000_000_000_000);
        let ds = Dataset::new(&topo.id, "photos", 500_000_000_000);
        let mut pl = Placement::new(&topo.id, &ds.id, &vol.id);
        pl.role = "backup".to_string();
        pl.priority = 10;

        db.transaction(|tx| {
            topo.insert(tx)?;
            node.insert(tx)?;
            vol.insert(tx)?;
            ds.insert(tx)?;
            pl.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let loaded: Placement = db
            .conn()
            .query_row(
                "SELECT id, topology_id, dataset_id, volume_id, role, priority, created_at FROM placements WHERE id = ?1",
                [&pl.id],
                Placement::from_row,
            )
            .unwrap();

        assert_eq!(loaded.role, "backup");
        assert_eq!(loaded.priority, 10);
        assert_eq!(loaded.dataset_id, ds.id);
        assert_eq!(loaded.volume_id, vol.id);
    }

    #[test]
    fn test_link_roundtrip() {
        let mut db = Database::open_memory().unwrap();
        let topo = Topology::new("t", "d");
        let n1 = Node::new(&topo.id, "mac", "desktop");
        let n2 = Node::new(&topo.id, "nas", "nas");
        let mut link = Link::new(&topo.id, &n1.id, &n2.id, "lan");
        link.bandwidth_bytes_sec = Some(1_000_000_000);
        link.is_metered = false;

        db.transaction(|tx| {
            topo.insert(tx)?;
            n1.insert(tx)?;
            n2.insert(tx)?;
            link.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let loaded: Link = db
            .conn()
            .query_row(
                "SELECT id, topology_id, source_node_id, target_node_id, bandwidth_bytes_sec, connection_type, latency_ms, is_metered, cost_per_gb_cents, created_at, updated_at FROM links WHERE id = ?1",
                [&link.id],
                Link::from_row,
            )
            .unwrap();

        assert_eq!(loaded.connection_type, "lan");
        assert_eq!(loaded.bandwidth_bytes_sec, Some(1_000_000_000));
        assert!(!loaded.is_metered);
    }

    #[test]
    fn test_sync_regime_roundtrip() {
        let mut db = Database::open_memory().unwrap();
        let topo = Topology::new("t", "d");
        let n1 = Node::new(&topo.id, "mac", "desktop");
        let n2 = Node::new(&topo.id, "nas", "nas");
        let v1 = Volume::new(&topo.id, &n1.id, "ssd", 1_000_000_000_000);
        let v2 = Volume::new(&topo.id, &n2.id, "pool", 4_000_000_000_000);
        let ds = Dataset::new(&topo.id, "photos", 500_000_000_000);
        let mut sr = SyncRegime::new(&topo.id, "daily-backup", &ds.id, &v1.id, &v2.id, "rsync");
        sr.schedule = Some("0 2 * * *".to_string());

        db.transaction(|tx| {
            topo.insert(tx)?;
            n1.insert(tx)?;
            n2.insert(tx)?;
            v1.insert(tx)?;
            v2.insert(tx)?;
            ds.insert(tx)?;
            sr.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let loaded: SyncRegime = db
            .conn()
            .query_row(
                "SELECT id, topology_id, name, dataset_id, source_volume_id, target_volume_id, sync_type, schedule, direction, created_at, updated_at FROM sync_regimes WHERE id = ?1",
                [&sr.id],
                SyncRegime::from_row,
            )
            .unwrap();

        assert_eq!(loaded.name, "daily-backup");
        assert_eq!(loaded.sync_type, "rsync");
        assert_eq!(loaded.schedule, Some("0 2 * * *".to_string()));
        assert_eq!(loaded.direction, "push");
    }

    #[test]
    fn test_event_roundtrip() {
        let mut db = Database::open_memory().unwrap();
        let topo = Topology::new("t", "d");
        let after_json = topo.to_json().unwrap();
        let evt = Event::new(
            1,
            "topology.created",
            "topology",
            &topo.id,
            "Created topology 't'",
            None,
            Some(after_json.clone()),
            "user",
            "morgan",
        );

        db.transaction(|tx| {
            topo.insert(tx)?;
            evt.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let loaded: Event = db
            .conn()
            .query_row(
                "SELECT id, sequence, event_type, entity_type, entity_id, summary, before_state, after_state, source, actor, timestamp FROM events WHERE id = ?1",
                [&evt.id],
                Event::from_row,
            )
            .unwrap();

        assert_eq!(loaded.sequence, 1);
        assert_eq!(loaded.event_type, "topology.created");
        assert_eq!(loaded.entity_type, "topology");
        assert!(loaded.before_state.is_none());
        assert!(loaded.after_state.is_some());
        assert_eq!(loaded.source, "user");
        assert_eq!(loaded.actor, "morgan");
    }

    #[test]
    fn test_decision_roundtrip() {
        let mut db = Database::open_memory().unwrap();
        let mut decision = Decision::new("NAS Upgrade 2026");
        decision.description = "Deciding between Synology and custom build".to_string();
        decision.status = "open".to_string();

        db.transaction(|tx| {
            decision.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let loaded: Decision = db
            .conn()
            .query_row(
                "SELECT id, title, description, status, parent_id, chosen_topology_id, rationale, snapshot, created_at, updated_at, closed_at FROM decisions WHERE id = ?1",
                [&decision.id],
                Decision::from_row,
            )
            .unwrap();

        assert_eq!(loaded.id, decision.id);
        assert_eq!(loaded.title, "NAS Upgrade 2026");
        assert_eq!(
            loaded.description,
            "Deciding between Synology and custom build"
        );
        assert_eq!(loaded.status, "open");
        assert!(loaded.parent_id.is_none());
        assert!(loaded.chosen_topology_id.is_none());
        assert!(loaded.rationale.is_none());
        assert!(loaded.snapshot.is_none());
        assert!(loaded.closed_at.is_none());
    }

    #[test]
    fn test_decision_constraint_roundtrip() {
        let mut db = Database::open_memory().unwrap();
        let decision = Decision::new("Budget Test");
        let constraint = DecisionConstraint::new(&decision.id, "budget", 1500.0);

        db.transaction(|tx| {
            decision.insert(tx)?;
            constraint.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let loaded: DecisionConstraint = db
            .conn()
            .query_row(
                "SELECT id, decision_id, constraint_type, max_value, created_at FROM decision_constraints WHERE id = ?1",
                [&constraint.id],
                DecisionConstraint::from_row,
            )
            .unwrap();

        assert_eq!(loaded.id, constraint.id);
        assert_eq!(loaded.decision_id, decision.id);
        assert_eq!(loaded.constraint_type, "budget");
        assert_eq!(loaded.max_value, 1500.0);
    }

    #[test]
    fn test_decision_topology_roundtrip() {
        let mut db = Database::open_memory().unwrap();
        let topo = Topology::new("option-a", "First option");
        let decision = Decision::new("Which Setup?");
        let dt = DecisionTopology::new(&decision.id, &topo.id);

        db.transaction(|tx| {
            topo.insert(tx)?;
            decision.insert(tx)?;
            dt.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let loaded: DecisionTopology = db
            .conn()
            .query_row(
                "SELECT id, decision_id, topology_id, added_at FROM decision_topologies WHERE id = ?1",
                [&dt.id],
                DecisionTopology::from_row,
            )
            .unwrap();

        assert_eq!(loaded.id, dt.id);
        assert_eq!(loaded.decision_id, decision.id);
        assert_eq!(loaded.topology_id, topo.id);
    }

    #[test]
    fn test_catalog_item_new() {
        let item = CatalogItem::new("Samsung 870 EVO 4TB", "ssd");
        assert_eq!(item.name, "Samsung 870 EVO 4TB");
        assert_eq!(item.category, "ssd");
        assert_eq!(item.specs, serde_json::json!({}));
        assert!(item.url.is_none());
        assert_eq!(item.notes, "");
        assert_eq!(item.id.len(), 36);
    }

    #[test]
    fn test_catalog_item_roundtrip() {
        let mut db = Database::open_memory().unwrap();
        let mut item = CatalogItem::new("Samsung 870 EVO 4TB", "ssd");
        item.specs = serde_json::json!({"capacity_gb": 4000, "interface": "SATA"});
        item.url = Some("https://example.com/samsung-870-evo".to_string());
        item.notes = "Good reviews on NAS usage".to_string();

        db.transaction(|tx| {
            item.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let loaded: CatalogItem = db
            .conn()
            .query_row(
                "SELECT id, name, category, specs, url, notes, created_at, updated_at FROM catalog_items WHERE id = ?1",
                [&item.id],
                CatalogItem::from_row,
            )
            .unwrap();

        assert_eq!(loaded.id, item.id);
        assert_eq!(loaded.name, "Samsung 870 EVO 4TB");
        assert_eq!(loaded.category, "ssd");
        assert_eq!(loaded.specs["capacity_gb"], 4000);
        assert_eq!(loaded.specs["interface"], "SATA");
        assert_eq!(
            loaded.url,
            Some("https://example.com/samsung-870-evo".to_string())
        );
        assert_eq!(loaded.notes, "Good reviews on NAS usage");
    }

    #[test]
    fn test_catalog_item_to_json() {
        let item = CatalogItem::new("Test Drive", "hdd");
        let json_str = item.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["name"], "Test Drive");
        assert_eq!(parsed["category"], "hdd");
    }

    #[test]
    fn test_price_new() {
        let price = Price::new("item-123", 29999);
        assert_eq!(price.item_id, "item-123");
        assert_eq!(price.amount_cents, 29999);
        assert_eq!(price.currency, "USD");
        assert_eq!(price.source, "manual");
        assert_eq!(price.condition, "new");
        assert_eq!(price.price_type, "one-time");
        assert_eq!(price.id.len(), 36);
    }

    #[test]
    fn test_price_amount_dollars() {
        let price = Price::new("item-123", 29999);
        assert!((price.amount_dollars() - 299.99).abs() < f64::EPSILON);

        let price_zero = Price::new("item-123", 0);
        assert!((price_zero.amount_dollars() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_price_roundtrip() {
        let mut db = Database::open_memory().unwrap();
        let item = CatalogItem::new("Samsung 870 EVO 4TB", "ssd");
        let mut price = Price::new(&item.id, 29999);
        price.source = "bestbuy".to_string();
        price.condition = "new".to_string();
        price.price_type = "one-time".to_string();

        db.transaction(|tx| {
            item.insert(tx)?;
            price.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let loaded: Price = db
            .conn()
            .query_row(
                "SELECT id, item_id, amount_cents, currency, source, condition, price_type, observed_at FROM prices WHERE id = ?1",
                [&price.id],
                Price::from_row,
            )
            .unwrap();

        assert_eq!(loaded.id, price.id);
        assert_eq!(loaded.item_id, item.id);
        assert_eq!(loaded.amount_cents, 29999);
        assert_eq!(loaded.currency, "USD");
        assert_eq!(loaded.source, "bestbuy");
        assert_eq!(loaded.condition, "new");
        assert_eq!(loaded.price_type, "one-time");
    }

    #[test]
    fn test_price_to_json() {
        let price = Price::new("item-123", 15000);
        let json_str = price.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["amount_cents"], 15000);
        assert_eq!(parsed["currency"], "USD");
    }
}
