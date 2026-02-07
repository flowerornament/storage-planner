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
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Topology {
    /// Create a new topology. Defaults: is_active=false, no parent.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            parent_id: None,
            is_active: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO topologies (id, name, description, parent_id, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                self.id,
                self.name,
                self.description,
                self.parent_id,
                self.is_active as i32,
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
            is_active: row.get::<_, i32>("is_active")? != 0,
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
            created_at: now,
            updated_at: now,
        }
    }

    pub fn insert(&self, tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute(
            "INSERT INTO nodes (id, topology_id, name, role, location, available_bays, interface_types, power_draw_watts, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                self.id,
                self.topology_id,
                self.name,
                self.role,
                self.location,
                self.available_bays,
                self.interface_types,
                self.power_draw_watts,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::Database;

    #[test]
    fn test_topology_new() {
        let topo = Topology::new("my-setup", "Main storage topology");
        assert_eq!(topo.name, "my-setup");
        assert_eq!(topo.description, "Main storage topology");
        assert!(!topo.is_active);
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
                "SELECT id, name, description, parent_id, is_active, created_at, updated_at FROM topologies WHERE id = ?1",
                [&topo.id],
                Topology::from_row,
            )
            .unwrap();

        assert_eq!(loaded.id, topo.id);
        assert_eq!(loaded.name, "test-topo");
        assert_eq!(loaded.description, "A test topology");
        assert!(!loaded.is_active);
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

        db.transaction(|tx| {
            node.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let loaded: Node = db
            .conn()
            .query_row(
                "SELECT id, topology_id, name, role, location, available_bays, interface_types, power_draw_watts, created_at, updated_at FROM nodes WHERE id = ?1",
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
    }

    #[test]
    fn test_topology_to_json() {
        let topo = Topology::new("json-test", "Testing JSON");
        let json_str = topo.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["name"], "json-test");
        assert_eq!(parsed["description"], "Testing JSON");
        assert_eq!(parsed["is_active"], false);
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
}
