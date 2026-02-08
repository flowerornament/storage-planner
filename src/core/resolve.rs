//! Entity resolver: name-or-ID lookup with disambiguation
//!
//! All entity commands use these functions to resolve user input (name or UUID prefix)
//! into concrete entities. Supports:
//! - Exact name match within a topology
//! - UUID prefix match (minimum 4 chars) with ambiguity detection
//! - Active topology resolution with --topology override
//! - Slug-like name validation

use anyhow::{bail, Result};
use rusqlite::params;

use super::db::Database;
use super::models::{CatalogItem, Dataset, Decision, Node, Topology, Volume};

/// Validate that a name is a valid slug: alphanumeric, hyphens, and underscores only.
///
/// Rejects names with spaces, special characters, or empty strings.
pub fn validate_slug(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Invalid name: must not be empty");
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        bail!(
            "Invalid name '{}': must contain only alphanumeric characters, hyphens, and underscores",
            name
        );
    }
    Ok(())
}

/// Resolve the active topology, or the one specified by `--topology` override.
///
/// If `override_name` is Some, resolves that topology by name-or-ID.
/// Otherwise, finds the topology where `tag = 'current'`.
pub fn resolve_active_topology(db: &Database, override_name: Option<&str>) -> Result<Topology> {
    if let Some(name) = override_name {
        return resolve_topology(db, name);
    }

    db.conn()
        .query_row(
            "SELECT id, name, description, parent_id, tag, created_at, updated_at \
             FROM topologies WHERE tag = 'current'",
            [],
            Topology::from_row,
        )
        .map_err(|_| {
            anyhow::anyhow!(
                "No active topology. Create one with 'sp topology create' or use --topology"
            )
        })
}

