//! sp topology -- Manage storage topologies (named configurations)
//!
//! Subcommands: create, list, show, update, set-active, delete
//! All mutating commands log events for undo/redo support.
//! All lookups support name-or-ID resolution via the entity resolver.

use anyhow::{bail, Result};
use clap::Subcommand;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::events::{record_event, EventSource};
use crate::core::models::{Node, Topology, Volume};
use crate::core::resolve::{resolve_topology, validate_slug};
use crate::core::specs::Capacity;

use super::OutputFormat;

#[derive(Subcommand)]
pub enum TopologyCommands {
    /// Create a new topology
    Create {
        /// Name for the topology (must be unique, slug-like)
        name: String,

        /// Optional description
        #[arg(long, default_value = "")]
        description: String,
    },

    /// List all topologies
    List,

    /// Show details of a topology
    Show {
        /// Topology name or ID prefix
        name: String,

        /// Display hierarchical tree of nodes and volumes
        #[arg(long)]
        tree: bool,
    },

    /// Update a topology's name or description
    Update {
        /// Topology name or ID prefix
        name: String,

        /// New description
        #[arg(long)]
        description: Option<String>,

        /// Rename the topology (must be a valid slug)
        #[arg(long)]
        rename: Option<String>,
    },

    /// Set a topology as the active topology (deprecated: use 'tag' instead)
    SetActive {
        /// Topology name or ID prefix to activate
        name: String,
    },

    /// Tag a topology with a lifecycle state
    Tag {
        /// Topology name or ID prefix
        name: String,
        /// Tag to apply: current, exploring, or archived
        tag: String,
    },

    /// Remove a topology's tag
    Untag {
        /// Topology name or ID prefix
        name: String,
    },

    /// Delete a topology and all its contents
    Delete {
        /// Topology name or ID prefix to delete
        name: String,
    },
}

pub fn run(cmd: TopologyCommands, db: &mut Database, format: OutputFormat) -> Result<()> {
    match cmd {
        TopologyCommands::Create { name, description } => create(db, &name, &description, format),
        TopologyCommands::List => list(db, format),
        TopologyCommands::Show { name, tree } => show(db, &name, tree, format),
        TopologyCommands::Update {
            name,
            description,
            rename,
        } => update(db, &name, description.as_deref(), rename.as_deref(), format),
        TopologyCommands::SetActive { name } => set_active(db, &name),
        TopologyCommands::Tag {
            name,
            tag: tag_value,
        } => tag(db, &name, &tag_value, format),
        TopologyCommands::Untag { name } => untag(db, &name, format),
        TopologyCommands::Delete { name } => delete(db, &name),
    }
}

