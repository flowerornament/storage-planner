//! Storage analysis functions
//!
//! Pure functions that analyze topology data for redundancy and capacity issues.
//! These functions take pre-loaded data and return scored reports -- no database
//! access happens inside analysis functions themselves.
//!
//! The `load_placements_with_context` loader JOINs across placements, datasets,
//! volumes, and nodes to produce enriched placement data for analysis.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::core::db::Database;
use crate::core::models::{Dataset, Volume};

// ---------------------------------------------------------------------------
// Enriched placement data
// ---------------------------------------------------------------------------

/// A placement enriched with dataset, volume, and node context via JOIN.
/// Used by all analysis functions to avoid repeated lookups.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementWithContext {
    pub placement_id: String,
    pub dataset_id: String,
    pub dataset_name: String,
    pub volume_id: String,
    pub volume_name: String,
    pub node_id: String,
    pub node_name: String,
    pub node_location: String,
    pub role: String,
    pub capacity_bytes: i64,
    pub usable_bytes: Option<i64>,
    pub size_bytes: i64,
    pub growth_rate_bytes_month: Option<f64>,
    pub criticality: String,
    pub min_copies: i32,
    pub min_locations: i32,
    pub max_rpo_hours: Option<i32>,
}

/// Load all placements for a topology with full context from JOINed tables.
///
/// Uses block-scoped prepared statements per D023 pattern.
pub fn load_placements_with_context(
    db: &Database,
    topology_id: &str,
) -> Result<Vec<PlacementWithContext>> {
    let results = {
        let mut stmt = db.conn().prepare(
            "SELECT
                p.id AS placement_id,
                p.dataset_id,
                d.name AS dataset_name,
                p.volume_id,
                v.name AS volume_name,
                v.node_id,
                n.name AS node_name,
                n.location AS node_location,
                p.role,
                v.capacity_bytes,
                v.usable_bytes,
                d.size_bytes,
                d.growth_rate_bytes_month,
                d.criticality,
                d.min_copies,
                d.min_locations,
                d.max_rpo_hours
            FROM placements p
            JOIN datasets d ON p.dataset_id = d.id
            JOIN volumes v ON p.volume_id = v.id
            JOIN nodes n ON v.node_id = n.id
            WHERE p.topology_id = ?1",
        )?;

        let rows = stmt.query_map(params![topology_id], |row| {
            Ok(PlacementWithContext {
                placement_id: row.get("placement_id")?,
                dataset_id: row.get("dataset_id")?,
                dataset_name: row.get("dataset_name")?,
                volume_id: row.get("volume_id")?,
                volume_name: row.get("volume_name")?,
                node_id: row.get("node_id")?,
                node_name: row.get("node_name")?,
                node_location: row.get("node_location")?,
                role: row.get("role")?,
                capacity_bytes: row.get("capacity_bytes")?,
                usable_bytes: row.get("usable_bytes")?,
                size_bytes: row.get("size_bytes")?,
                growth_rate_bytes_month: row.get("growth_rate_bytes_month")?,
                criticality: row.get("criticality")?,
                min_copies: row.get("min_copies")?,
                min_locations: row.get("min_locations")?,
                max_rpo_hours: row.get("max_rpo_hours")?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()?
    };

    Ok(results)
}

// ---------------------------------------------------------------------------
// Redundancy analysis
// ---------------------------------------------------------------------------

/// Scored report on dataset redundancy compliance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedundancyReport {
    pub score: f64,
    pub issues: Vec<RedundancyIssue>,
    pub dataset_count: usize,
    pub ok_count: usize,
}

/// A single dataset that fails redundancy requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedundancyIssue {
    pub dataset_name: String,
    pub criticality: String,
    pub required_copies: i32,
    pub actual_copies: usize,
    pub required_locations: i32,
    pub actual_locations: usize,
    pub problems: Vec<String>,
    pub suggestion: Option<String>,
}

/// Analyze dataset redundancy against min_copies and min_locations requirements.
///
/// For each dataset, counts actual placements (copies) and distinct node locations.
/// Empty string locations each count as separate unknown locations to avoid
/// false-positive "same location" matches.
///
/// Score = (datasets meeting ALL requirements / total datasets) * 100.
/// If no datasets exist, score = 100.0.
pub fn analyze_redundancy(
    datasets: &[Dataset],
    placements: &[PlacementWithContext],
) -> RedundancyReport {
    if datasets.is_empty() {
        return RedundancyReport {
            score: 100.0,
            issues: vec![],
            dataset_count: 0,
            ok_count: 0,
        };
    }

    let mut issues = Vec::new();

    for dataset in datasets {
        let ds_placements: Vec<&PlacementWithContext> = placements
            .iter()
            .filter(|p| p.dataset_id == dataset.id)
            .collect();

        let actual_copies = ds_placements.len();

        // Count distinct locations. Empty string locations each count as
        // separate unknowns so we don't accidentally merge them.
        let actual_locations = count_distinct_locations(&ds_placements);

        let mut problems = Vec::new();
        let mut suggestion = None;

        if actual_copies == 0 {
            problems.push("no placements -- dataset is unplaced".to_string());
            suggestion = Some(format!(
                "add {} placement(s)",
                dataset.min_copies
            ));
        } else {
            if (actual_copies as i32) < dataset.min_copies {
                let needed = dataset.min_copies as usize - actual_copies;
                problems.push(format!(
                    "needs {} copies, has {}",
                    dataset.min_copies, actual_copies
                ));
                suggestion = Some(format!("add {} more placement(s)", needed));
            }
            if (actual_locations as i32) < dataset.min_locations {
                problems.push(format!(
                    "needs {} locations, has {}",
                    dataset.min_locations, actual_locations
                ));
                // Location suggestion takes precedence if both are issues
                suggestion = Some("add placement on volume in different location".to_string());
            }
        }

        if !problems.is_empty() {
            issues.push(RedundancyIssue {
                dataset_name: dataset.name.clone(),
                criticality: dataset.criticality.clone(),
                required_copies: dataset.min_copies,
                actual_copies,
                required_locations: dataset.min_locations,
                actual_locations,
                problems,
                suggestion,
            });
        }
    }

    let ok_count = datasets.len() - issues.len();
    let score = (ok_count as f64 / datasets.len() as f64) * 100.0;

    RedundancyReport {
        score,
        issues,
        dataset_count: datasets.len(),
        ok_count,
    }
}

/// Count distinct locations from placements. Empty string locations are each
/// counted as separate unknowns.
fn count_distinct_locations(placements: &[&PlacementWithContext]) -> usize {
    let mut known_locations = HashSet::new();
    let mut empty_count = 0usize;

    for p in placements {
        if p.node_location.is_empty() {
            empty_count += 1;
        } else {
            known_locations.insert(&p.node_location);
        }
    }

    known_locations.len() + empty_count
}

// ---------------------------------------------------------------------------
// Capacity analysis
// ---------------------------------------------------------------------------

/// Scored report on capacity projections across volumes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityReport {
    pub score: f64,
    pub issues: Vec<CapacityIssue>,
    pub projections: Vec<VolumeProjection>,
    pub skipped_datasets: Vec<String>,
}

/// A volume projected to fill within the warning threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityIssue {
    pub volume_name: String,
    pub node_name: String,
    pub months_until_full: f64,
    pub ceiling_bytes: i64,
    pub warn_threshold_months: i32,
}

