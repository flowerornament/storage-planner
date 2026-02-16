//! sp export / sp import -- YAML topology export and import
//!
//! TOPO-11: Export topology to YAML (identity-preserving or template mode)
//! TOPO-10: Import topology from YAML with ID remapping

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{bail, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::db::Database;
use crate::core::events::{record_event, EventSource};
use crate::core::models::{Dataset, Link, Node, Placement, SyncRegime, Topology, Volume};
use crate::core::resolve::resolve_topology;

// ---------------------------------------------------------------------------
// Export data structures
// ---------------------------------------------------------------------------

/// Full topology export structure, serialized to/from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyExport {
    pub topology: Topology,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<Node>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volumes: Vec<Volume>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub datasets: Vec<Dataset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub placements: Vec<Placement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Link>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sync_regimes: Vec<SyncRegime>,
}

// ---------------------------------------------------------------------------
// Export command
// ---------------------------------------------------------------------------

pub fn run_export(
    db: &mut Database,
    topology_name: &str,
    template: bool,
    only: Option<&str>,
    output: Option<&PathBuf>,
) -> Result<()> {
    let topo = resolve_topology(db, topology_name)?;
    let topology_id = topo.id.clone();

    // Determine which entity types to include
    let include_all = only.is_none();
    let include_set: Vec<&str> = only
        .map(|s| s.split(',').map(|p| p.trim()).collect())
        .unwrap_or_default();

    let include = |name: &str| -> bool { include_all || include_set.contains(&name) };

    // Load entities using block-scoped statements (D023 pattern)
    let nodes: Vec<Node> = if include("nodes") {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, name, role, location, available_bays, interface_types, \
             power_draw_watts, cost_estimate, noise_db, rack_units, item_id, created_at, updated_at \
             FROM nodes WHERE topology_id = ?1 ORDER BY name",
        )?;
        let result = stmt
            .query_map(params![topology_id], Node::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    } else {
        Vec::new()
    };

    let volumes: Vec<Volume> = if include("volumes") {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, filesystem, \
             raid_level, pool_type, item_id, created_at, updated_at \
             FROM volumes WHERE topology_id = ?1 ORDER BY name",
        )?;
        let result = stmt
            .query_map(params![topology_id], Volume::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    } else {
        Vec::new()
    };

    let datasets: Vec<Dataset> = if include("datasets") {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, name, size_bytes, growth_rate_bytes_month, criticality, \
             min_copies, min_locations, max_rpo_hours, created_at, updated_at \
             FROM datasets WHERE topology_id = ?1 ORDER BY name",
        )?;
        let result = stmt
            .query_map(params![topology_id], Dataset::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    } else {
        Vec::new()
    };

    let placements: Vec<Placement> = if include("placements") {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, dataset_id, volume_id, role, priority, created_at \
             FROM placements WHERE topology_id = ?1 ORDER BY role",
        )?;
        let result = stmt
            .query_map(params![topology_id], Placement::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    } else {
        Vec::new()
    };

    let links: Vec<Link> = if include("links") {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, source_node_id, target_node_id, bandwidth_bytes_sec, \
             connection_type, latency_ms, is_metered, cost_per_gb_cents, created_at, updated_at \
             FROM links WHERE topology_id = ?1 ORDER BY connection_type",
        )?;
        let result = stmt
            .query_map(params![topology_id], Link::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    } else {
        Vec::new()
    };

    let sync_regimes: Vec<SyncRegime> = if include("sync_regimes") {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, name, dataset_id, source_volume_id, target_volume_id, \
             sync_type, schedule, direction, created_at, updated_at \
             FROM sync_regimes WHERE topology_id = ?1 ORDER BY name",
        )?;
        let result = stmt
            .query_map(params![topology_id], SyncRegime::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    } else {
        Vec::new()
    };

    let mut export = TopologyExport {
        topology: topo,
        nodes,
        volumes,
        datasets,
        placements,
        links,
        sync_regimes,
    };

    // Template mode: strip all UUIDs so import generates fresh ones
    if template {
        strip_ids(&mut export);
    }

    let yaml = serde_yaml_ng::to_string(&export)?;

    match output {
        Some(path) => {
            std::fs::write(path, &yaml)?;
            eprintln!(
                "Exported topology '{}' to {}",
                export.topology.name,
                path.display()
            );
        }
        None => {
            print!("{}", yaml);
        }
    }

    Ok(())
}

