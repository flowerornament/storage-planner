//! SQLite database layer
//!
//! Provides atomic transactions, PRAGMA user_version migrations, and connection management.
//! All topology tables, the events table, and the undo_pointer are created by migration v1.

use anyhow::{Context, Result};
use rusqlite::{Connection, Transaction};
use std::path::{Path, PathBuf};

/// Current schema version. Bump this when adding new migrations.
pub const CURRENT_VERSION: i32 = 1;

/// A single schema migration step.
struct Migration {
    version: i32,
    sql: &'static str,
}

/// All migrations in order. Each migration's SQL must end with
/// `PRAGMA user_version = N;` to record its version.
const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    sql: SCHEMA_V1,
}];

/// Database wrapper providing atomic transactions and migrations
pub struct Database {
    path: PathBuf,
    conn: Connection,
}

impl Database {
    /// Open or create database at the given path.
    ///
    /// Creates parent directories if needed, sets PRAGMAs, and runs migrations.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open database: {}", path.display()))?;

        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )?;

        let mut db = Self {
            path: path.to_path_buf(),
            conn,
        };
        db.migrate()?;
        Ok(db)
    }

    /// Open an in-memory database (for testing).
    ///
    /// Sets foreign_keys ON and runs migrations.
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let mut db = Self {
            path: PathBuf::from(":memory:"),
            conn,
        };
        db.migrate()?;
        Ok(db)
    }

    /// Get the database file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Execute a function within a transaction.
    /// Automatically commits on success, rolls back on error.
    pub fn transaction<T, F>(&mut self, f: F) -> Result<T>
    where
        F: FnOnce(&Transaction) -> Result<T>,
    {
        let tx = self.conn.transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }

    /// Get a reference to the underlying connection (for queries).
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Get a mutable reference to the underlying connection.
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Run schema migrations using PRAGMA user_version tracking.
    ///
    /// Reads the current version, then applies any migrations with version > current
    /// in order. Each migration SQL sets `PRAGMA user_version = N` at the end.
    pub fn migrate(&mut self) -> Result<()> {
        let current: i32 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?;

        for migration in MIGRATIONS {
            if migration.version > current {
                self.conn
                    .execute_batch(migration.sql)
                    .with_context(|| format!("Failed to apply migration v{}", migration.version))?;
            }
        }
        Ok(())
    }

    /// Check if database has been initialized (topologies table exists).
    pub fn is_initialized(&self) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='topologies'",
            [],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

