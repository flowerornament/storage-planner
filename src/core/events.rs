//! Event system with undo/redo engine
//!
//! Records every mutation as an event with before/after JSON state.
//! The undo_pointer table tracks the current position in the event log.
//! Undo reverses the last action; redo re-applies it.
//! New actions after an undo clear the redo stack (events beyond the pointer).

use anyhow::{bail, Context, Result};
use rusqlite::{params, Transaction};
use serde::{Deserialize, Serialize};
use std::fmt;

use super::db::Database;
use super::models::{
    Dataset, Decision, DecisionConstraint, DecisionTopology, Event, Link, Node, Placement,
    SyncRegime, Topology, Volume,
};

// ---------------------------------------------------------------------------
// EventSource
// ---------------------------------------------------------------------------

/// Source of an event (who/what triggered the mutation)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventSource {
    User,
    Agent,
    Import,
    Migration,
}

impl fmt::Display for EventSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventSource::User => write!(f, "user"),
            EventSource::Agent => write!(f, "agent"),
            EventSource::Import => write!(f, "import"),
            EventSource::Migration => write!(f, "migration"),
        }
    }
}

impl std::str::FromStr for EventSource {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "user" => Ok(EventSource::User),
            "agent" => Ok(EventSource::Agent),
            "import" => Ok(EventSource::Import),
            "migration" => Ok(EventSource::Migration),
            _ => bail!("Unknown event source: {}", s),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Get the current actor from the USER environment variable.
pub fn current_actor() -> String {
    std::env::var("USER").unwrap_or_else(|_| "unknown".to_string())
}

/// Map entity type name to database table name.
pub fn entity_table_name(entity_type: &str) -> Result<&'static str> {
    match entity_type {
        "topology" => Ok("topologies"),
        "node" => Ok("nodes"),
        "volume" => Ok("volumes"),
        "dataset" => Ok("datasets"),
        "placement" => Ok("placements"),
        "link" => Ok("links"),
        "sync_regime" => Ok("sync_regimes"),
        "decision" => Ok("decisions"),
        "decision_constraint" => Ok("decision_constraints"),
        "decision_topology" => Ok("decision_topologies"),
        _ => bail!("Unknown entity type: {}", entity_type),
    }
}

/// Delete an entity from the database by entity_type and entity_id.
pub fn delete_entity(tx: &Transaction, entity_type: &str, entity_id: &str) -> Result<()> {
    let table = entity_table_name(entity_type)?;
    let sql = format!("DELETE FROM {} WHERE id = ?1", table);
    tx.execute(&sql, params![entity_id])?;
    Ok(())
}