/// Strip all ID fields for template mode export.
/// Sets topology id, parent_id, and all entity id/topology_id/FK fields to empty strings.
fn strip_ids(export: &mut TopologyExport) {
    let empty = String::new();

    export.topology.id = empty.clone();
    export.topology.parent_id = None;
    export.topology.tag = None;

    for node in &mut export.nodes {
        node.id = empty.clone();
        node.topology_id = empty.clone();
    }

    for vol in &mut export.volumes {
        vol.id = empty.clone();
        vol.topology_id = empty.clone();
        vol.node_id = empty.clone();
    }

    for ds in &mut export.datasets {
        ds.id = empty.clone();
        ds.topology_id = empty.clone();
    }

    for pl in &mut export.placements {
        pl.id = empty.clone();
        pl.topology_id = empty.clone();
        pl.dataset_id = empty.clone();
        pl.volume_id = empty.clone();
    }

    for link in &mut export.links {
        link.id = empty.clone();
        link.topology_id = empty.clone();
        link.source_node_id = empty.clone();
        link.target_node_id = empty.clone();
    }

    for sr in &mut export.sync_regimes {
        sr.id = empty.clone();
        sr.topology_id = empty.clone();
        sr.dataset_id = empty.clone();
        sr.source_volume_id = empty.clone();
        sr.target_volume_id = empty.clone();
    }
}

// ---------------------------------------------------------------------------
// Import command
// ---------------------------------------------------------------------------

