//! SQLite database layer
//!
//! Provides atomic transactions, migrations, and connection management.

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use rusqlite::{Connection, Transaction};
use std::fs;

/// Database wrapper providing atomic transactions and migrations
pub struct Database {
    path: Utf8PathBuf,
    conn: Connection,
}

impl Database {
    /// Open or create database at the given path
    pub fn open(path: &Utf8Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {parent}"))?;
        }

        let conn =
            Connection::open(path).with_context(|| format!("Failed to open database: {path}"))?;

        // Enable foreign keys and WAL mode for better concurrency
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )?;

        Ok(Self {
            path: path.to_owned(),
            conn,
        })
    }

    /// Open an in-memory database (for testing)
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        Ok(Self {
            path: Utf8PathBuf::from(":memory:"),
            conn,
        })
    }

    /// Get the database file path
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// Execute a function within a transaction
    /// Automatically commits on success, rolls back on error
    pub fn transaction<T, F>(&mut self, f: F) -> Result<T>
    where
        F: FnOnce(&Transaction) -> Result<T>,
    {
        let tx = self.conn.transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }

    /// Get a reference to the underlying connection (for queries)
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Get a mutable reference to the underlying connection
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Run schema migrations
    pub fn migrate(&mut self) -> Result<()> {
        self.conn.execute_batch(SCHEMA)?;
        Ok(())
    }

    /// Check if database has been initialized
    pub fn is_initialized(&self) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='items'",
            [],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get database statistics
    pub fn stats(&self) -> Result<DbStats> {
        let items: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM items", [], |row| row.get(0))?;
        let prices: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM prices", [], |row| row.get(0))?;
        let configurations: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM configurations", [], |row| row.get(0))?;
        let events: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
        let decisions: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM decisions", [], |row| row.get(0))?;

        Ok(DbStats {
            items,
            prices,
            configurations,
            events,
            decisions,
        })
    }
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DbStats {
    pub items: i64,
    pub prices: i64,
    pub configurations: i64,
    pub events: i64,
    pub decisions: i64,
}

/// Initial database schema
const SCHEMA: &str = r#"
-- Items: catalog of purchasable things
CREATE TABLE IF NOT EXISTS items (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    category TEXT NOT NULL,
    brand TEXT,
    specs TEXT NOT NULL DEFAULT '{}',  -- JSON object
    tags TEXT NOT NULL DEFAULT '[]',   -- JSON array
    metadata TEXT NOT NULL DEFAULT '{}',
    archived INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_items_category ON items(category);
CREATE INDEX IF NOT EXISTS idx_items_archived ON items(archived);

-- Prices: price observations (append-only)
CREATE TABLE IF NOT EXISTS prices (
    id TEXT PRIMARY KEY,
    item_id TEXT NOT NULL REFERENCES items(id),
    source TEXT NOT NULL,           -- 'ebay', 'bestbuy', 'amazon', 'manual', or custom
    price REAL NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    condition TEXT NOT NULL,        -- 'new', 'used', 'refurbished', 'open_box'
    url TEXT,
    observed_at TEXT NOT NULL DEFAULT (datetime('now')),
    metadata TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_prices_item_id ON prices(item_id);
CREATE INDEX IF NOT EXISTS idx_prices_observed_at ON prices(observed_at);

-- Configurations: named compositions of items forming a system
CREATE TABLE IF NOT EXISTS configurations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    domain TEXT NOT NULL DEFAULT 'storage',  -- 'storage', 'computing', etc.
    items TEXT NOT NULL DEFAULT '[]',        -- JSON array of item references with quantities
    domain_data TEXT NOT NULL DEFAULT '{}',  -- Domain-specific data (topology for storage)
    metadata TEXT NOT NULL DEFAULT '{}',
    is_current INTEGER NOT NULL DEFAULT 0,   -- Only one can be current
    archived INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_configurations_is_current ON configurations(is_current);
CREATE INDEX IF NOT EXISTS idx_configurations_archived ON configurations(archived);

-- Decisions: recorded choices (append-only)
CREATE TABLE IF NOT EXISTS decisions (
    id TEXT PRIMARY KEY,
    purpose TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',  -- 'active', 'decided', 'abandoned'
    options TEXT NOT NULL DEFAULT '{}',     -- JSON object mapping option names to config IDs
    chosen_option TEXT,                     -- Name of chosen option
    chosen_config_id TEXT REFERENCES configurations(id),
    rationale TEXT,
    decided_at TEXT,
    decided_by TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    metadata TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_decisions_status ON decisions(status);

-- Events: immutable audit log (append-only)
CREATE TABLE IF NOT EXISTS events (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    entity_type TEXT NOT NULL,  -- 'item', 'price', 'configuration', 'decision'
    entity_id TEXT NOT NULL,
    payload TEXT NOT NULL DEFAULT '{}',  -- JSON with event details
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    actor TEXT NOT NULL DEFAULT 'unknown'
);

CREATE INDEX IF NOT EXISTS idx_events_entity ON events(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);

-- Full-text search for items
CREATE VIRTUAL TABLE IF NOT EXISTS items_fts USING fts5(
    id,
    name,
    category,
    brand,
    tags,
    content='items',
    content_rowid='rowid'
);

-- Triggers to keep FTS index in sync
CREATE TRIGGER IF NOT EXISTS items_ai AFTER INSERT ON items BEGIN
    INSERT INTO items_fts(rowid, id, name, category, brand, tags)
    VALUES (NEW.rowid, NEW.id, NEW.name, NEW.category, NEW.brand, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS items_ad AFTER DELETE ON items BEGIN
    INSERT INTO items_fts(items_fts, rowid, id, name, category, brand, tags)
    VALUES ('delete', OLD.rowid, OLD.id, OLD.name, OLD.category, OLD.brand, OLD.tags);
END;

CREATE TRIGGER IF NOT EXISTS items_au AFTER UPDATE ON items BEGIN
    INSERT INTO items_fts(items_fts, rowid, id, name, category, brand, tags)
    VALUES ('delete', OLD.rowid, OLD.id, OLD.name, OLD.category, OLD.brand, OLD.tags);
    INSERT INTO items_fts(rowid, id, name, category, brand, tags)
    VALUES (NEW.rowid, NEW.id, NEW.name, NEW.category, NEW.brand, NEW.tags);
END;
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_memory() {
        let db = Database::open_memory().unwrap();
        assert_eq!(db.path(), ":memory:");
    }

    #[test]
    fn test_migrate() {
        let mut db = Database::open_memory().unwrap();
        db.migrate().unwrap();
        assert!(db.is_initialized().unwrap());
    }

    #[test]
    fn test_stats_empty() {
        let mut db = Database::open_memory().unwrap();
        db.migrate().unwrap();
        let stats = db.stats().unwrap();
        assert_eq!(stats.items, 0);
        assert_eq!(stats.prices, 0);
        assert_eq!(stats.events, 0);
    }

    #[test]
    fn test_transaction() {
        let mut db = Database::open_memory().unwrap();
        db.migrate().unwrap();

        let result = db.transaction(|tx| {
            tx.execute(
                "INSERT INTO items (id, name, category, specs) VALUES (?1, ?2, ?3, ?4)",
                ["test-1", "Test Item", "ssd", "{}"],
            )?;
            Ok(42)
        });

        assert_eq!(result.unwrap(), 42);

        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