/// Restore an entity from JSON state by deserializing and inserting.
pub fn restore_entity_from_json(
    tx: &Transaction,
    entity_type: &str,
    json_state: &str,
) -> Result<()> {
    match entity_type {
        "topology" => {
            let entity: Topology = serde_json::from_str(json_state)
                .context("Failed to deserialize topology from JSON")?;
            entity.insert(tx)?;
        }
        "node" => {
            let entity: Node =
                serde_json::from_str(json_state).context("Failed to deserialize node from JSON")?;
            entity.insert(tx)?;
        }
        "volume" => {
            let entity: Volume = serde_json::from_str(json_state)
                .context("Failed to deserialize volume from JSON")?;
            entity.insert(tx)?;
        }
        "dataset" => {
            let entity: Dataset = serde_json::from_str(json_state)
                .context("Failed to deserialize dataset from JSON")?;
            entity.insert(tx)?;
        }
        "placement" => {
            let entity: Placement = serde_json::from_str(json_state)
                .context("Failed to deserialize placement from JSON")?;
            entity.insert(tx)?;
        }
        "link" => {
            let entity: Link =
                serde_json::from_str(json_state).context("Failed to deserialize link from JSON")?;
            entity.insert(tx)?;
        }
        "sync_regime" => {
            let entity: SyncRegime = serde_json::from_str(json_state)
                .context("Failed to deserialize sync_regime from JSON")?;
            entity.insert(tx)?;
        }
        "decision" => {
            let entity: Decision = serde_json::from_str(json_state)
                .context("Failed to deserialize decision from JSON")?;
            entity.insert(tx)?;
        }
        "decision_constraint" => {
            let entity: DecisionConstraint = serde_json::from_str(json_state)
                .context("Failed to deserialize decision_constraint from JSON")?;
            entity.insert(tx)?;
        }
        "decision_topology" => {
            let entity: DecisionTopology = serde_json::from_str(json_state)
                .context("Failed to deserialize decision_topology from JSON")?;
            entity.insert(tx)?;
        }
        _ => bail!("Unknown entity type for restore: {}", entity_type),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Core event functions
// ---------------------------------------------------------------------------

/// Record a new event within an existing transaction.
///
/// - Clears the redo stack if we're not at the end (new action after undo)
/// - Assigns the next sequence number
/// - Updates the undo_pointer to the new sequence
/// - Returns the created Event
#[allow(clippy::too_many_arguments)]
pub fn record_event(
    tx: &Transaction,
    event_type: &str,
    entity_type: &str,
    entity_id: &str,
    summary: &str,
    before_state: Option<&str>,
    after_state: Option<&str>,
    source: &EventSource,
) -> Result<Event> {
    // Read current pointer position
    let current_seq: i64 = tx.query_row(
        "SELECT current_sequence FROM undo_pointer WHERE id = 1",
        [],
        |row| row.get(0),
    )?;

    // Read max sequence in events
    let max_seq: i64 =
        tx.query_row("SELECT COALESCE(MAX(sequence), 0) FROM events", [], |row| {
            row.get(0)
        })?;

    // If pointer is behind max, clear redo stack (events beyond pointer)
    if current_seq < max_seq {
        tx.execute(
            "DELETE FROM events WHERE sequence > ?1",
            params![current_seq],
        )?;
    }

    // Next sequence
    let new_seq = current_seq + 1;

    // Create and insert event
    let event = Event::new(
        new_seq,
        event_type,
        entity_type,
        entity_id,
        summary,
        before_state.map(|s| s.to_string()),
        after_state.map(|s| s.to_string()),
        source.to_string(),
        current_actor(),
    );
    event.insert(tx)?;

    // Update undo_pointer
    tx.execute(
        "UPDATE undo_pointer SET current_sequence = ?1 WHERE id = 1",
        params![new_seq],
    )?;

    Ok(event)
}

/// Undo the last action.
///
/// Reads the current sequence from undo_pointer, finds the event at that sequence,
/// reverses the mutation, and decrements the pointer.
/// Returns the summary of the undone event.
pub fn undo(db: &mut Database) -> Result<String> {
    db.transaction(|tx| {
        let current_seq: i64 = tx.query_row(
            "SELECT current_sequence FROM undo_pointer WHERE id = 1",
            [],
            |row| row.get(0),
        )?;

        if current_seq < 1 {
            bail!("Nothing to undo");
        }

        // Get the event at current_sequence
        let event: Event = tx.query_row(
            "SELECT id, sequence, event_type, entity_type, entity_id, summary, before_state, after_state, source, actor, timestamp FROM events WHERE sequence = ?1",
            params![current_seq],
            Event::from_row,
        ).context("Failed to find event for undo")?;

        // Reverse based on event_type suffix
        if event.event_type.ends_with(".created") {
            // Undo creation = delete the entity
            delete_entity(tx, &event.entity_type, &event.entity_id)?;
        } else if event.event_type.ends_with(".deleted") {
            // Undo deletion = restore from before_state
            let before = event
                .before_state
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("No before_state for deleted event"))?;
            restore_entity_from_json(tx, &event.entity_type, before)?;
        } else if event.event_type.ends_with(".updated") {
            // Undo update = restore before_state (delete current, insert before)
            let before = event
                .before_state
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("No before_state for updated event"))?;
            delete_entity(tx, &event.entity_type, &event.entity_id)?;
            restore_entity_from_json(tx, &event.entity_type, before)?;
        } else {
            bail!("Unknown event type suffix: {}", event.event_type);
        }

        // Decrement pointer
        tx.execute(
            "UPDATE undo_pointer SET current_sequence = ?1 WHERE id = 1",
            params![current_seq - 1],
        )?;

        Ok(event.summary)
    })
}

