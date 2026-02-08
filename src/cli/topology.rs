//! sp topology -- Manage storage topologies (named configurations)
//!
//! Subcommands: create, list, show, update, set-active, delete, diff, tree, log
//! All mutating commands log events for undo/redo support.
//! All lookups support name-or-ID resolution via the entity resolver.

use std::collections::HashMap;

use anyhow::{bail, Result};
use clap::Subcommand;
use console::style;
use rusqlite::params;
use serde_json::Value;
use uuid::Uuid;

use crate::core::db::Database;
use crate::core::events::{record_event, EventSource};
use crate::core::models::{Dataset, Link, Node, Placement, SyncRegime, Topology, Volume};
use crate::core::resolve::{resolve_active_topology, resolve_topology, validate_slug};
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

    /// Fork a topology (deep copy with new IDs)
    Fork {
        /// Source topology name or ID prefix to fork from
        source: String,

        /// Optional name for the fork (auto-generated if omitted)
        #[arg(long)]
        name: Option<String>,
    },

    /// Compare two topologies showing entity-level and field-level changes
    Diff {
        /// Target topology to compare (shows what changed TO this topology)
        target: String,
        /// Base topology (defaults to current/active topology if omitted)
        base: Option<String>,
        /// Only diff nodes
        #[arg(long)]
        nodes: bool,
        /// Only diff volumes
        #[arg(long)]
        volumes: bool,
        /// Only diff datasets
        #[arg(long)]
        datasets: bool,
        /// Only diff placements
        #[arg(long)]
        placements: bool,
        /// Only diff links
        #[arg(long)]
        links: bool,
        /// Only diff sync regimes
        #[arg(long)]
        syncs: bool,
    },

    /// Show fork tree of all topologies
    Tree,

    /// Show ancestry of a specific topology
    Log {
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
        TopologyCommands::Fork { source, name } => fork(db, &source, name.as_deref(), format),
        TopologyCommands::Diff {
            target,
            base,
            nodes,
            volumes,
            datasets,
            placements,
            links,
            syncs,
        } => diff(
            db,
            &target,
            base.as_deref(),
            nodes,
            volumes,
            datasets,
            placements,
            links,
            syncs,
            format,
        ),
        TopologyCommands::Tree => tree(db, format),
        TopologyCommands::Log { name } => log(db, &name, format),
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
         power_draw_watts, cost_estimate, noise_db, rack_units, item_id, created_at, updated_at \
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
         power_draw_watts, cost_estimate, noise_db, rack_units, item_id, created_at, updated_at \
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

fn generate_fork_name(db: &Database, source_name: &str) -> Result<String> {
    for n in 1..100 {
        let candidate = format!("{}-fork-{}", source_name, n);
        let exists: i64 = db.conn().query_row(
            "SELECT COUNT(*) FROM topologies WHERE name = ?1",
            params![candidate],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Ok(candidate);
        }
    }
    bail!(
        "Could not generate fork name for '{}' (tried 99 suffixes)",
        source_name
    );
}

fn fork(
    db: &mut Database,
    source_name: &str,
    fork_name: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    // Resolve source topology (can be ANY topology, not just active)
    let source = resolve_topology(db, source_name)?;

    // Determine fork name
    let name = match fork_name {
        Some(n) => {
            validate_slug(n)?;
            // Check uniqueness
            let exists: i64 = db.conn().query_row(
                "SELECT COUNT(*) FROM topologies WHERE name = ?1",
                params![n],
                |row| row.get(0),
            )?;
            if exists > 0 {
                bail!("Topology name '{}' already exists", n);
            }
            n.to_string()
        }
        None => generate_fork_name(db, &source.name)?,
    };

    // Build ID remapping tables
    let mut node_map: HashMap<String, String> = HashMap::new();
    let mut volume_map: HashMap<String, String> = HashMap::new();
    let mut dataset_map: HashMap<String, String> = HashMap::new();

    let new_topo_id = Uuid::new_v4().to_string();
    let fork_name_clone = name.clone();
    let source_name_clone = source.name.clone();
    let source_id = source.id.clone();

    // Load all entities from source BEFORE the transaction (D009 pattern)
    // Each block scopes the prepared statement so borrows are dropped before transaction
    // Load all entities from source BEFORE the transaction (D009 pattern)
    // Each block scopes the prepared statement so borrows are dropped before transaction
    let nodes: Vec<Node> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, name, role, location, available_bays, interface_types, \
             power_draw_watts, cost_estimate, noise_db, rack_units, item_id, created_at, updated_at FROM nodes WHERE topology_id = ?1",
        )?;
        let result = stmt
            .query_map(params![source_id], Node::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    let volumes: Vec<Volume> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, filesystem, \
             raid_level, pool_type, item_id, created_at, updated_at \
             FROM volumes WHERE topology_id = ?1",
        )?;
        let result = stmt
            .query_map(params![source_id], Volume::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    let datasets: Vec<Dataset> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, name, size_bytes, growth_rate_bytes_month, criticality, \
             min_copies, min_locations, max_rpo_hours, created_at, updated_at \
             FROM datasets WHERE topology_id = ?1",
        )?;
        let result = stmt
            .query_map(params![source_id], Dataset::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    let placements: Vec<Placement> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, dataset_id, volume_id, role, priority, created_at \
             FROM placements WHERE topology_id = ?1",
        )?;
        let result = stmt
            .query_map(params![source_id], Placement::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    let links: Vec<Link> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, source_node_id, target_node_id, bandwidth_bytes_sec, \
             connection_type, latency_ms, is_metered, cost_per_gb_cents, created_at, updated_at \
             FROM links WHERE topology_id = ?1",
        )?;
        let result = stmt
            .query_map(params![source_id], Link::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    let sync_regimes: Vec<SyncRegime> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, name, dataset_id, source_volume_id, target_volume_id, \
             sync_type, schedule, direction, created_at, updated_at \
             FROM sync_regimes WHERE topology_id = ?1",
        )?;
        let result = stmt
            .query_map(params![source_id], SyncRegime::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    // Execute deep copy in a single transaction
    db.transaction(|tx| {
        // 1. Create new topology with parent_id = source.id
        let now = chrono::Utc::now();
        tx.execute(
            "INSERT INTO topologies (id, name, description, parent_id, tag, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6)",
            params![
                new_topo_id,
                fork_name_clone,
                source.description,
                source_id,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        // 2. Copy nodes (build node_map)
        for node in &nodes {
            let new_id = Uuid::new_v4().to_string();
            node_map.insert(node.id.clone(), new_id.clone());
            let mut new_node = node.clone();
            new_node.id = new_id;
            new_node.topology_id = new_topo_id.clone();
            new_node.created_at = now;
            new_node.updated_at = now;
            new_node.insert(tx)?;
        }

        // 3. Copy volumes (remap node_id using node_map, build volume_map)
        for vol in &volumes {
            let new_id = Uuid::new_v4().to_string();
            volume_map.insert(vol.id.clone(), new_id.clone());
            let mut new_vol = vol.clone();
            new_vol.id = new_id;
            new_vol.topology_id = new_topo_id.clone();
            new_vol.node_id = node_map
                .get(&vol.node_id)
                .ok_or_else(|| {
                    anyhow::anyhow!("Node ID {} not found in remap table", vol.node_id)
                })?
                .clone();
            new_vol.created_at = now;
            new_vol.updated_at = now;
            new_vol.insert(tx)?;
        }

        // 4. Copy datasets (build dataset_map)
        for ds in &datasets {
            let new_id = Uuid::new_v4().to_string();
            dataset_map.insert(ds.id.clone(), new_id.clone());
            let mut new_ds = ds.clone();
            new_ds.id = new_id;
            new_ds.topology_id = new_topo_id.clone();
            new_ds.created_at = now;
            new_ds.updated_at = now;
            new_ds.insert(tx)?;
        }

        // 5. Copy placements (remap dataset_id and volume_id)
        for pl in &placements {
            let new_id = Uuid::new_v4().to_string();
            let mut new_pl = pl.clone();
            new_pl.id = new_id;
            new_pl.topology_id = new_topo_id.clone();
            new_pl.dataset_id = dataset_map
                .get(&pl.dataset_id)
                .ok_or_else(|| {
                    anyhow::anyhow!("Dataset ID {} not found in remap table", pl.dataset_id)
                })?
                .clone();
            new_pl.volume_id = volume_map
                .get(&pl.volume_id)
                .ok_or_else(|| {
                    anyhow::anyhow!("Volume ID {} not found in remap table", pl.volume_id)
                })?
                .clone();
            new_pl.created_at = now;
            new_pl.insert(tx)?;
        }

        // 6. Copy links (remap source_node_id and target_node_id)
        for link in &links {
            let new_id = Uuid::new_v4().to_string();
            let mut new_link = link.clone();
            new_link.id = new_id;
            new_link.topology_id = new_topo_id.clone();
            new_link.source_node_id = node_map
                .get(&link.source_node_id)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Source node ID {} not found in remap table",
                        link.source_node_id
                    )
                })?
                .clone();
            new_link.target_node_id = node_map
                .get(&link.target_node_id)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Target node ID {} not found in remap table",
                        link.target_node_id
                    )
                })?
                .clone();
            new_link.created_at = now;
            new_link.updated_at = now;
            new_link.insert(tx)?;
        }

        // 7. Copy sync regimes (remap dataset_id, source_volume_id, target_volume_id)
        for sr in &sync_regimes {
            let new_id = Uuid::new_v4().to_string();
            let mut new_sr = sr.clone();
            new_sr.id = new_id;
            new_sr.topology_id = new_topo_id.clone();
            new_sr.dataset_id = dataset_map
                .get(&sr.dataset_id)
                .ok_or_else(|| {
                    anyhow::anyhow!("Dataset ID {} not found in remap table", sr.dataset_id)
                })?
                .clone();
            new_sr.source_volume_id = volume_map
                .get(&sr.source_volume_id)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Source volume ID {} not found in remap table",
                        sr.source_volume_id
                    )
                })?
                .clone();
            new_sr.target_volume_id = volume_map
                .get(&sr.target_volume_id)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Target volume ID {} not found in remap table",
                        sr.target_volume_id
                    )
                })?
                .clone();
            new_sr.created_at = now;
            new_sr.updated_at = now;
            new_sr.insert(tx)?;
        }

        // 8. Record single fork event
        record_event(
            tx,
            "topology.created",
            "topology",
            &new_topo_id,
            &format!(
                "Forked topology '{}' from '{}'",
                fork_name_clone, source_name_clone
            ),
            None,
            None,
            &EventSource::User,
        )?;

        Ok(())
    })?;

    // Output
    let entity_count = nodes.len()
        + volumes.len()
        + datasets.len()
        + placements.len()
        + links.len()
        + sync_regimes.len();
    match format {
        OutputFormat::Text => {
            println!(
                "Forked '{}' from '{}' ({} entities copied)",
                name, source_name_clone, entity_count
            );
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "forked",
                "topology": name,
                "source": source_name_clone,
                "id": new_topo_id,
                "entities_copied": entity_count,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Diff engine
// ---------------------------------------------------------------------------

/// Skip metadata fields that differ between forks by definition
const DIFF_SKIP_FIELDS: &[&str] = &[
    "id",
    "topology_id",
    "node_id",
    "dataset_id",
    "volume_id",
    "source_node_id",
    "target_node_id",
    "source_volume_id",
    "target_volume_id",
    "created_at",
    "updated_at",
];

#[derive(Debug)]
enum DiffEntry {
    Added(String, Value),
    Removed(String, Value),
    Changed(String, Vec<FieldDiff>),
}

#[derive(Debug)]
struct FieldDiff {
    field: String,
    old_value: Value,
    new_value: Value,
}

/// Format a JSON value for human-readable diff output
fn format_diff_value(v: &Value) -> String {
    match v {
        Value::Null => "(none)".to_string(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        _ => v.to_string(),
    }
}

/// Compare two JSON objects field-by-field, skipping DIFF_SKIP_FIELDS
fn diff_json_fields(left: &Value, right: &Value) -> Vec<FieldDiff> {
    let mut diffs = Vec::new();

    let left_obj = match left.as_object() {
        Some(o) => o,
        None => return diffs,
    };
    let right_obj = match right.as_object() {
        Some(o) => o,
        None => return diffs,
    };

    // Check all keys from both sides
    let mut all_keys: Vec<&String> = left_obj.keys().collect();
    for k in right_obj.keys() {
        if !left_obj.contains_key(k) {
            all_keys.push(k);
        }
    }
    all_keys.sort();

    for key in all_keys {
        if DIFF_SKIP_FIELDS.contains(&key.as_str()) {
            continue;
        }
        let left_val = left_obj.get(key).unwrap_or(&Value::Null);
        let right_val = right_obj.get(key).unwrap_or(&Value::Null);
        if left_val != right_val {
            diffs.push(FieldDiff {
                field: key.clone(),
                old_value: left_val.clone(),
                new_value: right_val.clone(),
            });
        }
    }

    diffs
}

/// Diff two sets of entities matched by display key
fn diff_entities_by_name(
    left_items: &[(String, Value)],
    right_items: &[(String, Value)],
) -> Vec<DiffEntry> {
    let left_map: HashMap<&str, &Value> = left_items.iter().map(|(k, v)| (k.as_str(), v)).collect();
    let right_map: HashMap<&str, &Value> =
        right_items.iter().map(|(k, v)| (k.as_str(), v)).collect();

    let mut entries = Vec::new();

    // Removed: in left but not right
    let mut left_keys: Vec<&&str> = left_map.keys().collect();
    left_keys.sort();
    for key in left_keys {
        if !right_map.contains_key(*key) {
            entries.push(DiffEntry::Removed(
                key.to_string(),
                (*left_map.get(*key).unwrap()).clone(),
            ));
        }
    }

    // Added: in right but not left
    let mut right_keys: Vec<&&str> = right_map.keys().collect();
    right_keys.sort();
    for key in right_keys {
        if !left_map.contains_key(*key) {
            entries.push(DiffEntry::Added(
                key.to_string(),
                (*right_map.get(*key).unwrap()).clone(),
            ));
        }
    }

    // Changed: in both, check field differences
    let mut common_keys: Vec<&&str> = left_map
        .keys()
        .filter(|k| right_map.contains_key(**k))
        .collect();
    common_keys.sort();
    for key in common_keys {
        let left_val = left_map.get(*key).unwrap();
        let right_val = right_map.get(*key).unwrap();
        let field_diffs = diff_json_fields(left_val, right_val);
        if !field_diffs.is_empty() {
            entries.push(DiffEntry::Changed(key.to_string(), field_diffs));
        }
    }

    entries
}

// ---------------------------------------------------------------------------
// Entity loading helpers for diff
// ---------------------------------------------------------------------------

fn load_nodes_for_diff(db: &Database, topology_id: &str) -> Result<Vec<(String, Value)>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, topology_id, name, role, location, available_bays, interface_types, \
         power_draw_watts, cost_estimate, noise_db, rack_units, item_id, created_at, updated_at \
         FROM nodes WHERE topology_id = ?1 ORDER BY name",
    )?;
    let nodes: Vec<Node> = stmt
        .query_map(params![topology_id], Node::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    nodes
        .into_iter()
        .map(|n| {
            let key = n.name.clone();
            let val = serde_json::to_value(&n)?;
            Ok((key, val))
        })
        .collect()
}

fn load_volumes_for_diff(db: &Database, topology_id: &str) -> Result<Vec<(String, Value)>> {
    let mut stmt = db.conn().prepare(
        "SELECT v.id, v.topology_id, v.node_id, v.name, v.capacity_bytes, v.usable_bytes, \
         v.filesystem, v.raid_level, v.pool_type, v.item_id, v.created_at, v.updated_at, \
         n.name as node_name \
         FROM volumes v JOIN nodes n ON v.node_id = n.id \
         WHERE v.topology_id = ?1 ORDER BY n.name, v.name",
    )?;
    let results: Vec<(String, Value)> = stmt
        .query_map(params![topology_id], |row| {
            let node_name: String = row.get("node_name")?;
            let vol = Volume::from_row(row)?;
            Ok((node_name, vol))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(node_name, vol)| {
            let key = format!("{}/{}", node_name, vol.name);
            let val = serde_json::to_value(&vol).unwrap_or_default();
            (key, val)
        })
        .collect();
    Ok(results)
}

fn load_datasets_for_diff(db: &Database, topology_id: &str) -> Result<Vec<(String, Value)>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, topology_id, name, size_bytes, growth_rate_bytes_month, criticality, \
         min_copies, min_locations, max_rpo_hours, created_at, updated_at \
         FROM datasets WHERE topology_id = ?1 ORDER BY name",
    )?;
    let datasets: Vec<Dataset> = stmt
        .query_map(params![topology_id], Dataset::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    datasets
        .into_iter()
        .map(|d| {
            let key = d.name.clone();
            let val = serde_json::to_value(&d)?;
            Ok((key, val))
        })
        .collect()
}

fn load_placements_for_diff(db: &Database, topology_id: &str) -> Result<Vec<(String, Value)>> {
    let mut stmt = db.conn().prepare(
        "SELECT p.id, p.topology_id, p.dataset_id, p.volume_id, p.role, p.priority, p.created_at, \
         d.name as dataset_name, n.name as node_name, v.name as volume_name \
         FROM placements p \
         JOIN datasets d ON p.dataset_id = d.id \
         JOIN volumes v ON p.volume_id = v.id \
         JOIN nodes n ON v.node_id = n.id \
         WHERE p.topology_id = ?1 ORDER BY d.name, n.name, v.name",
    )?;
    let results: Vec<(String, Value)> = stmt
        .query_map(params![topology_id], |row| {
            let dataset_name: String = row.get("dataset_name")?;
            let node_name: String = row.get("node_name")?;
            let volume_name: String = row.get("volume_name")?;
            let pl = Placement::from_row(row)?;
            Ok((dataset_name, node_name, volume_name, pl))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(dataset_name, node_name, volume_name, pl)| {
            let key = format!("{} on {}/{}", dataset_name, node_name, volume_name);
            let val = serde_json::to_value(&pl).unwrap_or_default();
            (key, val)
        })
        .collect();
    Ok(results)
}

fn load_links_for_diff(db: &Database, topology_id: &str) -> Result<Vec<(String, Value)>> {
    let mut stmt = db.conn().prepare(
        "SELECT l.id, l.topology_id, l.source_node_id, l.target_node_id, l.bandwidth_bytes_sec, \
         l.connection_type, l.latency_ms, l.is_metered, l.cost_per_gb_cents, l.created_at, l.updated_at, \
         sn.name as source_name, tn.name as target_name \
         FROM links l \
         JOIN nodes sn ON l.source_node_id = sn.id \
         JOIN nodes tn ON l.target_node_id = tn.id \
         WHERE l.topology_id = ?1 ORDER BY sn.name, tn.name",
    )?;
    let results: Vec<(String, Value)> = stmt
        .query_map(params![topology_id], |row| {
            let source_name: String = row.get("source_name")?;
            let target_name: String = row.get("target_name")?;
            let link = Link::from_row(row)?;
            Ok((source_name, target_name, link))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(source_name, target_name, link)| {
            let key = format!("{} -> {}", source_name, target_name);
            let val = serde_json::to_value(&link).unwrap_or_default();
            (key, val)
        })
        .collect();
    Ok(results)
}

fn load_syncs_for_diff(db: &Database, topology_id: &str) -> Result<Vec<(String, Value)>> {
    let mut stmt = db.conn().prepare(
        "SELECT id, topology_id, name, dataset_id, source_volume_id, target_volume_id, \
         sync_type, schedule, direction, created_at, updated_at \
         FROM sync_regimes WHERE topology_id = ?1 ORDER BY name",
    )?;
    let syncs: Vec<SyncRegime> = stmt
        .query_map(params![topology_id], SyncRegime::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    syncs
        .into_iter()
        .map(|s| {
            let key = s.name.clone();
            let val = serde_json::to_value(&s)?;
            Ok((key, val))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Diff output
// ---------------------------------------------------------------------------

/// Print a diff section in text mode and return counts (added, modified, removed)
fn print_diff_section(section_name: &str, entries: &[DiffEntry]) -> (usize, usize, usize) {
    let mut added = 0;
    let mut modified = 0;
    let mut removed = 0;

    println!("{}:", section_name);

    if entries.is_empty() {
        println!("  (no changes)");
        return (0, 0, 0);
    }

    for entry in entries {
        match entry {
            DiffEntry::Added(name, _) => {
                added += 1;
                println!(
                    "  {} {} {}",
                    style("+").green(),
                    style(name).green(),
                    style("[added]").green()
                );
            }
            DiffEntry::Removed(name, _) => {
                removed += 1;
                println!(
                    "  {} {} {}",
                    style("-").red(),
                    style(name).red(),
                    style("[removed]").red()
                );
            }
            DiffEntry::Changed(name, field_diffs) => {
                modified += 1;
                println!(
                    "  {} {} {}",
                    style("~").yellow(),
                    name,
                    style("[modified]").yellow()
                );
                for fd in field_diffs {
                    println!(
                        "      {}: {} -> {}",
                        fd.field,
                        style(format_diff_value(&fd.old_value)).red(),
                        style(format_diff_value(&fd.new_value)).green(),
                    );
                }
            }
        }
    }

    (added, modified, removed)
}

/// Build a JSON representation of a diff section
fn diff_section_to_json(entries: &[DiffEntry]) -> Value {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();

    for entry in entries {
        match entry {
            DiffEntry::Added(name, val) => {
                added.push(serde_json::json!({ "name": name, "entity": val }));
            }
            DiffEntry::Removed(name, val) => {
                removed.push(serde_json::json!({ "name": name, "entity": val }));
            }
            DiffEntry::Changed(name, field_diffs) => {
                let fields: Vec<Value> = field_diffs
                    .iter()
                    .map(|fd| {
                        serde_json::json!({
                            "field": fd.field,
                            "old": fd.old_value,
                            "new": fd.new_value,
                        })
                    })
                    .collect();
                changed.push(serde_json::json!({ "name": name, "changes": fields }));
            }
        }
    }

    serde_json::json!({
        "added": added,
        "removed": removed,
        "changed": changed,
    })
}

#[allow(clippy::too_many_arguments)]
fn diff(
    db: &mut Database,
    target_name: &str,
    base_name: Option<&str>,
    filter_nodes: bool,
    filter_volumes: bool,
    filter_datasets: bool,
    filter_placements: bool,
    filter_links: bool,
    filter_syncs: bool,
    format: OutputFormat,
) -> Result<()> {
    // Resolve topologies
    let target = resolve_topology(db, target_name)?;
    let base = match base_name {
        Some(name) => resolve_topology(db, name)?,
        None => resolve_active_topology(db, None).map_err(|_| {
            anyhow::anyhow!(
                "No base topology specified and no current topology set. \
                 Provide a base topology or tag one as current."
            )
        })?,
    };

    // Determine which entity types to diff
    let any_filter = filter_nodes
        || filter_volumes
        || filter_datasets
        || filter_placements
        || filter_links
        || filter_syncs;
    let diff_nodes = !any_filter || filter_nodes;
    let diff_volumes = !any_filter || filter_volumes;
    let diff_datasets = !any_filter || filter_datasets;
    let diff_placements = !any_filter || filter_placements;
    let diff_links = !any_filter || filter_links;
    let diff_syncs = !any_filter || filter_syncs;

    // Load and diff each entity type
    let node_entries = if diff_nodes {
        let base_nodes = load_nodes_for_diff(db, &base.id)?;
        let target_nodes = load_nodes_for_diff(db, &target.id)?;
        diff_entities_by_name(&base_nodes, &target_nodes)
    } else {
        Vec::new()
    };

    let volume_entries = if diff_volumes {
        let base_vols = load_volumes_for_diff(db, &base.id)?;
        let target_vols = load_volumes_for_diff(db, &target.id)?;
        diff_entities_by_name(&base_vols, &target_vols)
    } else {
        Vec::new()
    };

    let dataset_entries = if diff_datasets {
        let base_ds = load_datasets_for_diff(db, &base.id)?;
        let target_ds = load_datasets_for_diff(db, &target.id)?;
        diff_entities_by_name(&base_ds, &target_ds)
    } else {
        Vec::new()
    };

    let placement_entries = if diff_placements {
        let base_pl = load_placements_for_diff(db, &base.id)?;
        let target_pl = load_placements_for_diff(db, &target.id)?;
        diff_entities_by_name(&base_pl, &target_pl)
    } else {
        Vec::new()
    };

    let link_entries = if diff_links {
        let base_links = load_links_for_diff(db, &base.id)?;
        let target_links = load_links_for_diff(db, &target.id)?;
        diff_entities_by_name(&base_links, &target_links)
    } else {
        Vec::new()
    };

    let sync_entries = if diff_syncs {
        let base_syncs = load_syncs_for_diff(db, &base.id)?;
        let target_syncs = load_syncs_for_diff(db, &target.id)?;
        diff_entities_by_name(&base_syncs, &target_syncs)
    } else {
        Vec::new()
    };

    match format {
        OutputFormat::Text => {
            println!("Diff: {} -> {}", base.name, target.name);
            println!();

            let mut total_added = 0;
            let mut total_modified = 0;
            let mut total_removed = 0;

            if diff_nodes {
                let (a, m, r) = print_diff_section("Nodes", &node_entries);
                total_added += a;
                total_modified += m;
                total_removed += r;
                println!();
            }
            if diff_volumes {
                let (a, m, r) = print_diff_section("Volumes", &volume_entries);
                total_added += a;
                total_modified += m;
                total_removed += r;
                println!();
            }
            if diff_datasets {
                let (a, m, r) = print_diff_section("Datasets", &dataset_entries);
                total_added += a;
                total_modified += m;
                total_removed += r;
                println!();
            }
            if diff_placements {
                let (a, m, r) = print_diff_section("Placements", &placement_entries);
                total_added += a;
                total_modified += m;
                total_removed += r;
                println!();
            }
            if diff_links {
                let (a, m, r) = print_diff_section("Links", &link_entries);
                total_added += a;
                total_modified += m;
                total_removed += r;
                println!();
            }
            if diff_syncs {
                let (a, m, r) = print_diff_section("Sync Regimes", &sync_entries);
                total_added += a;
                total_modified += m;
                total_removed += r;
                println!();
            }

            // Summary
            let mut parts = Vec::new();
            if total_added > 0 {
                parts.push(format!("{} added", total_added));
            }
            if total_modified > 0 {
                parts.push(format!("{} modified", total_modified));
            }
            if total_removed > 0 {
                parts.push(format!("{} removed", total_removed));
            }
            if parts.is_empty() {
                println!("No differences found.");
            } else {
                println!("Summary: {}", parts.join(", "));
            }
        }
        OutputFormat::Json => {
            let mut json = serde_json::json!({
                "base": base.name,
                "target": target.name,
            });
            if let Value::Object(ref mut map) = json {
                if diff_nodes {
                    map.insert("nodes".to_string(), diff_section_to_json(&node_entries));
                }
                if diff_volumes {
                    map.insert("volumes".to_string(), diff_section_to_json(&volume_entries));
                }
                if diff_datasets {
                    map.insert(
                        "datasets".to_string(),
                        diff_section_to_json(&dataset_entries),
                    );
                }
                if diff_placements {
                    map.insert(
                        "placements".to_string(),
                        diff_section_to_json(&placement_entries),
                    );
                }
                if diff_links {
                    map.insert("links".to_string(), diff_section_to_json(&link_entries));
                }
                if diff_syncs {
                    map.insert(
                        "sync_regimes".to_string(),
                        diff_section_to_json(&sync_entries),
                    );
                }
            }
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tree and Log commands
// ---------------------------------------------------------------------------

fn tree(db: &mut Database, format: OutputFormat) -> Result<()> {
    let topologies: Vec<Topology> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, name, description, parent_id, tag, created_at, updated_at \
             FROM topologies ORDER BY name",
        )?;
        let result = stmt
            .query_map([], Topology::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    if topologies.is_empty() {
        println!("No topologies found.");
        return Ok(());
    }

    match format {
        OutputFormat::Text => {
            // Build parent_id -> children map
            let mut children: HashMap<Option<String>, Vec<&Topology>> = HashMap::new();
            for topo in &topologies {
                children
                    .entry(topo.parent_id.clone())
                    .or_default()
                    .push(topo);
            }

            // Find roots (parent_id = None)
            let roots = children.get(&None).cloned().unwrap_or_default();

            println!("Topologies:");
            for (i, root) in roots.iter().enumerate() {
                let is_last = i == roots.len() - 1;
                print_tree_node_lineage(root, &children, "", is_last, true);
            }
        }
        OutputFormat::Json => {
            // Build hierarchical JSON
            let topo_json: Vec<Value> = topologies
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "id": t.id,
                        "name": t.name,
                        "description": t.description,
                        "parent_id": t.parent_id,
                        "tag": t.tag,
                        "created_at": t.created_at.to_rfc3339(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&topo_json)?);
        }
    }

    Ok(())
}

fn print_tree_node_lineage(
    topo: &Topology,
    children: &HashMap<Option<String>, Vec<&Topology>>,
    prefix: &str,
    is_last: bool,
    is_root: bool,
) {
    let connector = if is_root { "" } else { "+-- " };

    let tag_str = topo
        .tag
        .as_ref()
        .map(|t| format!(" [{}]", style(t).dim()))
        .unwrap_or_default();

    println!("{}{}{}{}", prefix, connector, topo.name, tag_str);

    let child_prefix = if is_root {
        String::new()
    } else if is_last {
        format!("{}    ", prefix)
    } else {
        format!("{}|   ", prefix)
    };

    if let Some(kids) = children.get(&Some(topo.id.clone())) {
        for (i, kid) in kids.iter().enumerate() {
            let is_last_kid = i == kids.len() - 1;
            print_tree_node_lineage(kid, children, &child_prefix, is_last_kid, false);
        }
    }
}

fn log(db: &mut Database, name: &str, format: OutputFormat) -> Result<()> {
    let topo = resolve_topology(db, name)?;
    let target_id = topo.id.clone();

    // Walk parent chain to build ancestry list
    let mut ancestry: Vec<Topology> = vec![topo];

    loop {
        let current = ancestry.last().unwrap();
        match current.parent_id {
            Some(ref parent_id) => {
                let parent = db.conn().query_row(
                    "SELECT id, name, description, parent_id, tag, created_at, updated_at \
                     FROM topologies WHERE id = ?1",
                    params![parent_id],
                    Topology::from_row,
                )?;
                ancestry.push(parent);
            }
            None => break,
        }
    }

    // Reverse so root is first
    ancestry.reverse();

    match format {
        OutputFormat::Text => {
            println!("Ancestry of {}:", name);
            for (i, ancestor) in ancestry.iter().enumerate() {
                let indent = "    ".repeat(i);
                let connector = if i == 0 { "" } else { "+-- " };
                let tag_str = ancestor
                    .tag
                    .as_ref()
                    .map(|t| format!(" [{}]", style(t).dim()))
                    .unwrap_or_default();
                let date = ancestor.created_at.format("%Y-%m-%d");
                let marker = if ancestor.id == target_id {
                    format!("  {}", style("<-- you are here").dim())
                } else {
                    String::new()
                };
                println!(
                    "{}{}{}  ({}){}{}",
                    indent, connector, ancestor.name, date, tag_str, marker
                );
            }
        }
        OutputFormat::Json => {
            let json: Vec<Value> = ancestry
                .iter()
                .map(|t| serde_json::to_value(t).unwrap_or_default())
                .collect();
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
