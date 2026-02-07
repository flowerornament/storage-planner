//! sp topology -- Manage storage topologies (named configurations)
//!
//! Subcommands: create, list, show, set-active, delete
//! All mutating commands log events for undo/redo support.

use anyhow::{bail, Result};
use clap::Subcommand;

use crate::core::db::Database;
use crate::core::events::{record_event, EventSource};
use crate::core::models::Topology;

use super::OutputFormat;

#[derive(Subcommand)]
pub enum TopologyCommands {
    /// Create a new topology
    Create {
        /// Name for the topology (must be unique)
        name: String,

        /// Optional description
        #[arg(long, default_value = "")]
        description: String,
    },

    /// List all topologies
    List,

    /// Show details of a topology
    Show {
        /// Topology name
        name: String,
    },

    /// Set a topology as the active topology
    SetActive {
        /// Topology name to activate
        name: String,
    },

    /// Delete a topology and all its contents
    Delete {
        /// Topology name to delete
        name: String,
    },
}

pub fn run(cmd: TopologyCommands, db: &mut Database, format: OutputFormat) -> Result<()> {
    match cmd {
        TopologyCommands::Create { name, description } => create(db, &name, &description, format),
        TopologyCommands::List => list(db, format),
        TopologyCommands::Show { name } => show(db, &name, format),
        TopologyCommands::SetActive { name } => set_active(db, &name),
        TopologyCommands::Delete { name } => delete(db, &name),
    }
}

fn create(db: &mut Database, name: &str, description: &str, format: OutputFormat) -> Result<()> {
    let topo = Topology::new(name, description);
    let after_json = topo.to_json()?;
    let topo_id = topo.id.clone();
    let topo_name = topo.name.clone();

    db.transaction(|tx| {
        // Check if this is the first topology -- if so, make it active
        let count: i64 =
            tx.query_row("SELECT COUNT(*) FROM topologies", [], |row| row.get(0))?;

        let mut topo = topo;
        if count == 0 {
            topo.is_active = true;
        }

        // Re-compute after_state with potentially updated is_active
        let after_json = if count == 0 {
            topo.to_json()?
        } else {
            after_json.clone()
        };

        topo.insert(tx)?;

        record_event(
            tx,
            "topology.created",
            "topology",
            &topo_id,
            &format!("Created topology '{}'", topo_name),
            None,
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!("Created topology '{}'", name);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "created",
                "topology": name,
                "id": topo_id,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn list(db: &mut Database, format: OutputFormat) -> Result<()> {
    let mut stmt = db.conn().prepare(
        "SELECT id, name, description, parent_id, is_active, created_at, updated_at FROM topologies ORDER BY name",
    )?;

    let topologies: Vec<Topology> = stmt
        .query_map([], Topology::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Text => {
            if topologies.is_empty() {
                println!("No topologies found. Create one with 'sp topology create <name>'");
            } else {
                for topo in &topologies {
                    let active = if topo.is_active { " (active)" } else { "" };
                    let desc = if topo.description.is_empty() {
                        String::new()
                    } else {
                        format!(" - {}", topo.description)
                    };
                    println!("  {}{}{}", topo.name, active, desc);
                }
            }
        }
        OutputFormat::Json => {
            let json: Vec<serde_json::Value> = topologies
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "id": t.id,
                        "name": t.name,
                        "description": t.description,
                        "is_active": t.is_active,
                        "created_at": t.created_at.to_rfc3339(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn show(db: &mut Database, name: &str, format: OutputFormat) -> Result<()> {
    let topo: Topology = db
        .conn()
        .query_row(
            "SELECT id, name, description, parent_id, is_active, created_at, updated_at FROM topologies WHERE name = ?1",
            [name],
            Topology::from_row,
        )
        .map_err(|_| anyhow::anyhow!("Topology '{}' not found", name))?;

    match format {
        OutputFormat::Text => {
            let active = if topo.is_active { "yes" } else { "no" };
            println!("Topology: {}", topo.name);
            println!("  ID:          {}", topo.id);
            println!("  Description: {}", topo.description);
            println!("  Active:      {}", active);
            println!("  Created:     {}", topo.created_at.format("%Y-%m-%d %H:%M:%S"));

            // Count child entities
            let node_count: i64 = db.conn().query_row(
                "SELECT COUNT(*) FROM nodes WHERE topology_id = ?1",
                [&topo.id],
                |row| row.get(0),
            )?;
            let volume_count: i64 = db.conn().query_row(
                "SELECT COUNT(*) FROM volumes WHERE topology_id = ?1",
                [&topo.id],
                |row| row.get(0),
            )?;
            let dataset_count: i64 = db.conn().query_row(
                "SELECT COUNT(*) FROM datasets WHERE topology_id = ?1",
                [&topo.id],
                |row| row.get(0),
            )?;

            println!("  Nodes:       {}", node_count);
            println!("  Volumes:     {}", volume_count);
            println!("  Datasets:    {}", dataset_count);
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&topo)?);
        }
    }

    Ok(())
}

fn set_active(db: &mut Database, name: &str) -> Result<()> {
    db.transaction(|tx| {
        // Find the topology
        let topo: Topology = tx
            .query_row(
                "SELECT id, name, description, parent_id, is_active, created_at, updated_at FROM topologies WHERE name = ?1",
                [name],
                Topology::from_row,
            )
            .map_err(|_| anyhow::anyhow!("Topology '{}' not found", name))?;

        if topo.is_active {
            bail!("Topology '{}' is already active", name);
        }

        let before_json = topo.to_json()?;

        // Deactivate all topologies
        tx.execute("UPDATE topologies SET is_active = 0", [])?;

        // Activate the target
        tx.execute(
            "UPDATE topologies SET is_active = 1, updated_at = datetime('now') WHERE id = ?1",
            [&topo.id],
        )?;

        // Build after state
        let mut after = topo.clone();
        after.is_active = true;
        let after_json = after.to_json()?;

        record_event(
            tx,
            "topology.updated",
            "topology",
            &topo.id,
            &format!("Set topology '{}' as active", name),
            Some(&before_json),
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    println!("Set topology '{}' as active", name);
    Ok(())
}

fn delete(db: &mut Database, name: &str) -> Result<()> {
    db.transaction(|tx| {
        // Find the topology
        let topo: Topology = tx
            .query_row(
                "SELECT id, name, description, parent_id, is_active, created_at, updated_at FROM topologies WHERE name = ?1",
                [name],
                Topology::from_row,
            )
            .map_err(|_| anyhow::anyhow!("Topology '{}' not found", name))?;

        let before_json = topo.to_json()?;

        // Delete (cascades to nodes, volumes, etc.)
        tx.execute("DELETE FROM topologies WHERE id = ?1", [&topo.id])?;

        record_event(
            tx,
            "topology.deleted",
            "topology",
            &topo.id,
            &format!("Deleted topology '{}'", name),
            Some(&before_json),
            None,
            &EventSource::User,
        )?;

        Ok(())
    })?;

    println!("Deleted topology '{}'", name);
    Ok(())
}