/// Phase 1 schema (version 1): All topology tables + redesigned events + undo_pointer
const SCHEMA_V1: &str = r#"
-- Topologies: named storage configurations
CREATE TABLE topologies (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    parent_id TEXT REFERENCES topologies(id),
    is_active INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Nodes: compute devices that host storage
CREATE TABLE nodes (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    role TEXT NOT NULL,
    location TEXT NOT NULL DEFAULT '',
    available_bays INTEGER,
    interface_types TEXT NOT NULL DEFAULT '',
    power_draw_watts REAL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(topology_id, name)
);

-- Volumes: storage units attached to nodes
CREATE TABLE volumes (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    capacity_bytes INTEGER NOT NULL,
    usable_bytes INTEGER,
    filesystem TEXT,
    raid_level TEXT,
    pool_type TEXT,
    item_id TEXT,  -- FK added in Phase 6 when catalog is ported
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(topology_id, node_id, name)
);

-- Datasets: logical data groups with requirements
CREATE TABLE datasets (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    growth_rate_bytes_month REAL,
    criticality TEXT NOT NULL DEFAULT 'normal',
    min_copies INTEGER NOT NULL DEFAULT 1,
    min_locations INTEGER NOT NULL DEFAULT 1,
    max_rpo_hours INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(topology_id, name)
);

-- Placements: junction table mapping datasets to volumes
CREATE TABLE placements (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    dataset_id TEXT NOT NULL REFERENCES datasets(id) ON DELETE CASCADE,
    volume_id TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'primary',
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(dataset_id, volume_id)
);

-- Links: network connections between nodes
CREATE TABLE links (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    source_node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    target_node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    bandwidth_bytes_sec INTEGER,
    connection_type TEXT NOT NULL,
    latency_ms REAL,
    is_metered INTEGER NOT NULL DEFAULT 0,
    cost_per_gb_cents INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(topology_id, source_node_id, target_node_id)
);

-- Sync regimes: data movement definitions
CREATE TABLE sync_regimes (
    id TEXT PRIMARY KEY,
    topology_id TEXT NOT NULL REFERENCES topologies(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    dataset_id TEXT NOT NULL REFERENCES datasets(id) ON DELETE CASCADE,
    source_volume_id TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
    target_volume_id TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
    sync_type TEXT NOT NULL,
    schedule TEXT,
    direction TEXT NOT NULL DEFAULT 'push',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(topology_id, name)
);

-- Events: redesigned with before/after state for undo/redo
CREATE TABLE events (
    id TEXT PRIMARY KEY,
    sequence INTEGER NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    summary TEXT NOT NULL,
    before_state TEXT,
    after_state TEXT,
    source TEXT NOT NULL DEFAULT 'user',
    actor TEXT NOT NULL DEFAULT 'unknown',
    timestamp TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_events_sequence ON events(sequence);
CREATE INDEX idx_events_entity ON events(entity_type, entity_id);
CREATE INDEX idx_events_timestamp ON events(timestamp);

-- Undo pointer: single row tracking current position in event log
CREATE TABLE undo_pointer (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    current_sequence INTEGER NOT NULL DEFAULT 0
);
INSERT INTO undo_pointer (id, current_sequence) VALUES (1, 0);

-- Topology table indexes
CREATE INDEX idx_nodes_topology ON nodes(topology_id);
CREATE INDEX idx_volumes_topology ON volumes(topology_id);
CREATE INDEX idx_volumes_node ON volumes(node_id);
CREATE INDEX idx_datasets_topology ON datasets(topology_id);
CREATE INDEX idx_placements_dataset ON placements(dataset_id);
CREATE INDEX idx_placements_volume ON placements(volume_id);
CREATE INDEX idx_links_topology ON links(topology_id);
CREATE INDEX idx_sync_regimes_topology ON sync_regimes(topology_id);
CREATE INDEX idx_sync_regimes_dataset ON sync_regimes(dataset_id);

PRAGMA user_version = 1;
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_memory() {
        let db = Database::open_memory().unwrap();
        assert_eq!(db.path(), Path::new(":memory:"));
    }

    #[test]
    fn test_migrate() {
        let db = Database::open_memory().unwrap();
        let version: i32 = db
            .conn()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_VERSION);
    }

    #[test]
    fn test_tables_exist() {
        let db = Database::open_memory().unwrap();
        let mut stmt = db
            .conn()
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        let expected = vec![
            "datasets",
            "events",
            "links",
            "nodes",
            "placements",
            "sync_regimes",
            "topologies",
            "undo_pointer",
            "volumes",
        ];
        assert_eq!(tables, expected);
    }

    #[test]
    fn test_foreign_keys_on() {
        let db = Database::open_memory().unwrap();
        let fk_enabled: i32 = db
            .conn()
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert_eq!(fk_enabled, 1);
    }

    #[test]
    fn test_cascade_delete() {
        let mut db = Database::open_memory().unwrap();

        // Insert a topology
        db.transaction(|tx| {
            tx.execute(
                "INSERT INTO topologies (id, name, created_at, updated_at) VALUES (?1, ?2, datetime('now'), datetime('now'))",
                ["topo-1", "test-topology"],
            )?;
            // Insert a node in that topology
            tx.execute(
                "INSERT INTO nodes (id, topology_id, name, role, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, datetime('now'), datetime('now'))",
                ["node-1", "topo-1", "test-node", "desktop"],
            )?;
            Ok(())
        })
        .unwrap();

        // Verify node exists
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // Delete the topology
        db.transaction(|tx| {
            tx.execute("DELETE FROM topologies WHERE id = ?1", ["topo-1"])?;
            Ok(())
        })
        .unwrap();

        // Verify node was cascade-deleted
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_is_initialized() {
        let db = Database::open_memory().unwrap();
        assert!(db.is_initialized().unwrap());
    }

    #[test]
    fn test_undo_pointer_initialized() {
        let db = Database::open_memory().unwrap();
        let seq: i64 = db
            .conn()
            .query_row(
                "SELECT current_sequence FROM undo_pointer WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(seq, 0);
    }

    #[test]
    fn test_volumes_item_id_no_fk() {
        let mut db = Database::open_memory().unwrap();

        // Insert topology and node
        db.transaction(|tx| {
            tx.execute(
                "INSERT INTO topologies (id, name) VALUES ('t1', 'test')",
                [],
            )?;
            tx.execute(
                "INSERT INTO nodes (id, topology_id, name, role) VALUES ('n1', 't1', 'node', 'desktop')",
                [],
            )?;
            Ok(())
        }).unwrap();

        // Insert volume with arbitrary item_id (no FK constraint should fail)
        let result = db.transaction(|tx| {
            tx.execute(
                "INSERT INTO volumes (id, topology_id, node_id, name, capacity_bytes, item_id) VALUES ('v1', 't1', 'n1', 'vol', 1000000, 'nonexistent-item')",
                [],
            )?;
            Ok(())
        });
        assert!(result.is_ok(), "item_id should have no FK constraint");
    }
}