/// Resolve a topology by exact name or UUID prefix (minimum 4 chars).
///
/// Priority: exact name match first, then UUID prefix match.
/// Errors on ambiguous prefix match (multiple topologies share the prefix).
pub fn resolve_topology(db: &Database, name_or_id: &str) -> Result<Topology> {
    // Try exact name match first
    let name_result = db.conn().query_row(
        "SELECT id, name, description, parent_id, tag, created_at, updated_at \
         FROM topologies WHERE name = ?1",
        params![name_or_id],
        Topology::from_row,
    );

    if let Ok(topo) = name_result {
        return Ok(topo);
    }

    // Try UUID prefix match
    if name_or_id.len() < 4 {
        bail!(
            "Topology '{}' not found. UUID prefix must be at least 4 characters.",
            name_or_id
        );
    }

    let pattern = format!("{}%", name_or_id);
    let mut stmt = db.conn().prepare(
        "SELECT id, name, description, parent_id, tag, created_at, updated_at \
         FROM topologies WHERE id LIKE ?1",
    )?;

    let matches: Vec<Topology> = stmt
        .query_map(params![pattern], Topology::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match matches.len() {
        0 => bail!("Topology '{}' not found", name_or_id),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => {
            let names: Vec<String> = matches.iter().map(|t| t.name.clone()).collect();
            bail!(
                "Ambiguous topology prefix '{}': matches {}",
                name_or_id,
                names.join(", ")
            )
        }
    }
}

/// Resolve a node by exact name (within topology) or UUID prefix.
pub fn resolve_node(db: &Database, topology_id: &str, name_or_id: &str) -> Result<Node> {
    // Try exact name match within topology
    let name_result = db.conn().query_row(
        "SELECT id, topology_id, name, role, location, available_bays, interface_types, \
         power_draw_watts, cost_estimate, noise_db, rack_units, item_id, created_at, updated_at \
         FROM nodes WHERE topology_id = ?1 AND name = ?2",
        params![topology_id, name_or_id],
        Node::from_row,
    );

    if let Ok(node) = name_result {
        return Ok(node);
    }

    // Try UUID prefix match
    if name_or_id.len() < 4 {
        bail!(
            "Node '{}' not found. UUID prefix must be at least 4 characters.",
            name_or_id
        );
    }

    let pattern = format!("{}%", name_or_id);
    let mut stmt = db.conn().prepare(
        "SELECT id, topology_id, name, role, location, available_bays, interface_types, \
         power_draw_watts, cost_estimate, noise_db, rack_units, item_id, created_at, updated_at \
         FROM nodes WHERE topology_id = ?1 AND id LIKE ?2",
    )?;

    let matches: Vec<Node> = stmt
        .query_map(params![topology_id, pattern], Node::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match matches.len() {
        0 => bail!("Node '{}' not found", name_or_id),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => {
            let names: Vec<String> = matches.iter().map(|n| n.name.clone()).collect();
            bail!(
                "Ambiguous node prefix '{}': matches {}",
                name_or_id,
                names.join(", ")
            )
        }
    }
}

/// Resolve a volume by exact name (within topology) or UUID prefix.
///
/// If the name matches multiple volumes on different nodes, requires `node_hint`
/// to disambiguate. If `node_hint` is provided, resolves the node first, then
/// filters volumes by that node_id.
pub fn resolve_volume(
    db: &Database,
    topology_id: &str,
    name_or_id: &str,
    node_hint: Option<&str>,
) -> Result<Volume> {
    // If node_hint is provided, resolve node first and filter by it
    if let Some(hint) = node_hint {
        let node = resolve_node(db, topology_id, hint)?;

        // Try exact name match within node
        let name_result = db.conn().query_row(
            "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, \
             filesystem, raid_level, pool_type, item_id, created_at, updated_at \
             FROM volumes WHERE topology_id = ?1 AND node_id = ?2 AND name = ?3",
            params![topology_id, node.id, name_or_id],
            Volume::from_row,
        );

        if let Ok(vol) = name_result {
            return Ok(vol);
        }

        // Try UUID prefix within node
        if name_or_id.len() < 4 {
            bail!(
                "Volume '{}' not found on node '{}'. UUID prefix must be at least 4 characters.",
                name_or_id,
                hint
            );
        }

        let pattern = format!("{}%", name_or_id);
        let mut stmt = db.conn().prepare(
            "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, \
             filesystem, raid_level, pool_type, item_id, created_at, updated_at \
             FROM volumes WHERE topology_id = ?1 AND node_id = ?2 AND id LIKE ?3",
        )?;

        let matches: Vec<Volume> = stmt
            .query_map(params![topology_id, node.id, pattern], Volume::from_row)?
            .collect::<Result<Vec<_>, _>>()?;

        return match matches.len() {
            0 => bail!("Volume '{}' not found on node '{}'", name_or_id, hint),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => {
                let names: Vec<String> = matches.iter().map(|v| v.name.clone()).collect();
                bail!(
                    "Ambiguous volume prefix '{}' on node '{}': matches {}",
                    name_or_id,
                    hint,
                    names.join(", ")
                )
            }
        };
    }

    // No node hint -- try exact name match within topology
    let mut stmt = db.conn().prepare(
        "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, \
         filesystem, raid_level, pool_type, item_id, created_at, updated_at \
         FROM volumes WHERE topology_id = ?1 AND name = ?2",
    )?;

    let name_matches: Vec<Volume> = stmt
        .query_map(params![topology_id, name_or_id], Volume::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match name_matches.len() {
        1 => return Ok(name_matches.into_iter().next().unwrap()),
        n if n > 1 => {
            // Ambiguous -- need node hint
            let node_names: Vec<String> = name_matches
                .iter()
                .filter_map(|v| {
                    db.conn()
                        .query_row(
                            "SELECT name FROM nodes WHERE id = ?1",
                            params![v.node_id],
                            |row| row.get::<_, String>(0),
                        )
                        .ok()
                })
                .collect();
            bail!(
                "Volume '{}' exists on multiple nodes: {}. Use --node to disambiguate.",
                name_or_id,
                node_names.join(", ")
            );
        }
        _ => {}
    }

    // Try UUID prefix match
    if name_or_id.len() < 4 {
        bail!(
            "Volume '{}' not found. UUID prefix must be at least 4 characters.",
            name_or_id
        );
    }

    let pattern = format!("{}%", name_or_id);
    let mut stmt = db.conn().prepare(
        "SELECT id, topology_id, node_id, name, capacity_bytes, usable_bytes, \
         filesystem, raid_level, pool_type, item_id, created_at, updated_at \
         FROM volumes WHERE topology_id = ?1 AND id LIKE ?2",
    )?;

    let matches: Vec<Volume> = stmt
        .query_map(params![topology_id, pattern], Volume::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match matches.len() {
        0 => bail!("Volume '{}' not found", name_or_id),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => {
            let names: Vec<String> = matches.iter().map(|v| v.name.clone()).collect();
            bail!(
                "Ambiguous volume prefix '{}': matches {}",
                name_or_id,
                names.join(", ")
            )
        }
    }
}

/// Resolve a dataset by exact name (within topology) or UUID prefix.
pub fn resolve_dataset(db: &Database, topology_id: &str, name_or_id: &str) -> Result<Dataset> {
    // Try exact name match within topology
    let name_result = db.conn().query_row(
        "SELECT id, topology_id, name, size_bytes, growth_rate_bytes_month, criticality, \
         min_copies, min_locations, max_rpo_hours, created_at, updated_at \
         FROM datasets WHERE topology_id = ?1 AND name = ?2",
        params![topology_id, name_or_id],
        Dataset::from_row,
    );

    if let Ok(ds) = name_result {
        return Ok(ds);
    }

    // Try UUID prefix match
    if name_or_id.len() < 4 {
        bail!(
            "Dataset '{}' not found. UUID prefix must be at least 4 characters.",
            name_or_id
        );
    }

    let pattern = format!("{}%", name_or_id);
    let mut stmt = db.conn().prepare(
        "SELECT id, topology_id, name, size_bytes, growth_rate_bytes_month, criticality, \
         min_copies, min_locations, max_rpo_hours, created_at, updated_at \
         FROM datasets WHERE topology_id = ?1 AND id LIKE ?2",
    )?;

    let matches: Vec<Dataset> = stmt
        .query_map(params![topology_id, pattern], Dataset::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match matches.len() {
        0 => bail!("Dataset '{}' not found", name_or_id),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => {
            let names: Vec<String> = matches.iter().map(|d| d.name.clone()).collect();
            bail!(
                "Ambiguous dataset prefix '{}': matches {}",
                name_or_id,
                names.join(", ")
            )
        }
    }
}

/// Resolve a decision by exact title or UUID prefix (minimum 4 chars).
///
/// Decisions use titles (not slug names), so no slug validation is performed.
/// Titles may contain spaces and special characters.
pub fn resolve_decision(db: &Database, title_or_id: &str) -> Result<Decision> {
    // Try exact title match
    let title_result = db.conn().query_row(
        "SELECT id, title, description, status, parent_id, chosen_topology_id, \
         rationale, snapshot, created_at, updated_at, closed_at \
         FROM decisions WHERE title = ?1",
        params![title_or_id],
        Decision::from_row,
    );

    if let Ok(decision) = title_result {
        return Ok(decision);
    }

    // Try UUID prefix match
    if title_or_id.len() < 4 {
        bail!(
            "Decision '{}' not found. UUID prefix must be at least 4 characters.",
            title_or_id
        );
    }

    let pattern = format!("{}%", title_or_id);
    let mut stmt = db.conn().prepare(
        "SELECT id, title, description, status, parent_id, chosen_topology_id, \
         rationale, snapshot, created_at, updated_at, closed_at \
         FROM decisions WHERE id LIKE ?1",
    )?;

    let matches: Vec<Decision> = stmt
        .query_map(params![pattern], Decision::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match matches.len() {
        0 => bail!("Decision '{}' not found", title_or_id),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => {
            let titles: Vec<String> = matches.iter().map(|d| d.title.clone()).collect();
            bail!(
                "Ambiguous decision prefix '{}': matches {}",
                title_or_id,
                titles.join(", ")
            )
        }
    }
}

/// Resolve a catalog item by exact name or UUID prefix (minimum 4 chars).
///
/// Catalog items are global (not scoped to a topology), similar to decisions.
/// Exact name match is tried first, then UUID prefix match.
pub fn resolve_catalog_item(db: &Database, name_or_id: &str) -> Result<CatalogItem> {
    // Try exact name match
    let name_result = db.conn().query_row(
        "SELECT id, name, category, specs, url, notes, created_at, updated_at \
         FROM catalog_items WHERE name = ?1",
        params![name_or_id],
        CatalogItem::from_row,
    );

    if let Ok(item) = name_result {
        return Ok(item);
    }

    // Try UUID prefix match
    if name_or_id.len() < 4 {
        bail!(
            "Catalog item '{}' not found. UUID prefix must be at least 4 characters.",
            name_or_id
        );
    }

    let pattern = format!("{}%", name_or_id);
    let mut stmt = db.conn().prepare(
        "SELECT id, name, category, specs, url, notes, created_at, updated_at \
         FROM catalog_items WHERE id LIKE ?1",
    )?;

    let matches: Vec<CatalogItem> = stmt
        .query_map(params![pattern], CatalogItem::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    match matches.len() {
        0 => bail!("Catalog item '{}' not found", name_or_id),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => {
            let names: Vec<String> = matches.iter().map(|i| i.name.clone()).collect();
            bail!(
                "Ambiguous catalog item prefix '{}': matches {}",
                name_or_id,
                names.join(", ")
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::{CatalogItem, Decision, Node, Volume};

    fn setup_db() -> Database {
        Database::open_memory().unwrap()
    }

    #[test]
    fn test_validate_slug_valid() {
        assert!(validate_slug("my-node").is_ok());
        assert!(validate_slug("node_1").is_ok());
        assert!(validate_slug("abc123").is_ok());
    }

    #[test]
    fn test_validate_slug_invalid() {
        assert!(validate_slug("my node").is_err());
        assert!(validate_slug("node!").is_err());
        assert!(validate_slug("hello world").is_err());
        assert!(validate_slug("").is_err());
    }

    #[test]
    fn test_resolve_topology_by_name() {
        let mut db = setup_db();
        let topo = Topology::new("my-setup", "Test topology");
        db.transaction(|tx| {
            topo.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let resolved = resolve_topology(&db, "my-setup").unwrap();
        assert_eq!(resolved.id, topo.id);
        assert_eq!(resolved.name, "my-setup");
    }

    #[test]
    fn test_resolve_topology_by_id_prefix() {
        let mut db = setup_db();
        let topo = Topology::new("my-setup", "Test topology");
        let id_prefix = topo.id[..8].to_string();

        db.transaction(|tx| {
            topo.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let resolved = resolve_topology(&db, &id_prefix).unwrap();
        assert_eq!(resolved.id, topo.id);
    }

    #[test]
    fn test_resolve_topology_prefix_too_short() {
        let mut db = setup_db();
        let topo = Topology::new("my-setup", "Test topology");
        let short_prefix = topo.id[..3].to_string();

        db.transaction(|tx| {
            topo.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let result = resolve_topology(&db, &short_prefix);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not found"),
            "Expected 'not found' error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_resolve_topology_not_found() {
        let db = setup_db();
        let result = resolve_topology(&db, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_resolve_node_by_name() {
        let mut db = setup_db();
        let topo = Topology::new("my-setup", "Test");
        let node = Node::new(&topo.id, "mac-mini", "desktop");

        db.transaction(|tx| {
            topo.insert(tx)?;
            node.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let resolved = resolve_node(&db, &topo.id, "mac-mini").unwrap();
        assert_eq!(resolved.id, node.id);
        assert_eq!(resolved.name, "mac-mini");
    }

    #[test]
    fn test_resolve_active_topology() {
        let mut db = setup_db();
        let mut topo = Topology::new("active-one", "Active topology");
        topo.tag = Some("current".to_string());

        db.transaction(|tx| {
            topo.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let resolved = resolve_active_topology(&db, None).unwrap();
        assert_eq!(resolved.id, topo.id);
        assert_eq!(resolved.tag.as_deref(), Some("current"));
    }

    #[test]
    fn test_resolve_active_topology_with_override() {
        let mut db = setup_db();
        let mut topo1 = Topology::new("active-one", "Active");
        topo1.tag = Some("current".to_string());
        let topo2 = Topology::new("other-one", "Not active");

        db.transaction(|tx| {
            topo1.insert(tx)?;
            topo2.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let resolved = resolve_active_topology(&db, Some("other-one")).unwrap();
        assert_eq!(resolved.id, topo2.id);
    }

    #[test]
    fn test_resolve_active_topology_none_active() {
        let db = setup_db();
        let result = resolve_active_topology(&db, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No active topology"));
    }

    #[test]
    fn test_resolve_volume_disambiguation() {
        let mut db = setup_db();
        let topo = Topology::new("my-setup", "Test");
        let node1 = Node::new(&topo.id, "mac-mini", "desktop");
        let node2 = Node::new(&topo.id, "nas", "nas");
        let vol1 = Volume::new(&topo.id, &node1.id, "data", 4_000_000_000_000);
        let vol2 = Volume::new(&topo.id, &node2.id, "data", 8_000_000_000_000);

        db.transaction(|tx| {
            topo.insert(tx)?;
            node1.insert(tx)?;
            node2.insert(tx)?;
            vol1.insert(tx)?;
            vol2.insert(tx)?;
            Ok(())
        })
        .unwrap();

        // Without node hint: ambiguous
        let result = resolve_volume(&db, &topo.id, "data", None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("multiple nodes"), "Got: {}", err_msg);
        assert!(err_msg.contains("--node"), "Got: {}", err_msg);

        // With node hint: resolves correctly
        let resolved = resolve_volume(&db, &topo.id, "data", Some("mac-mini")).unwrap();
        assert_eq!(resolved.id, vol1.id);

        let resolved = resolve_volume(&db, &topo.id, "data", Some("nas")).unwrap();
        assert_eq!(resolved.id, vol2.id);
    }

    #[test]
    fn test_resolve_decision_by_title() {
        let mut db = setup_db();
        let decision = Decision::new("NAS Upgrade 2026");
        db.transaction(|tx| {
            decision.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let resolved = resolve_decision(&db, "NAS Upgrade 2026").unwrap();
        assert_eq!(resolved.id, decision.id);
        assert_eq!(resolved.title, "NAS Upgrade 2026");
    }

    #[test]
    fn test_resolve_decision_by_id_prefix() {
        let mut db = setup_db();
        let decision = Decision::new("Storage Choice");
        let id_prefix = decision.id[..8].to_string();

        db.transaction(|tx| {
            decision.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let resolved = resolve_decision(&db, &id_prefix).unwrap();
        assert_eq!(resolved.id, decision.id);
    }

    #[test]
    fn test_resolve_decision_not_found() {
        let db = setup_db();
        let result = resolve_decision(&db, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_resolve_catalog_item_by_name() {
        let mut db = setup_db();
        let item = CatalogItem::new("Samsung 870 EVO 4TB", "ssd");
        db.transaction(|tx| {
            item.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let resolved = resolve_catalog_item(&db, "Samsung 870 EVO 4TB").unwrap();
        assert_eq!(resolved.id, item.id);
        assert_eq!(resolved.name, "Samsung 870 EVO 4TB");
    }

    #[test]
    fn test_resolve_catalog_item_by_id_prefix() {
        let mut db = setup_db();
        let item = CatalogItem::new("WD Red Plus 8TB", "hdd");
        let id_prefix = item.id[..8].to_string();

        db.transaction(|tx| {
            item.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let resolved = resolve_catalog_item(&db, &id_prefix).unwrap();
        assert_eq!(resolved.id, item.id);
    }

    #[test]
    fn test_resolve_catalog_item_not_found() {
        let db = setup_db();
        let result = resolve_catalog_item(&db, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_resolve_catalog_item_prefix_too_short() {
        let mut db = setup_db();
        let item = CatalogItem::new("Test Item", "misc");
        let short_prefix = item.id[..3].to_string();

        db.transaction(|tx| {
            item.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let result = resolve_catalog_item(&db, &short_prefix);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not found"),
            "Expected 'not found' error, got: {}",
            err_msg
        );
    }
}
