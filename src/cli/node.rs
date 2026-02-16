//! sp node -- Manage compute nodes within a topology
//!
//! Subcommands: add, list, show, remove, update
//! All mutating commands log events for undo/redo support.
//! All lookups support name-or-ID resolution via the entity resolver.

use anyhow::{bail, Result};
use clap::Subcommand;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::events::{record_event, EventSource, NodeSnapshot};
use crate::core::models::{Node, Placement, Volume};
use crate::core::resolve::{
    resolve_active_topology, resolve_catalog_item, resolve_node, validate_slug,
};
use crate::core::specs::Capacity;

use super::OutputFormat;

#[derive(Subcommand)]
pub enum NodeCommands {
    /// Add a compute node to the active topology
    Add {
        /// Node name (must be unique within topology)
        name: String,

        /// Node role (e.g., desktop, nas, server, cloud)
        #[arg(long)]
        role: String,

        /// Physical location (e.g., office, closet, datacenter)
        #[arg(long)]
        location: Option<String>,

        /// Number of available drive bays
        #[arg(long)]
        bays: Option<i32>,

        /// Supported interface types (e.g., usb3,thunderbolt4,sata)
        #[arg(long)]
        interface_types: Option<String>,

        /// Power draw in watts
        #[arg(long)]
        power_draw: Option<f64>,

        /// Estimated cost in dollars
        #[arg(long)]
        cost: Option<f64>,

        /// Noise level in dB
        #[arg(long)]
        noise: Option<f64>,

        /// Rack units consumed
        #[arg(long)]
        rack_units: Option<f64>,

        /// Link to a catalog item (name or ID prefix)
        #[arg(long)]
        item_id: Option<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// List nodes in the active topology
    List {
        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Show details of a specific node
    Show {
        /// Node name or ID
        name: String,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Remove a node (and its volumes) from the active topology
    Remove {
        /// Node name or ID to remove
        name: String,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },

    /// Update a node's properties
    Update {
        /// Node name or ID to update
        name: String,

        /// Rename the node (must be a valid slug)
        #[arg(long)]
        rename: Option<String>,

        /// Change the node role
        #[arg(long)]
        role: Option<String>,

        /// Change the location
        #[arg(long)]
        location: Option<String>,

        /// Change the number of available drive bays
        #[arg(long)]
        bays: Option<i32>,

        /// Change the interface types
        #[arg(long)]
        interface_types: Option<String>,

        /// Change the power draw in watts
        #[arg(long)]
        power_draw: Option<f64>,

        /// Change the estimated cost in dollars
        #[arg(long)]
        cost: Option<f64>,

        /// Change the noise level in dB
        #[arg(long)]
        noise: Option<f64>,

        /// Change the rack units consumed
        #[arg(long)]
        rack_units: Option<f64>,

        /// Link to a catalog item (name or ID prefix)
        #[arg(long)]
        item_id: Option<String>,

        /// Target topology (defaults to active)
        #[arg(long)]
        topology: Option<String>,
    },
}

pub fn run(cmd: NodeCommands, db: &mut Database, format: OutputFormat) -> Result<()> {
    match cmd {
        NodeCommands::Add {
            name,
            role,
            location,
            bays,
            interface_types,
            power_draw,
            cost,
            noise,
            rack_units,
            item_id,
            topology,
        } => add(
            db,
            &name,
            &role,
            location.as_deref(),
            bays,
            interface_types.as_deref(),
            power_draw,
            cost,
            noise,
            rack_units,
            item_id.as_deref(),
            topology.as_deref(),
            format,
        ),
        NodeCommands::List { topology } => list(db, topology.as_deref(), format),
        NodeCommands::Show { name, topology } => show(db, &name, topology.as_deref(), format),
        NodeCommands::Remove { name, topology } => remove(db, &name, topology.as_deref(), format),
        NodeCommands::Update {
            name,
            rename,
            role,
            location,
            bays,
            interface_types,
            power_draw,
            cost,
            noise,
            rack_units,
            item_id,
            topology,
        } => update(
            db,
            &name,
            rename.as_deref(),
            role.as_deref(),
            location.as_deref(),
            bays,
            interface_types.as_deref(),
            power_draw,
            cost,
            noise,
            rack_units,
            item_id.as_deref(),
            topology.as_deref(),
            format,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn add(
    db: &mut Database,
    name: &str,
    role: &str,
    location: Option<&str>,
    bays: Option<i32>,
    interface_types: Option<&str>,
    power_draw: Option<f64>,
    cost: Option<f64>,
    noise: Option<f64>,
    rack_units: Option<f64>,
    item_id: Option<&str>,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    validate_slug(name)?;

    // Validate non-negative values
    if let Some(pw) = power_draw {
        if pw < 0.0 {
            bail!("Power draw cannot be negative (got {}W)", pw);
        }
    }
    if let Some(c) = cost {
        if c < 0.0 {
            bail!("Cost cannot be negative (got ${})", c);
        }
    }
    if let Some(n) = noise {
        if n < 0.0 {
            bail!("Noise cannot be negative (got {}dB)", n);
        }
    }
    if let Some(ru) = rack_units {
        if ru < 0.0 {
            bail!("Rack units cannot be negative (got {}U)", ru);
        }
    }

    // Resolve active topology
    let topo = resolve_active_topology(db, topology_override)?;

    // Pre-insert uniqueness check
    let existing: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM nodes WHERE topology_id = ?1 AND name = ?2",
        params![topo.id, name],
        |row| row.get(0),
    )?;
    if existing > 0 {
        bail!("Node '{}' already exists in topology '{}'", name, topo.name);
    }

    // Resolve catalog item before transaction (D009 pattern)
    let resolved_item_id = if let Some(iid) = item_id {
        let item = resolve_catalog_item(db, iid)?;
        Some(item.id)
    } else {
        None
    };

    let mut node = Node::new(&topo.id, name, role);
    if let Some(loc) = location {
        node.location = loc.to_string();
    }
    node.available_bays = bays;
    if let Some(ifaces) = interface_types {
        node.interface_types = ifaces.to_string();
    }
    node.power_draw_watts = power_draw;
    node.cost_estimate = cost;
    node.noise_db = noise;
    node.rack_units = rack_units;
    node.item_id = resolved_item_id;

    let after_json = node.to_json()?;
    let node_id = node.id.clone();
    let node_name = node.name.clone();

    db.transaction(|tx| {
        node.insert(tx)?;

        record_event(
            tx,
            "node.created",
            "node",
            &node_id,
            &format!("Created node '{}'", node_name),
            None,
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    let id_prefix = &node_id[..8];
    match format {
        OutputFormat::Text => {
            println!("Created node '{}' (id: {})", name, id_prefix);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "created",
                "node": name,
                "id": node_id,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn list(db: &mut Database, topology_override: Option<&str>, format: OutputFormat) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;

    let mut stmt = db.conn().prepare(
        "SELECT id, topology_id, name, role, location, available_bays, interface_types, \
         power_draw_watts, cost_estimate, noise_db, rack_units, item_id, created_at, updated_at \
         FROM nodes WHERE topology_id = ?1 ORDER BY name",
    )?;

    let nodes: Vec<Node> = stmt
        .query_map(params![topo.id], Node::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Text => {
            if nodes.is_empty() {
                println!("No nodes found. Create one with 'sp node add <name> --role=<role>'");
            } else {
                for node in &nodes {
                    let location = if node.location.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", node.location)
                    };
                    println!("  {} [{}]{}", node.name, node.role, location);
                }
            }
        }
        OutputFormat::Json => {
            let json: Vec<serde_json::Value> = nodes
                .iter()
                .map(|n| {
                    serde_json::json!({
                        "id": n.id,
                        "name": n.name,
                        "role": n.role,
                        "location": n.location,
                        "available_bays": n.available_bays,
                        "interface_types": n.interface_types,
                        "power_draw_watts": n.power_draw_watts,
                        "created_at": n.created_at.to_rfc3339(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

fn show(
    db: &mut Database,
    name: &str,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let node = resolve_node(db, &topo.id, name)?;

    // Query volumes for this node
    let mut vol_stmt = db.conn().prepare(
        "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, \
         filesystem, raid_level, pool_type, item_id, created_at, updated_at \
         FROM volumes WHERE node_id = ?1 ORDER BY name",
    )?;
    let volumes: Vec<Volume> = vol_stmt
        .query_map(params![node.id], Volume::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match format {
        OutputFormat::Text => {
            println!("Node: {} [{}]", node.name, node.role);
            if !node.location.is_empty() {
                println!("  Location:        {}", node.location);
            }
            if let Some(bays) = node.available_bays {
                println!("  Available bays:  {}", bays);
            }
            if !node.interface_types.is_empty() {
                println!("  Interfaces:      {}", node.interface_types);
            }
            if let Some(watts) = node.power_draw_watts {
                println!("  Power draw:      {:.0}W", watts);
            }
            if let Some(cost) = node.cost_estimate {
                println!("  Cost estimate:   ${:.2}", cost);
            }
            if let Some(noise) = node.noise_db {
                println!("  Noise:           {:.1} dB", noise);
            }
            if let Some(ru) = node.rack_units {
                println!("  Rack units:      {:.0}U", ru);
            }
            println!("  ID:              {}", node.id);
            println!(
                "  Created:         {}",
                node.created_at.format("%Y-%m-%d %H:%M:%S")
            );

            if volumes.is_empty() {
                println!("  Volumes:         (none)");
            } else {
                println!("  Volumes:");
                for vol in &volumes {
                    let cap = Capacity::from_bytes(vol.capacity_bytes as u64);
                    let fs = vol
                        .filesystem
                        .as_deref()
                        .map(|f| format!(" {}", f))
                        .unwrap_or_default();
                    let raid = vol
                        .raid_level
                        .as_ref()
                        .map(|r| format!("/{}", r))
                        .unwrap_or_default();
                    println!("    {}: {}{}{}", vol.name, cap, fs, raid);
                }
            }
        }
        OutputFormat::Json => {
            let vol_json: Vec<serde_json::Value> = volumes
                .iter()
                .map(|v| serde_json::to_value(v).unwrap_or_default())
                .collect();

            let mut node_val = serde_json::to_value(&node)?;
            if let serde_json::Value::Object(ref mut map) = node_val {
                map.insert("volumes".to_string(), serde_json::Value::Array(vol_json));
            }
            println!("{}", serde_json::to_string_pretty(&node_val)?);
        }
    }

    Ok(())
}

fn remove(
    db: &mut Database,
    name: &str,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_override)?;
    let node = resolve_node(db, &topo.id, name)?;

    let node_id = node.id.clone();
    let node_name = node.name.clone();

    // Capture composite snapshot: node + volumes + placements (for undo)
    let volumes: Vec<Volume> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, filesystem, \
             raid_level, pool_type, item_id, created_at, updated_at \
             FROM volumes WHERE node_id = ?1 ORDER BY name",
        )?;
        let result = stmt
            .query_map(params![node_id], Volume::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    let volume_ids: Vec<String> = volumes.iter().map(|v| v.id.clone()).collect();
    let placements: Vec<Placement> = if !volume_ids.is_empty() {
        let placeholders: String = volume_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, topology_id, dataset_id, volume_id, role, priority, created_at \
             FROM placements WHERE volume_id IN ({}) ORDER BY role",
            placeholders
        );
        let mut stmt = db.conn().prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> = volume_ids
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();
        let result = stmt
            .query_map(params.as_slice(), Placement::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    } else {
        Vec::new()
    };

    let volume_count = volumes.len();
    let snapshot = NodeSnapshot {
        node: node.clone(),
        volumes,
        placements,
    };
    let before_json = serde_json::to_string(&snapshot)?;

    // Print warning about cascading deletes
    if volume_count > 0 {
        eprintln!(
            "Warning: Removing node '{}' (and {} volume{})",
            node_name,
            volume_count,
            if volume_count == 1 { "" } else { "s" }
        );
    }

    db.transaction(|tx| {
        tx.execute("DELETE FROM nodes WHERE id = ?1", params![node_id])?;

        record_event(
            tx,
            "node.deleted",
            "node",
            &node_id,
            &format!("Deleted node '{}'", node_name),
            Some(&before_json),
            None,
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!("Removed node '{}'", node_name);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "deleted",
                "node": node_name,
                "id": node_id,
                "volumes_removed": volume_count,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn update(
    db: &mut Database,
    name: &str,
    rename: Option<&str>,
    role: Option<&str>,
    location: Option<&str>,
    bays: Option<i32>,
    interface_types: Option<&str>,
    power_draw: Option<f64>,
    cost: Option<f64>,
    noise: Option<f64>,
    rack_units: Option<f64>,
    item_id: Option<&str>,
    topology_override: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    if rename.is_none()
        && role.is_none()
        && location.is_none()
        && bays.is_none()
        && interface_types.is_none()
        && power_draw.is_none()
        && cost.is_none()
        && noise.is_none()
        && rack_units.is_none()
        && item_id.is_none()
    {
        bail!("Nothing to update. Provide --rename, --role, --location, --bays, --interface-types, --power-draw, --cost, --noise, --rack-units, or --item-id.");
    }

    // Validate new name if renaming
    if let Some(new_name) = rename {
        validate_slug(new_name)?;
    }

    // Validate non-negative values
    if let Some(pw) = power_draw {
        if pw < 0.0 {
            bail!("Power draw cannot be negative (got {}W)", pw);
        }
    }
    if let Some(c) = cost {
        if c < 0.0 {
            bail!("Cost cannot be negative (got ${})", c);
        }
    }
    if let Some(n) = noise {
        if n < 0.0 {
            bail!("Noise cannot be negative (got {}dB)", n);
        }
    }
    if let Some(ru) = rack_units {
        if ru < 0.0 {
            bail!("Rack units cannot be negative (got {}U)", ru);
        }
    }

    // Resolve outside transaction
    let topo = resolve_active_topology(db, topology_override)?;
    let node = resolve_node(db, &topo.id, name)?;
    let before_json = node.to_json()?;
    let node_id = node.id.clone();
    let original_name = node.name.clone();

    // Check uniqueness of new name if renaming
    if let Some(new_name) = rename {
        if new_name != original_name {
            let existing: i64 = db.conn().query_row(
                "SELECT COUNT(*) FROM nodes WHERE topology_id = ?1 AND name = ?2 AND id != ?3",
                params![topo.id, new_name, node_id],
                |row| row.get(0),
            )?;
            if existing > 0 {
                bail!("Node name '{}' is already taken in this topology", new_name);
            }
        }
    }

    // Resolve catalog item before transaction (D009 pattern)
    let resolved_item_id = if let Some(iid) = item_id {
        let item = resolve_catalog_item(db, iid)?;
        Some(item.id)
    } else {
        None
    };

    // Build after state for event
    let mut after = node.clone();
    if let Some(new_name) = rename {
        after.name = new_name.to_string();
    }
    if let Some(r) = role {
        after.role = r.to_string();
    }
    if let Some(loc) = location {
        after.location = loc.to_string();
    }
    if let Some(b) = bays {
        after.available_bays = Some(b);
    }
    if let Some(ifaces) = interface_types {
        after.interface_types = ifaces.to_string();
    }
    if let Some(watts) = power_draw {
        after.power_draw_watts = Some(watts);
    }
    if let Some(c) = cost {
        after.cost_estimate = Some(c);
    }
    if let Some(n) = noise {
        after.noise_db = Some(n);
    }
    if let Some(ru) = rack_units {
        after.rack_units = Some(ru);
    }
    if let Some(ref iid) = resolved_item_id {
        after.item_id = Some(iid.clone());
    }
    let after_json = after.to_json()?;
    let final_name = after.name.clone();

    db.transaction(|tx| {
        if let Some(new_name) = rename {
            tx.execute(
                "UPDATE nodes SET name = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![new_name, node_id],
            )?;
        }
        if let Some(r) = role {
            tx.execute(
                "UPDATE nodes SET role = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![r, node_id],
            )?;
        }
        if let Some(loc) = location {
            tx.execute(
                "UPDATE nodes SET location = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![loc, node_id],
            )?;
        }
        if let Some(b) = bays {
            tx.execute(
                "UPDATE nodes SET available_bays = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![b, node_id],
            )?;
        }
        if let Some(ifaces) = interface_types {
            tx.execute(
                "UPDATE nodes SET interface_types = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![ifaces, node_id],
            )?;
        }
        if let Some(watts) = power_draw {
            tx.execute(
                "UPDATE nodes SET power_draw_watts = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![watts, node_id],
            )?;
        }
        if let Some(c) = cost {
            tx.execute(
                "UPDATE nodes SET cost_estimate = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![c, node_id],
            )?;
        }
        if let Some(n) = noise {
            tx.execute(
                "UPDATE nodes SET noise_db = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![n, node_id],
            )?;
        }
        if let Some(ru) = rack_units {
            tx.execute(
                "UPDATE nodes SET rack_units = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![ru, node_id],
            )?;
        }
        if let Some(ref iid) = resolved_item_id {
            tx.execute(
                "UPDATE nodes SET item_id = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![iid, node_id],
            )?;
        }

        record_event(
            tx,
            "node.updated",
            "node",
            &node_id,
            &format!("Updated node '{}'", original_name),
            Some(&before_json),
            Some(&after_json),
            &EventSource::User,
        )?;

        Ok(())
    })?;

    match format {
        OutputFormat::Text => {
            println!("Updated node '{}'", final_name);
        }
        OutputFormat::Json => {
            let json = serde_json::json!({
                "action": "updated",
                "node": final_name,
                "id": node_id,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}