/// Redo the last undone action.
///
/// Reads the current sequence from undo_pointer, finds the event at sequence+1,
/// re-applies the mutation, and increments the pointer.
/// Returns the summary of the redone event.
pub fn redo(db: &mut Database) -> Result<String> {
    db.transaction(|tx| {
        let current_seq: i64 = tx.query_row(
            "SELECT current_sequence FROM undo_pointer WHERE id = 1",
            [],
            |row| row.get(0),
        )?;

        let next_seq = current_seq + 1;

        // Check if event exists at next_seq
        let event: Event = tx.query_row(
            "SELECT id, sequence, event_type, entity_type, entity_id, summary, before_state, after_state, source, actor, timestamp FROM events WHERE sequence = ?1",
            params![next_seq],
            Event::from_row,
        ).map_err(|_| anyhow::anyhow!("Nothing to redo"))?;

        // Re-apply based on event_type suffix
        if event.event_type.ends_with(".created") {
            // Redo creation = re-insert from after_state
            let after = event
                .after_state
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("No after_state for created event"))?;
            restore_entity_from_json(tx, &event.entity_type, after)?;
        } else if event.event_type.ends_with(".deleted") {
            // Redo deletion = delete the entity again
            delete_entity(tx, &event.entity_type, &event.entity_id)?;
        } else if event.event_type.ends_with(".updated") {
            // Redo update = apply after_state (delete current, insert after)
            let after = event
                .after_state
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("No after_state for updated event"))?;
            delete_entity(tx, &event.entity_type, &event.entity_id)?;
            restore_entity_from_json(tx, &event.entity_type, after)?;
        } else {
            bail!("Unknown event type suffix: {}", event.event_type);
        }

        // Increment pointer
        tx.execute(
            "UPDATE undo_pointer SET current_sequence = ?1 WHERE id = 1",
            params![next_seq],
        )?;

        Ok(event.summary)
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Database {
        Database::open_memory().unwrap()
    }

    #[test]
    fn test_event_source_display() {
        assert_eq!(EventSource::User.to_string(), "user");
        assert_eq!(EventSource::Agent.to_string(), "agent");
        assert_eq!(EventSource::Import.to_string(), "import");
        assert_eq!(EventSource::Migration.to_string(), "migration");
    }

    #[test]
    fn test_event_source_from_str() {
        assert!(matches!(
            "user".parse::<EventSource>().unwrap(),
            EventSource::User
        ));
        assert!(matches!(
            "Agent".parse::<EventSource>().unwrap(),
            EventSource::Agent
        ));
        assert!("invalid".parse::<EventSource>().is_err());
    }

    #[test]
    fn test_entity_table_name() {
        assert_eq!(entity_table_name("topology").unwrap(), "topologies");
        assert_eq!(entity_table_name("node").unwrap(), "nodes");
        assert_eq!(entity_table_name("volume").unwrap(), "volumes");
        assert_eq!(entity_table_name("dataset").unwrap(), "datasets");
        assert_eq!(entity_table_name("placement").unwrap(), "placements");
        assert_eq!(entity_table_name("link").unwrap(), "links");
        assert_eq!(entity_table_name("sync_regime").unwrap(), "sync_regimes");
        assert_eq!(entity_table_name("decision").unwrap(), "decisions");
        assert_eq!(
            entity_table_name("decision_constraint").unwrap(),
            "decision_constraints"
        );
        assert_eq!(
            entity_table_name("decision_topology").unwrap(),
            "decision_topologies"
        );
        assert!(entity_table_name("unknown").is_err());
    }

    #[test]
    fn test_record_event() {
        let mut db = setup_db();
        let topo = Topology::new("test-topo", "A test topology");
        let after_json = topo.to_json().unwrap();

        let event = db
            .transaction(|tx| {
                topo.insert(tx)?;
                record_event(
                    tx,
                    "topology.created",
                    "topology",
                    &topo.id,
                    "Created topology 'test-topo'",
                    None,
                    Some(&after_json),
                    &EventSource::User,
                )
            })
            .unwrap();

        assert_eq!(event.sequence, 1);
        assert_eq!(event.event_type, "topology.created");
        assert_eq!(event.entity_type, "topology");
        assert_eq!(event.entity_id, topo.id);
        assert!(event.before_state.is_none());
        assert!(event.after_state.is_some());
        assert_eq!(event.source, "user");

        // Verify undo_pointer was updated
        let ptr: i64 = db
            .conn()
            .query_row(
                "SELECT current_sequence FROM undo_pointer WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(ptr, 1);
    }

    #[test]
    fn test_undo_create() {
        let mut db = setup_db();
        let topo = Topology::new("undo-test", "Will be undone");
        let after_json = topo.to_json().unwrap();
        let topo_id = topo.id.clone();

        db.transaction(|tx| {
            topo.insert(tx)?;
            record_event(
                tx,
                "topology.created",
                "topology",
                &topo_id,
                "Created topology 'undo-test'",
                None,
                Some(&after_json),
                &EventSource::User,
            )
        })
        .unwrap();

        // Verify topology exists
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM topologies", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // Undo
        let summary = undo(&mut db).unwrap();
        assert_eq!(summary, "Created topology 'undo-test'");

        // Verify topology is gone
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM topologies", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        // Verify pointer is back to 0
        let ptr: i64 = db
            .conn()
            .query_row(
                "SELECT current_sequence FROM undo_pointer WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(ptr, 0);
    }

    #[test]
    fn test_redo_after_undo() {
        let mut db = setup_db();
        let topo = Topology::new("redo-test", "Will be undone then redone");
        let after_json = topo.to_json().unwrap();
        let topo_id = topo.id.clone();

        db.transaction(|tx| {
            topo.insert(tx)?;
            record_event(
                tx,
                "topology.created",
                "topology",
                &topo_id,
                "Created topology 'redo-test'",
                None,
                Some(&after_json),
                &EventSource::User,
            )
        })
        .unwrap();

        // Undo
        undo(&mut db).unwrap();

        // Verify gone
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM topologies", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        // Redo
        let summary = redo(&mut db).unwrap();
        assert_eq!(summary, "Created topology 'redo-test'");

        // Verify back
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM topologies", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // Verify pointer is back to 1
        let ptr: i64 = db
            .conn()
            .query_row(
                "SELECT current_sequence FROM undo_pointer WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(ptr, 1);
    }

    #[test]
    fn test_multi_level_undo() {
        let mut db = setup_db();

        // Create 3 topologies
        for i in 1..=3 {
            let topo = Topology::new(format!("topo-{}", i), format!("Topology {}", i));
            let after_json = topo.to_json().unwrap();
            let topo_id = topo.id.clone();

            db.transaction(|tx| {
                topo.insert(tx)?;
                record_event(
                    tx,
                    "topology.created",
                    "topology",
                    &topo_id,
                    &format!("Created topology 'topo-{}'", i),
                    None,
                    Some(&after_json),
                    &EventSource::User,
                )
            })
            .unwrap();
        }

        // Verify 3 topologies
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM topologies", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3);

        // Undo all 3
        undo(&mut db).unwrap(); // undo topo-3
        undo(&mut db).unwrap(); // undo topo-2
        undo(&mut db).unwrap(); // undo topo-1

        // Verify all gone
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM topologies", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);

        // Redo all 3
        redo(&mut db).unwrap(); // redo topo-1
        redo(&mut db).unwrap(); // redo topo-2
        redo(&mut db).unwrap(); // redo topo-3

        // Verify all back
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM topologies", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_redo_stack_cleared() {
        let mut db = setup_db();

        // Create 2 topologies
        for i in 1..=2 {
            let topo = Topology::new(format!("topo-{}", i), format!("Topology {}", i));
            let after_json = topo.to_json().unwrap();
            let topo_id = topo.id.clone();

            db.transaction(|tx| {
                topo.insert(tx)?;
                record_event(
                    tx,
                    "topology.created",
                    "topology",
                    &topo_id,
                    &format!("Created topology 'topo-{}'", i),
                    None,
                    Some(&after_json),
                    &EventSource::User,
                )
            })
            .unwrap();
        }

        // Undo topo-2
        undo(&mut db).unwrap();

        // Create a new topology (should clear redo stack)
        let topo3 = Topology::new("topo-3", "New topology after undo");
        let after_json = topo3.to_json().unwrap();
        let topo3_id = topo3.id.clone();

        db.transaction(|tx| {
            topo3.insert(tx)?;
            record_event(
                tx,
                "topology.created",
                "topology",
                &topo3_id,
                "Created topology 'topo-3'",
                None,
                Some(&after_json),
                &EventSource::User,
            )
        })
        .unwrap();

        // Try to redo -- should fail because redo stack was cleared
        let result = redo(&mut db);
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("Nothing to redo"),
            "Expected 'Nothing to redo' error"
        );

        // Verify: topo-1 and topo-3 exist, topo-2 does not
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM topologies", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);

        // Verify event count: should be 2 (topo-1 created, topo-3 created; topo-2 was cleared)
        let event_count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(event_count, 2);
    }

    #[test]
    fn test_undo_nothing() {
        let mut db = setup_db();
        let result = undo(&mut db);
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("Nothing to undo"),
            "Expected 'Nothing to undo' error"
        );
    }

    #[test]
    fn test_redo_nothing() {
        let mut db = setup_db();
        let result = redo(&mut db);
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("Nothing to redo"),
            "Expected 'Nothing to redo' error"
        );
    }
}
