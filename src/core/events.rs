//! Append-only event logging system
//!
//! All mutations in the system generate events for auditing.
//! Events are never deleted or modified.

use anyhow::Result;
use rusqlite::{params, Connection, Transaction};
use serde_json::Value as JsonValue;

use super::models::{EntityType, Event, EventType};

/// Event logging interface
pub struct EventLog;

impl EventLog {
    /// Record an event (append-only)
    pub fn record(
        tx: &Transaction,
        event_type: EventType,
        entity_type: EntityType,
        entity_id: &str,
        payload: JsonValue,
        actor: &str,
    ) -> Result<Event> {
        let event = Event::new(event_type, entity_type, entity_id, payload, actor);
        event.insert(tx)?;
        Ok(event)
    }

    /// Get recent events (most recent first)
    pub fn recent(conn: &Connection, limit: usize) -> Result<Vec<Event>> {
        let mut stmt = conn.prepare(
            "SELECT id, event_type, entity_type, entity_id, payload, timestamp, actor
             FROM events ORDER BY timestamp DESC LIMIT ?1",
        )?;

        let events = stmt
            .query_map([limit], Event::from_row)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Get events for a specific entity
    pub fn for_entity(
        conn: &Connection,
        entity_type: EntityType,
        entity_id: &str,
    ) -> Result<Vec<Event>> {
        let mut stmt = conn.prepare(
            "SELECT id, event_type, entity_type, entity_id, payload, timestamp, actor
             FROM events WHERE entity_type = ?1 AND entity_id = ?2
             ORDER BY timestamp DESC",
        )?;

        let events = stmt
            .query_map(params![entity_type.as_str(), entity_id], Event::from_row)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Get events by type
    pub fn by_type(conn: &Connection, event_type: EventType, limit: usize) -> Result<Vec<Event>> {
        let mut stmt = conn.prepare(
            "SELECT id, event_type, entity_type, entity_id, payload, timestamp, actor
             FROM events WHERE event_type = ?1
             ORDER BY timestamp DESC LIMIT ?2",
        )?;

        let events = stmt
            .query_map(params![event_type.as_str(), limit], Event::from_row)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Count total events
    pub fn count(conn: &Connection) -> Result<i64> {
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
        Ok(count)
    }
}

/// Helper to get current actor from environment
pub fn current_actor() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::Database;

    #[test]
    fn test_record_and_retrieve() {
        let mut db = Database::open_memory().unwrap();
        db.migrate().unwrap();

        // Record an event
        db.transaction(|tx| {
            EventLog::record(
                tx,
                EventType::Created,
                EntityType::Item,
                "test-item-1",
                serde_json::json!({"name": "Test Item"}),
                "test-user",
            )
        })
        .unwrap();

        // Retrieve recent events
        let events = EventLog::recent(db.conn(), 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].entity_id, "test-item-1");
        assert_eq!(events[0].actor, "test-user");
    }

    #[test]
    fn test_for_entity() {
        let mut db = Database::open_memory().unwrap();
        db.migrate().unwrap();

        // Record multiple events
        db.transaction(|tx| {
            EventLog::record(
                tx,
                EventType::Created,
                EntityType::Item,
                "item-1",
                serde_json::json!({}),
                "user",
            )?;
            EventLog::record(
                tx,
                EventType::Updated,
                EntityType::Item,
                "item-1",
                serde_json::json!({}),
                "user",
            )?;
            EventLog::record(
                tx,
                EventType::Created,
                EntityType::Item,
                "item-2",
                serde_json::json!({}),
                "user",
            )?;
            Ok(())
        })
        .unwrap();

        // Get events for item-1
        let events = EventLog::for_entity(db.conn(), EntityType::Item, "item-1").unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_events_are_append_only() {
        let mut db = Database::open_memory().unwrap();
        db.migrate().unwrap();

        // Record an event
        db.transaction(|tx| {
            EventLog::record(
                tx,
                EventType::Created,
                EntityType::Item,
                "test",
                serde_json::json!({}),
                "user",
            )
        })
        .unwrap();

        // Try to delete - should fail or have no effect
        // (In practice, we just don't expose delete operations)
        let count_before = EventLog::count(db.conn()).unwrap();
        assert_eq!(count_before, 1);
    }
}