/// Projected capacity timeline for a volume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeProjection {
    pub volume_name: String,
    pub node_name: String,
    pub current_used_bytes: i64,
    pub ceiling_bytes: i64,
    pub monthly_growth_bytes: f64,
    pub months_until_full: Option<f64>,
    pub timeline: Vec<TimelinePoint>,
}

/// A single point in a capacity timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelinePoint {
    pub months: i32,
    pub projected_bytes: i64,
    pub label: String,
}

/// Analyze capacity usage and project time-until-full for each volume.
///
/// For each volume, sums size_bytes of placed datasets (current usage) and
/// growth_rate_bytes_month of datasets with growth data (monthly growth).
///
/// Ceiling = volume.usable_bytes.unwrap_or(volume.capacity_bytes).
/// months_until_full = (ceiling - current) / monthly_growth if growth > 0.
///
/// Score = (volumes NOT within warn_months / total volumes with growth data) * 100.
/// Volumes with zero growth data are excluded from scoring but included in projections.
pub fn analyze_capacity(
    datasets: &[Dataset],
    volumes: &[Volume],
    placements: &[PlacementWithContext],
    warn_months: i32,
) -> CapacityReport {
    // Build dataset lookup by ID
    let dataset_map: HashMap<&str, &Dataset> =
        datasets.iter().map(|d| (d.id.as_str(), d)).collect();

    // Track which datasets lack growth_rate across all volumes
    let mut skipped_set: HashSet<String> = HashSet::new();

    // Build node name lookup from placements
    let node_names: HashMap<&str, &str> = placements
        .iter()
        .map(|p| (p.node_id.as_str(), p.node_name.as_str()))
        .collect();

    let mut projections = Vec::new();
    let mut issues = Vec::new();
    let mut volumes_with_growth = 0usize;
    let mut volumes_within_threshold = 0usize;

    for volume in volumes {
        let ceiling = volume.usable_bytes.unwrap_or(volume.capacity_bytes);

        // Find all placements on this volume
        let vol_placements: Vec<&PlacementWithContext> = placements
            .iter()
            .filter(|p| p.volume_id == volume.id)
            .collect();

        // Sum current usage and growth rate
        let mut current_used: i64 = 0;
        let mut monthly_growth: f64 = 0.0;
        let mut has_growth_data = false;

        for p in &vol_placements {
            current_used += p.size_bytes;

            if let Some(ds) = dataset_map.get(p.dataset_id.as_str()) {
                if let Some(rate) = ds.growth_rate_bytes_month {
                    monthly_growth += rate;
                    has_growth_data = true;
                } else {
                    skipped_set.insert(ds.name.clone());
                }
            }
        }

        let months_until_full = if monthly_growth > 0.0 {
            let remaining = ceiling - current_used;
            if remaining > 0 {
                Some(remaining as f64 / monthly_growth)
            } else {
                Some(0.0)
            }
        } else {
            None
        };

        // Generate timeline points
        let timeline = if monthly_growth > 0.0 {
            [3, 6, 12]
                .iter()
                .map(|&m| {
                    let projected = current_used + (monthly_growth * m as f64) as i64;
                    TimelinePoint {
                        months: m,
                        projected_bytes: projected.min(ceiling),
                        label: format!("{}mo", m),
                    }
                })
                .collect()
        } else {
            vec![]
        };

        // Determine node name for this volume
        let vol_node_name = node_names
            .get(volume.node_id.as_str())
            .copied()
            .unwrap_or("unknown");

        projections.push(VolumeProjection {
            volume_name: volume.name.clone(),
            node_name: vol_node_name.to_string(),
            current_used_bytes: current_used,
            ceiling_bytes: ceiling,
            monthly_growth_bytes: monthly_growth,
            months_until_full,
            timeline,
        });

        // Scoring: only volumes with growth data count
        if has_growth_data {
            volumes_with_growth += 1;
            if let Some(mtf) = months_until_full {
                if mtf < warn_months as f64 {
                    volumes_within_threshold += 1;
                    issues.push(CapacityIssue {
                        volume_name: volume.name.clone(),
                        node_name: vol_node_name.to_string(),
                        months_until_full: mtf,
                        ceiling_bytes: ceiling,
                        warn_threshold_months: warn_months,
                    });
                }
            }
        }
    }

    let score = if volumes_with_growth == 0 {
        100.0
    } else {
        ((volumes_with_growth - volumes_within_threshold) as f64 / volumes_with_growth as f64)
            * 100.0
    };

    let mut skipped_datasets: Vec<String> = skipped_set.into_iter().collect();
    skipped_datasets.sort();

    CapacityReport {
        score,
        issues,
        projections,
        skipped_datasets,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::{Dataset, Node, Placement, Topology, Volume};

    /// Helper to create a dataset with specified requirements
    fn make_dataset(
        topo_id: &str,
        name: &str,
        size_bytes: i64,
        growth_rate: Option<f64>,
        criticality: &str,
        min_copies: i32,
        min_locations: i32,
    ) -> Dataset {
        let mut ds = Dataset::new(topo_id, name, size_bytes);
        ds.growth_rate_bytes_month = growth_rate;
        ds.criticality = criticality.to_string();
        ds.min_copies = min_copies;
        ds.min_locations = min_locations;
        ds
    }

    /// Helper to create a PlacementWithContext for testing pure functions
    fn make_placement_ctx(
        dataset: &Dataset,
        volume: &Volume,
        node_name: &str,
        node_location: &str,
    ) -> PlacementWithContext {
        PlacementWithContext {
            placement_id: uuid::Uuid::new_v4().to_string(),
            dataset_id: dataset.id.clone(),
            dataset_name: dataset.name.clone(),
            volume_id: volume.id.clone(),
            volume_name: volume.name.clone(),
            node_id: uuid::Uuid::new_v4().to_string(),
            node_name: node_name.to_string(),
            node_location: node_location.to_string(),
            role: "primary".to_string(),
            capacity_bytes: volume.capacity_bytes,
            usable_bytes: volume.usable_bytes,
            size_bytes: dataset.size_bytes,
            growth_rate_bytes_month: dataset.growth_rate_bytes_month,
            criticality: dataset.criticality.clone(),
            min_copies: dataset.min_copies,
            min_locations: dataset.min_locations,
            max_rpo_hours: dataset.max_rpo_hours,
        }
    }

    // -----------------------------------------------------------------------
    // Redundancy tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_redundancy_all_met() {
        let ds1 = make_dataset("t1", "photos", 500_000_000_000, None, "critical", 2, 2);
        let ds2 = make_dataset("t1", "docs", 100_000_000_000, None, "normal", 1, 1);

        let vol1 = Volume::new("t1", "n1", "pool-1", 4_000_000_000_000);
        let vol2 = Volume::new("t1", "n2", "pool-2", 4_000_000_000_000);
        let vol3 = Volume::new("t1", "n1", "ssd-1", 1_000_000_000_000);

        let placements = vec![
            make_placement_ctx(&ds1, &vol1, "nas-01", "office"),
            make_placement_ctx(&ds1, &vol2, "nas-02", "closet"),
            make_placement_ctx(&ds2, &vol3, "mac-mini", "office"),
        ];

        let report = analyze_redundancy(&[ds1, ds2], &placements);
        assert_eq!(report.score, 100.0);
        assert!(report.issues.is_empty());
        assert_eq!(report.dataset_count, 2);
        assert_eq!(report.ok_count, 2);
    }

    #[test]
    fn test_redundancy_copies_short() {
        let ds = make_dataset("t1", "photos", 500_000_000_000, None, "critical", 3, 1);
        let vol = Volume::new("t1", "n1", "pool-1", 4_000_000_000_000);

        let placements = vec![make_placement_ctx(&ds, &vol, "nas-01", "office")];

        let report = analyze_redundancy(&[ds], &placements);
        assert_eq!(report.score, 0.0);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].actual_copies, 1);
        assert_eq!(report.issues[0].required_copies, 3);
        assert!(report.issues[0].problems[0].contains("needs 3 copies, has 1"));
        assert!(report.issues[0]
            .suggestion
            .as_ref()
            .unwrap()
            .contains("add 2 more placement(s)"));
    }

    #[test]
    fn test_redundancy_locations_short() {
        let ds = make_dataset("t1", "photos", 500_000_000_000, None, "critical", 2, 2);
        let vol1 = Volume::new("t1", "n1", "pool-1", 4_000_000_000_000);
        let vol2 = Volume::new("t1", "n1", "pool-2", 4_000_000_000_000);

        // Both placements on the same location
        let placements = vec![
            make_placement_ctx(&ds, &vol1, "nas-01", "office"),
            make_placement_ctx(&ds, &vol2, "nas-02", "office"),
        ];

        let report = analyze_redundancy(&[ds], &placements);
        assert_eq!(report.score, 0.0);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].actual_locations, 1);
        assert_eq!(report.issues[0].required_locations, 2);
        assert!(report.issues[0]
            .suggestion
            .as_ref()
            .unwrap()
            .contains("different location"));
    }

    #[test]
    fn test_redundancy_no_datasets() {
        let report = analyze_redundancy(&[], &[]);
        assert_eq!(report.score, 100.0);
        assert!(report.issues.is_empty());
        assert_eq!(report.dataset_count, 0);
        assert_eq!(report.ok_count, 0);
    }

    #[test]
    fn test_redundancy_unplaced_dataset() {
        let ds = make_dataset("t1", "photos", 500_000_000_000, None, "critical", 2, 2);

        let report = analyze_redundancy(&[ds], &[]);
        assert_eq!(report.score, 0.0);
        assert_eq!(report.issues.len(), 1);
        assert!(report.issues[0].problems[0].contains("unplaced"));
        assert_eq!(report.issues[0].actual_copies, 0);
    }

    // -----------------------------------------------------------------------
    // Capacity tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_capacity_basic() {
        let ds = make_dataset(
            "t1",
            "photos",
            1_000_000_000_000,              // 1TB used
            Some(100_000_000_000.0),         // 100GB/month growth
            "normal",
            1,
            1,
        );
        let mut vol = Volume::new("t1", "n1", "pool-1", 4_000_000_000_000); // 4TB
        vol.usable_bytes = Some(3_600_000_000_000); // 3.6TB usable

        let node_id = vol.node_id.clone();
        let mut p = make_placement_ctx(&ds, &vol, "nas-01", "office");
        p.node_id = node_id;

        let report = analyze_capacity(&[ds], &[vol], &[p], 12);

        assert_eq!(report.projections.len(), 1);
        let proj = &report.projections[0];
        assert_eq!(proj.current_used_bytes, 1_000_000_000_000);
        assert_eq!(proj.ceiling_bytes, 3_600_000_000_000);
        assert_eq!(proj.monthly_growth_bytes, 100_000_000_000.0);

        // (3.6TB - 1TB) / 100GB = 26 months
        let mtf = proj.months_until_full.unwrap();
        assert!((mtf - 26.0).abs() < 0.01);

        // Not within 12 month threshold, so score should be 100
        assert_eq!(report.score, 100.0);
        assert!(report.issues.is_empty());
    }

    #[test]
    fn test_capacity_no_growth() {
        let ds = make_dataset("t1", "photos", 1_000_000_000_000, None, "normal", 1, 1);
        let vol = Volume::new("t1", "n1", "pool-1", 4_000_000_000_000);

        let node_id = vol.node_id.clone();
        let mut p = make_placement_ctx(&ds, &vol, "nas-01", "office");
        p.node_id = node_id;

        let report = analyze_capacity(std::slice::from_ref(&ds), &[vol], &[p], 12);

        assert_eq!(report.projections.len(), 1);
        assert!(report.projections[0].months_until_full.is_none());
        assert!(report.projections[0].timeline.is_empty());
        // No growth data => score is 100 (excluded from scoring)
        assert_eq!(report.score, 100.0);
        // Dataset should be in skipped list
        assert!(report.skipped_datasets.contains(&ds.name));
    }

    #[test]
    fn test_capacity_within_threshold() {
        let ds = make_dataset(
            "t1",
            "photos",
            3_000_000_000_000,       // 3TB used
            Some(500_000_000_000.0), // 500GB/month growth
            "normal",
            1,
            1,
        );
        let vol = Volume::new("t1", "n1", "pool-1", 4_000_000_000_000); // 4TB

        let node_id = vol.node_id.clone();
        let mut p = make_placement_ctx(&ds, &vol, "nas-01", "office");
        p.node_id = node_id;

        // (4TB - 3TB) / 500GB = 2 months -- well within 12 month threshold
        let report = analyze_capacity(&[ds], &[vol], &[p], 12);

        assert_eq!(report.score, 0.0);
        assert_eq!(report.issues.len(), 1);
        assert!((report.issues[0].months_until_full - 2.0).abs() < 0.01);
        assert_eq!(report.issues[0].warn_threshold_months, 12);
    }

    #[test]
    fn test_capacity_usable_bytes_precedence() {
        let ds = make_dataset(
            "t1",
            "photos",
            1_000_000_000_000,
            Some(100_000_000_000.0),
            "normal",
            1,
            1,
        );

        // capacity_bytes = 4TB, usable_bytes = 2TB -- usable should be used as ceiling
        let mut vol = Volume::new("t1", "n1", "pool-1", 4_000_000_000_000);
        vol.usable_bytes = Some(2_000_000_000_000);

        let node_id = vol.node_id.clone();
        let mut p = make_placement_ctx(&ds, &vol, "nas-01", "office");
        p.node_id = node_id;

        let report = analyze_capacity(&[ds], &[vol], &[p], 24);

        let proj = &report.projections[0];
        assert_eq!(proj.ceiling_bytes, 2_000_000_000_000); // usable, not capacity
        // (2TB - 1TB) / 100GB = 10 months
        let mtf = proj.months_until_full.unwrap();
        assert!((mtf - 10.0).abs() < 0.01);
    }

    // -----------------------------------------------------------------------
    // DB loader test
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_placements_with_context() {
        let mut db = Database::open_memory().unwrap();

        let mut topo = Topology::new("test-setup", "Test topology");
        topo.tag = Some("current".to_string());

        let mut node = Node::new(&topo.id, "nas-01", "nas");
        node.location = "office".to_string();

        let vol = Volume::new(&topo.id, &node.id, "pool-1", 4_000_000_000_000);

        let mut ds = Dataset::new(&topo.id, "photos", 500_000_000_000);
        ds.criticality = "critical".to_string();
        ds.min_copies = 2;
        ds.min_locations = 2;
        ds.growth_rate_bytes_month = Some(50_000_000_000.0);

        let placement = Placement::new(&topo.id, &ds.id, &vol.id);

        db.transaction(|tx| {
            topo.insert(tx)?;
            node.insert(tx)?;
            vol.insert(tx)?;
            ds.insert(tx)?;
            placement.insert(tx)?;
            Ok(())
        })
        .unwrap();

        let results = load_placements_with_context(&db, &topo.id).unwrap();
        assert_eq!(results.len(), 1);

        let p = &results[0];
        assert_eq!(p.dataset_name, "photos");
        assert_eq!(p.volume_name, "pool-1");
        assert_eq!(p.node_name, "nas-01");
        assert_eq!(p.node_location, "office");
        assert_eq!(p.criticality, "critical");
        assert_eq!(p.min_copies, 2);
        assert_eq!(p.min_locations, 2);
        assert_eq!(p.size_bytes, 500_000_000_000);
        assert_eq!(p.growth_rate_bytes_month, Some(50_000_000_000.0));
    }
}