pub fn run_import(db: &mut Database, file: &PathBuf, name: Option<&str>) -> Result<()> {
    let yaml_content = std::fs::read_to_string(file)?;
    let export: TopologyExport = serde_yaml_ng::from_str(&yaml_content)?;

    // Determine the topology name
    let topo_name = match name {
        Some(n) => n.to_string(),
        None => {
            // Check if name already exists, append "-imported" if so
            let existing: i64 = db.conn().query_row(
                "SELECT COUNT(*) FROM topologies WHERE name = ?1",
                params![export.topology.name],
                |row| row.get(0),
            )?;
            if existing > 0 {
                format!("{}-imported", export.topology.name)
            } else {
                export.topology.name.clone()
            }
        }
    };

    // Check the final name doesn't collide either
    let name_exists: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM topologies WHERE name = ?1",
        params![topo_name],
        |row| row.get(0),
    )?;
    if name_exists > 0 {
        bail!(
            "Topology name '{}' already exists. Use --name to specify a different name.",
            topo_name
        );
    }

    // Build ID remapping tables (fork pattern)
    let mut id_map: HashMap<String, String> = HashMap::new();

    // Generate new topology ID
    let new_topo_id = Uuid::new_v4().to_string();
    if !export.topology.id.is_empty() {
        id_map.insert(export.topology.id.clone(), new_topo_id.clone());
    }

    // Generate new IDs for all entities
    for node in &export.nodes {
        let new_id = Uuid::new_v4().to_string();
        if !node.id.is_empty() {
            id_map.insert(node.id.clone(), new_id.clone());
        }
        // Also map by name for template mode (where ids are empty)
        // We'll use a separate name-based map for template mode FK resolution
        id_map.insert(format!("node:{}", node.name), new_id);
    }

    for vol in &export.volumes {
        let new_id = Uuid::new_v4().to_string();
        if !vol.id.is_empty() {
            id_map.insert(vol.id.clone(), new_id.clone());
        }
        id_map.insert(format!("volume:{}", vol.name), new_id);
    }

    for ds in &export.datasets {
        let new_id = Uuid::new_v4().to_string();
        if !ds.id.is_empty() {
            id_map.insert(ds.id.clone(), new_id.clone());
        }
        id_map.insert(format!("dataset:{}", ds.name), new_id);
    }

    for pl in &export.placements {
        let new_id = Uuid::new_v4().to_string();
        if !pl.id.is_empty() {
            id_map.insert(pl.id.clone(), new_id);
        }
    }

    for link in &export.links {
        let new_id = Uuid::new_v4().to_string();
        if !link.id.is_empty() {
            id_map.insert(link.id.clone(), new_id);
        }
    }

    for sr in &export.sync_regimes {
        let new_id = Uuid::new_v4().to_string();
        if !sr.id.is_empty() {
            id_map.insert(sr.id.clone(), new_id);
        }
    }

    // Helper to remap an ID: look up in map, or if empty (template mode), return fallback
    let remap = |old_id: &str, fallback_key: &str| -> Result<String> {
        if !old_id.is_empty() {
            if let Some(new_id) = id_map.get(old_id) {
                return Ok(new_id.clone());
            }
        }
        // Try fallback key (name-based lookup for template mode)
        if let Some(new_id) = id_map.get(fallback_key) {
            return Ok(new_id.clone());
        }
        bail!(
            "Cannot remap ID '{}' (fallback: '{}')",
            old_id,
            fallback_key
        );
    };

    // Count entities for summary
    let node_count = export.nodes.len();
    let volume_count = export.volumes.len();
    let dataset_count = export.datasets.len();
    let placement_count = export.placements.len();
    let link_count = export.links.len();
    let sync_count = export.sync_regimes.len();

    // Insert everything in a single transaction
    db.transaction(|tx| {
        let now = chrono::Utc::now();

        // Collect newly-inserted entities for composite after_state snapshot
        let mut new_nodes = Vec::new();
        let mut new_volumes = Vec::new();
        let mut new_datasets = Vec::new();
        let mut new_placements = Vec::new();
        let mut new_links = Vec::new();
        let mut new_sync_regimes = Vec::new();

        // 1. Insert topology
        let mut topo = export.topology.clone();
        topo.id = new_topo_id.clone();
        topo.name = topo_name.clone();
        topo.parent_id = None; // Imported topologies have no parent
        topo.tag = None; // Start untagged
        topo.created_at = now;
        topo.updated_at = now;
        topo.insert(tx)?;

        // 2. Insert nodes
        for node in &export.nodes {
            let new_id = remap(&node.id, &format!("node:{}", node.name))?;
            let mut new_node = node.clone();
            new_node.id = new_id;
            new_node.topology_id = new_topo_id.clone();
            new_node.created_at = now;
            new_node.updated_at = now;
            new_node.insert(tx)?;
            new_nodes.push(new_node);
        }

        // 3. Insert volumes (remap node_id)
        for vol in &export.volumes {
            let new_id = remap(&vol.id, &format!("volume:{}", vol.name))?;
            let new_node_id = remap(&vol.node_id, "")?;
            let mut new_vol = vol.clone();
            new_vol.id = new_id;
            new_vol.topology_id = new_topo_id.clone();
            new_vol.node_id = new_node_id;
            new_vol.created_at = now;
            new_vol.updated_at = now;
            new_vol.insert(tx)?;
            new_volumes.push(new_vol);
        }

        // 4. Insert datasets
        for ds in &export.datasets {
            let new_id = remap(&ds.id, &format!("dataset:{}", ds.name))?;
            let mut new_ds = ds.clone();
            new_ds.id = new_id;
            new_ds.topology_id = new_topo_id.clone();
            new_ds.created_at = now;
            new_ds.updated_at = now;
            new_ds.insert(tx)?;
            new_datasets.push(new_ds);
        }

        // 5. Insert placements (remap dataset_id, volume_id)
        for pl in &export.placements {
            let new_id = Uuid::new_v4().to_string();
            let new_dataset_id = remap(&pl.dataset_id, "")?;
            let new_volume_id = remap(&pl.volume_id, "")?;
            let mut new_pl = pl.clone();
            new_pl.id = new_id;
            new_pl.topology_id = new_topo_id.clone();
            new_pl.dataset_id = new_dataset_id;
            new_pl.volume_id = new_volume_id;
            new_pl.created_at = now;
            new_pl.insert(tx)?;
            new_placements.push(new_pl);
        }

        // 6. Insert links (remap source_node_id, target_node_id)
        for link in &export.links {
            let new_id = Uuid::new_v4().to_string();
            let new_source = remap(&link.source_node_id, "")?;
            let new_target = remap(&link.target_node_id, "")?;
            let mut new_link = link.clone();
            new_link.id = new_id;
            new_link.topology_id = new_topo_id.clone();
            new_link.source_node_id = new_source;
            new_link.target_node_id = new_target;
            new_link.created_at = now;
            new_link.updated_at = now;
            new_link.insert(tx)?;
            new_links.push(new_link);
        }

        // 7. Insert sync regimes (remap dataset_id, source_volume_id, target_volume_id)
        for sr in &export.sync_regimes {
            let new_id = Uuid::new_v4().to_string();
            let new_dataset_id = remap(&sr.dataset_id, "")?;
            let new_source_vol = remap(&sr.source_volume_id, "")?;
            let new_target_vol = remap(&sr.target_volume_id, "")?;
            let mut new_sr = sr.clone();
            new_sr.id = new_id;
            new_sr.topology_id = new_topo_id.clone();
            new_sr.dataset_id = new_dataset_id;
            new_sr.source_volume_id = new_source_vol;
            new_sr.target_volume_id = new_target_vol;
            new_sr.created_at = now;
            new_sr.updated_at = now;
            new_sr.insert(tx)?;
            new_sync_regimes.push(new_sr);
        }

        // 8. Build composite after_state snapshot for redo support
        let after_snapshot = TopologyExport {
            topology: topo.clone(),
            nodes: new_nodes,
            volumes: new_volumes,
            datasets: new_datasets,
            placements: new_placements,
            links: new_links,
            sync_regimes: new_sync_regimes,
        };
        let after_json = serde_json::to_string(&after_snapshot)?;

        // 9. Record import event with composite after_state
        record_event(
            tx,
            "topology.created",
            "topology",
            &new_topo_id,
            &format!("Imported topology '{}'", topo_name),
            None,
            Some(&after_json),
            &EventSource::Import,
        )?;

        Ok(())
    })?;

    println!(
        "Imported topology: {} with {} nodes, {} volumes, {} datasets",
        topo_name, node_count, volume_count, dataset_count
    );
    if placement_count > 0 || link_count > 0 || sync_count > 0 {
        println!(
            "  Also: {} placements, {} links, {} sync regimes",
            placement_count, link_count, sync_count
        );
    }

    Ok(())
}
