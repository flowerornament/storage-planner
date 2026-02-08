//! sp diagram -- ASCII visualization of topology structure
//!
//! TOPO-09: Two perspectives:
//! - --tree: node-volume-dataset hierarchy with box-drawing characters
//! - --network: link topology between nodes

use anyhow::Result;
use rusqlite::params;

use crate::core::db::Database;
use crate::core::models::{Dataset, Link, Node, Placement, Volume};
use crate::core::resolve::resolve_active_topology;
use crate::core::specs::Capacity;

/// Run the diagram command
pub fn run(
    db: &mut Database,
    topology_name: Option<&str>,
    tree: bool,
    network: bool,
) -> Result<()> {
    let topo = resolve_active_topology(db, topology_name)?;

    // Default to tree mode if neither specified
    let show_tree = tree || !network;
    let show_network = network;

    if show_tree {
        print_tree(db, &topo.id, &topo.name)?;
    }

    if show_tree && show_network {
        println!();
    }

    if show_network {
        print_network(db, &topo.id, &topo.name)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tree mode: node -> volume -> dataset hierarchy
// ---------------------------------------------------------------------------

fn print_tree(db: &Database, topology_id: &str, topology_name: &str) -> Result<()> {
    // Load nodes
    let nodes: Vec<Node> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, name, role, location, available_bays, interface_types, \
             power_draw_watts, cost_estimate, noise_db, rack_units, item_id, created_at, updated_at \
             FROM nodes WHERE topology_id = ?1 ORDER BY name",
        )?;
        let result = stmt
            .query_map(params![topology_id], Node::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    println!("{}", topology_name);

    if nodes.is_empty() {
        println!("  (no nodes)");
        return Ok(());
    }

    for (ni, node) in nodes.iter().enumerate() {
        let is_last_node = ni == nodes.len() - 1;
        let node_connector = if is_last_node { "\u{2514}\u{2500}\u{2500}" } else { "\u{251c}\u{2500}\u{2500}" };
        let node_detail = format_node_detail(node);
        println!("{} {}", node_connector, node_detail);

        // Load volumes for this node
        let volumes: Vec<Volume> = {
            let mut stmt = db.conn().prepare(
                "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, \
                 filesystem, raid_level, pool_type, item_id, created_at, updated_at \
                 FROM volumes WHERE node_id = ?1 ORDER BY name",
            )?;
            let result = stmt
                .query_map(params![node.id], Volume::from_row)?
                .collect::<Result<Vec<_>, _>>()?;
            result
        };

        let node_prefix = if is_last_node { "    " } else { "\u{2502}   " };

        for (vi, vol) in volumes.iter().enumerate() {
            let is_last_vol = vi == volumes.len() - 1;
            let vol_connector = if is_last_vol { "\u{2514}\u{2500}\u{2500}" } else { "\u{251c}\u{2500}\u{2500}" };
            let vol_detail = format_volume_detail(vol);
            println!("{}{} {}", node_prefix, vol_connector, vol_detail);

            // Load placements + datasets for this volume
            let placements: Vec<(Placement, Dataset)> = {
                let mut stmt = db.conn().prepare(
                    "SELECT p.id, p.topology_id, p.dataset_id, p.volume_id, p.role, p.priority, \
                     p.created_at, d.id as d_id, d.topology_id as d_topology_id, d.name as d_name, \
                     d.size_bytes, d.growth_rate_bytes_month, d.criticality, d.min_copies, \
                     d.min_locations, d.max_rpo_hours, d.created_at as d_created_at, \
                     d.updated_at as d_updated_at \
                     FROM placements p JOIN datasets d ON p.dataset_id = d.id \
                     WHERE p.volume_id = ?1 ORDER BY d.name",
                )?;
                let result = stmt
                    .query_map(params![vol.id], |row| {
                        let placement = Placement::from_row(row)?;
                        let dataset = Dataset {
                            id: row.get("d_id")?,
                            topology_id: row.get("d_topology_id")?,
                            name: row.get("d_name")?,
                            size_bytes: row.get("size_bytes")?,
                            growth_rate_bytes_month: row.get("growth_rate_bytes_month")?,
                            criticality: row.get("criticality")?,
                            min_copies: row.get("min_copies")?,
                            min_locations: row.get("min_locations")?,
                            max_rpo_hours: row.get("max_rpo_hours")?,
                            created_at: chrono::DateTime::parse_from_rfc3339(
                                &row.get::<_, String>("d_created_at")?,
                            )
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_else(|_| chrono::Utc::now()),
                            updated_at: chrono::DateTime::parse_from_rfc3339(
                                &row.get::<_, String>("d_updated_at")?,
                            )
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or_else(|_| chrono::Utc::now()),
                        };
                        Ok((placement, dataset))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                result
            };

            let vol_prefix = if is_last_vol {
                format!("{}    ", node_prefix)
            } else {
                format!("{}\u{2502}   ", node_prefix)
            };

            for (di, (pl, ds)) in placements.iter().enumerate() {
                let is_last_ds = di == placements.len() - 1;
                let ds_connector = if is_last_ds { "\u{2514}\u{2500}\u{2500}" } else { "\u{251c}\u{2500}\u{2500}" };
                let ds_detail = format_dataset_detail(ds, pl);
                println!("{}{} {}", vol_prefix, ds_connector, ds_detail);
            }
        }
    }

    Ok(())
}

fn format_node_detail(node: &Node) -> String {
    let mut parts = vec![node.name.clone()];
    let mut meta = Vec::new();
    if !node.role.is_empty() {
        meta.push(node.role.clone());
    }
    if !node.location.is_empty() {
        meta.push(node.location.clone());
    }
    if !meta.is_empty() {
        parts.push(format!("({})", meta.join(", ")));
    }
    parts.join(" ")
}

fn format_volume_detail(vol: &Volume) -> String {
    let cap = Capacity::from_bytes(vol.capacity_bytes as u64);
    let mut meta = vec![cap.to_string()];
    if let Some(ref fs) = vol.filesystem {
        if !fs.is_empty() {
            meta.push(fs.clone());
        }
    }
    if let Some(ref raid) = vol.raid_level {
        if !raid.is_empty() {
            meta.push(raid.clone());
        }
    }
    format!("{} [{}]", vol.name, meta.join(", "))
}

fn format_dataset_detail(ds: &Dataset, pl: &Placement) -> String {
    let size = Capacity::from_bytes(ds.size_bytes as u64);
    if pl.role == "primary" {
        format!("{} ({}, {})", ds.name, size, ds.criticality)
    } else {
        format!("{} ({})", ds.name, pl.role)
    }
}

// ---------------------------------------------------------------------------
// Network mode: link topology between nodes
// ---------------------------------------------------------------------------

fn print_network(db: &Database, topology_id: &str, topology_name: &str) -> Result<()> {
    // Load nodes for name lookup
    let nodes: Vec<Node> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, name, role, location, available_bays, interface_types, \
             power_draw_watts, cost_estimate, noise_db, rack_units, item_id, created_at, updated_at \
             FROM nodes WHERE topology_id = ?1 ORDER BY name",
        )?;
        let result = stmt
            .query_map(params![topology_id], Node::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    let node_map: std::collections::HashMap<String, &Node> =
        nodes.iter().map(|n| (n.id.clone(), n)).collect();

    // Load links
    let links: Vec<Link> = {
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, source_node_id, target_node_id, bandwidth_bytes_sec, \
             connection_type, latency_ms, is_metered, cost_per_gb_cents, created_at, updated_at \
             FROM links WHERE topology_id = ?1 ORDER BY connection_type",
        )?;
        let result = stmt
            .query_map(params![topology_id], Link::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        result
    };

    println!("Network Topology: {}", topology_name);
    println!();

    if links.is_empty() {
        println!("No network links defined");
    } else {
        for link in &links {
            let source = node_map
                .get(&link.source_node_id)
                .map(|n| format_node_short(n))
                .unwrap_or_else(|| "?".to_string());
            let target = node_map
                .get(&link.target_node_id)
                .map(|n| format_node_short(n))
                .unwrap_or_else(|| "?".to_string());
            let link_label = format_link_label(link);
            println!(
                "{} \u{2500}\u{2500}[{}]\u{2500}\u{2500}> {}",
                source, link_label, target
            );
        }
    }

    println!();
    println!("Nodes: {}  Links: {}", nodes.len(), links.len());

    Ok(())
}

fn format_node_short(node: &Node) -> String {
    if node.location.is_empty() {
        node.name.clone()
    } else {
        format!("{} ({})", node.name, node.location)
    }
}

fn format_link_label(link: &Link) -> String {
    let mut parts = vec![link.connection_type.clone()];
    if let Some(bw) = link.bandwidth_bytes_sec {
        parts.push(format_bandwidth(bw));
    }
    parts.join(", ")
}

fn format_bandwidth(bytes_per_sec: i64) -> String {
    let bps = bytes_per_sec as f64;
    if bps >= Capacity::GB as f64 {
        format!("{:.0}GB/s", bps / Capacity::GB as f64)
    } else if bps >= Capacity::MB as f64 {
        format!("{:.0}MB/s", bps / Capacity::MB as f64)
    } else if bps >= Capacity::KB as f64 {
        format!("{:.0}KB/s", bps / Capacity::KB as f64)
    } else {
        format!("{}B/s", bytes_per_sec)
    }
}