fn create(db: &mut Database, name: &str, description: &str, format: OutputFormat) -> Result<()> {
    validate_slug(name)?;

    let topo = Topology::new(name, description);
    let after_json = topo.to_json()?;
    let topo_id = topo.id.clone();
    let topo_name = topo.name.clone();

    db.transaction(|tx| {
        // Check if this is the first topology -- if so, tag it as current
        let count: i64 = tx.query_row("SELECT COUNT(*) FROM topologies", [], |row| row.get(0))?;

        let mut topo = topo;
        if count == 0 {
            topo.tag = Some("current".to_string());
        }

        // Re-compute after_state with potentially updated tag
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

    let id_prefix = &topo_id[..8];
    match format {
        OutputFormat::Text => {
            println!("Created topology '{}' (id: {})", name, id_prefix);
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
        "SELECT id, name, description, parent_id, tag, created_at, updated_at FROM topologies ORDER BY name",
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
                    let tag_str = topo
                        .tag
                        .as_ref()
                        .map(|t| format!(" [{}]", t))
                        .unwrap_or_default();
                    let desc = if topo.description.is_empty() {
                        String::new()
                    } else {
                        format!(" - {}", topo.description)
                    };
                    println!("  {}{}{}", topo.name, tag_str, desc);
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
                        "tag": t.tag,
                        "created_at": t.created_at.to_rfc3339(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn show(db: &mut Database, name: &str, tree: bool, format: OutputFormat) -> Result<()> {
    let topo = resolve_topology(db, name)?;

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

    // Get parent info and fork count
    let parent_name: Option<String> = topo.parent_id.as_ref().and_then(|pid| {
        db.conn()
            .query_row("SELECT name FROM topologies WHERE id = ?1", [pid], |row| {
                row.get(0)
            })
            .ok()
    });
    let fork_count: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM topologies WHERE parent_id = ?1",
        [&topo.id],
        |row| row.get(0),
    )?;

    match format {
        OutputFormat::Text => {
            let tag_str = topo
                .tag
                .as_ref()
                .map(|t| format!(" [{}]", t))
                .unwrap_or_default();
            println!("Topology: {}{}", topo.name, tag_str);
            println!("  Description: {}", topo.description);
            if let Some(ref pname) = parent_name {
                println!("  Forked from: {}", pname);
            }
            if fork_count > 0 {
                println!("  Forks: {}", fork_count);
            }
            println!(
                "  Nodes: {} | Volumes: {} | Datasets: {}",
                node_count, volume_count, dataset_count
            );

            if tree {
                println!();
                show_tree_text(db, &topo.id)?;
            } else {
                println!("  ID:          {}", topo.id);
                println!(
                    "  Created:     {}",
                    topo.created_at.format("%Y-%m-%d %H:%M:%S")
                );
            }
        }
        OutputFormat::Json => {
            if tree {
                let tree_json = build_tree_json(db, &topo)?;
                println!("{}", serde_json::to_string_pretty(&tree_json)?);
            } else {
                println!("{}", serde_json::to_string_pretty(&topo)?);
            }
        }
    }

    Ok(())
}

/// Display the tree view of a topology in text mode.
fn show_tree_text(db: &Database, topology_id: &str) -> Result<()> {
    let mut node_stmt = db.conn().prepare(
        "SELECT id, topology_id, name, role, location, available_bays, interface_types, \
         power_draw_watts, created_at, updated_at \
         FROM nodes WHERE topology_id = ?1 ORDER BY name",
    )?;
    let nodes: Vec<Node> = node_stmt
        .query_map(params![topology_id], Node::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    if nodes.is_empty() {
        println!("  (no nodes)");
        return Ok(());
    }

    for node in &nodes {
        let location = if node.location.is_empty() {
            String::new()
        } else {
            format!(" ({})", node.location)
        };
        println!("  {} [{}]{}", node.name, node.role, location);

        // Get volumes for this node
        let mut vol_stmt = db.conn().prepare(
            "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, \
             filesystem, raid_level, pool_type, item_id, created_at, updated_at \
             FROM volumes WHERE node_id = ?1 ORDER BY name",
        )?;
        let volumes: Vec<Volume> = vol_stmt
            .query_map(params![node.id], Volume::from_row)?
            .collect::<Result<Vec<_>, _>>()?;

        for vol in &volumes {
            let cap = Capacity::from_bytes(vol.capacity_bytes as u64);
            let fs = vol.filesystem.as_deref().unwrap_or("");
            let raid = vol
                .raid_level
                .as_ref()
                .map(|r| format!("/{}", r))
                .unwrap_or_default();
            println!("    {}: {} {}{}", vol.name, cap, fs, raid);
        }
    }

    Ok(())
}

/// Build the tree JSON structure for a topology.
fn build_tree_json(db: &Database, topo: &Topology) -> Result<serde_json::Value> {
    let mut node_stmt = db.conn().prepare(
        "SELECT id, topology_id, name, role, location, available_bays, interface_types, \
         power_draw_watts, created_at, updated_at \
         FROM nodes WHERE topology_id = ?1 ORDER BY name",
    )?;
    let nodes: Vec<Node> = node_stmt
        .query_map(params![topo.id], Node::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    let mut node_json_list = Vec::new();
    for node in &nodes {
        let mut vol_stmt = db.conn().prepare(
            "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, \
             filesystem, raid_level, pool_type, item_id, created_at, updated_at \
             FROM volumes WHERE node_id = ?1 ORDER BY name",
        )?;
        let volumes: Vec<Volume> = vol_stmt
            .query_map(params![node.id], Volume::from_row)?
            .collect::<Result<Vec<_>, _>>()?;

        let vol_json: Vec<serde_json::Value> = volumes
            .iter()
            .map(|v| serde_json::to_value(v).unwrap_or_default())
            .collect();

        let mut node_val = serde_json::to_value(node)?;
        if let serde_json::Value::Object(ref mut map) = node_val {
            map.insert("volumes".to_string(), serde_json::Value::Array(vol_json));
        }
        node_json_list.push(node_val);
    }

    // Get datasets for this topology
    let mut ds_stmt = db.conn().prepare(
        "SELECT id, topology_id, name, size_bytes, growth_rate_bytes_month, criticality, \
         min_copies, min_locations, max_rpo_hours, created_at, updated_at \
         FROM datasets WHERE topology_id = ?1 ORDER BY name",
    )?;
    let datasets: Vec<crate::core::models::Dataset> = ds_stmt
        .query_map(params![topo.id], crate::core::models::Dataset::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    let ds_json: Vec<serde_json::Value> = datasets
        .iter()
        .map(|d| serde_json::to_value(d).unwrap_or_default())
        .collect();

    let mut topo_val = serde_json::to_value(topo)?;
    if let serde_json::Value::Object(ref mut map) = topo_val {
        map.insert(
            "nodes".to_string(),
            serde_json::Value::Array(node_json_list),
        );
        map.insert("datasets".to_string(), serde_json::Value::Array(ds_json));
    }

    Ok(topo_val)
}

fn update(
    db: &mut Database,
    name: &str,
    description: Option<&str>,
    rename: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    if description.is_none() && rename.is_none() {
        bail!("Nothing to update. Provide --description or --rename.");
    }

    // Validate new name if renaming
    if let Some(new_name) = rename {
        validate_slug(new_name)?;
    }

    // Resolve outside transaction
    let topo = resolve_topology(db, name)?;
    let before_json = topo.to_json()?;
    let topo_id = topo.id.clone();
    let original_name = topo.name.clone();

    // Check uniqueness of new name if renaming
    if let Some(new_name) = rename {
        if new_name != original_name {
            let existing: i64 = db.conn().query_row(
                "SELECT COUNT(*) FROM topologies WHERE name = ?1 AND id != ?2",
                params![new_name, topo_id],
                |row| row.get(0),
            )?;
            if existing > 0 {
                bail!("Topology name '{}' is already taken", new_name);
            }
        }
    }

    let final_name = rename.unwrap_or(&original_name).to_string();
    let final_desc = description
        .map(|d| d.to_string())
        .unwrap_or_else(|| topo.description.clone());

    db.transaction(|tx| {
        if let Some(new_name) = rename {
            tx.execute(
                "UPDATE topologies SET name = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![new_name, topo_id],
            )?;
        }

        if let Some(desc) = description {
            tx.execute(
                "UPDATE topologies SET description = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![desc, topo_id],
            )?;
        }

        // Build after state
        let mut after = topo.clone();
        after.name = final_name.clone();
        after.description = final_desc.clone();
        let after_json = after.to_json()?;

        record_event(
            tx,
            "topology.updated",
            "topology",
            &topo_id,
            &format!("Updated topology '{}'", original_name),
            Some(&before_json),
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            if rename.is_some() && description.is_some() {
                println!(
                    "Updated topology '{}': renamed to '{}', description updated",
                    original_name, final_name
                );
            } else if rename.is_some() {
                println!(
                    "Updated topology '{}': renamed to '{}'",
                    original_name, final_name
                );
            } else {
                println!("Updated topology '{}': description updated", original_name);
            }
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "updated",
                "topology": final_name,
                "id": topo_id,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn set_active(db: &mut Database, name: &str) -> Result<()> {
    // Resolve outside transaction
    let topo = resolve_topology(db, name)?;
    let topo_name = topo.name.clone();

    if topo.tag.as_deref() == Some("current") {
        bail!("Topology '{}' is already the current topology", topo_name);
    }

    let before_json = topo.to_json()?;
    let topo_id = topo.id.clone();

    db.transaction(|tx| {
        // Clear any existing current tag
        tx.execute(
            "UPDATE topologies SET tag = NULL, updated_at = datetime('now') WHERE tag = 'current'",
            [],
        )?;

        // Tag the target as current
        tx.execute(
            "UPDATE topologies SET tag = 'current', updated_at = datetime('now') WHERE id = ?1",
            [&topo_id],
        )?;

        // Build after state
        let mut after = topo.clone();
        after.tag = Some("current".to_string());
        let after_json = after.to_json()?;

        record_event(
            tx,
            "topology.updated",
            "topology",
            &topo_id,
            &format!("Set topology '{}' as current", topo_name),
            Some(&before_json),
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    eprintln!(
        "Note: 'set-active' is deprecated. Use 'sp topology tag {} current' instead.",
        topo_name
    );
    println!("Set topology '{}' as current", topo_name);
    Ok(())
}

fn tag(db: &mut Database, name: &str, tag_value: &str, format: OutputFormat) -> Result<()> {
    // Validate tag value
    match tag_value {
        "current" | "exploring" | "archived" => {}
        _ => bail!(
            "Invalid tag '{}'. Must be one of: current, exploring, archived",
            tag_value
        ),
    }

    // Resolve outside transaction
    let topo = resolve_topology(db, name)?;
    let before_json = topo.to_json()?;
    let topo_id = topo.id.clone();
    let topo_name = topo.name.clone();

    db.transaction(|tx| {
        // If tagging as "current", first clear any existing current
        if tag_value == "current" {
            tx.execute(
                "UPDATE topologies SET tag = NULL, updated_at = datetime('now') WHERE tag = 'current'",
                [],
            )?;
        }

        // Set the tag
        tx.execute(
            "UPDATE topologies SET tag = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![tag_value, topo_id],
        )?;

        // Build after state
        let mut after = topo.clone();
        after.tag = Some(tag_value.to_string());
        let after_json = after.to_json()?;

        record_event(
            tx,
            "topology.updated",
            "topology",
            &topo_id,
            &format!("Tagged topology '{}' as '{}'", topo_name, tag_value),
            Some(&before_json),
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!("Tagged topology '{}' as [{}]", topo_name, tag_value);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "tagged",
                "topology": topo_name,
                "id": topo_id,
                "tag": tag_value,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn untag(db: &mut Database, name: &str, format: OutputFormat) -> Result<()> {
    // Resolve outside transaction
    let topo = resolve_topology(db, name)?;
    let topo_name = topo.name.clone();

    if topo.tag.is_none() {
        bail!("Topology '{}' has no tag", topo_name);
    }

    let before_json = topo.to_json()?;
    let topo_id = topo.id.clone();
    let old_tag = topo.tag.clone().unwrap();

    db.transaction(|tx| {
        // Clear the tag
        tx.execute(
            "UPDATE topologies SET tag = NULL, updated_at = datetime('now') WHERE id = ?1",
            [&topo_id],
        )?;

        // Build after state
        let mut after = topo.clone();
        after.tag = None;
        let after_json = after.to_json()?;

        record_event(
            tx,
            "topology.updated",
            "topology",
            &topo_id,
            &format!("Removed tag '{}' from topology '{}'", old_tag, topo_name),
            Some(&before_json),
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!("Removed tag [{}] from topology '{}'", old_tag, topo_name);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "untagged",
                "topology": topo_name,
                "id": topo_id,
                "previous_tag": old_tag,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn delete(db: &mut Database, name: &str) -> Result<()> {
    // Resolve outside transaction
    let topo = resolve_topology(db, name)?;
    let topo_name = topo.name.clone();
    let before_json = topo.to_json()?;
    let topo_id = topo.id.clone();

    db.transaction(|tx| {
        // Delete (cascades to nodes, volumes, etc.)
        tx.execute("DELETE FROM topologies WHERE id = ?1", [&topo_id])?;

        record_event(
            tx,
            "topology.deleted",
            "topology",
            &topo_id,
            &format!("Deleted topology '{}'", topo_name),
            Some(&before_json),
            None,
            &EventSource::User,
        )?;

        Ok(())
    })?;

    println!("Deleted topology '{}'", topo_name);
    Ok(())
}
