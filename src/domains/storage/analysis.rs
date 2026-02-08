//! Storage analysis functions
//!
//! Pure functions that analyze topology data for redundancy, capacity, RPO
//! compliance, and failure simulation. These functions take pre-loaded data and
//! return scored reports -- no database access happens inside analysis functions
//! themselves.
//!
//! The `load_placements_with_context` and `load_sync_regimes_with_context`
//! loaders JOIN across tables to produce enriched data for analysis.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use anyhow::{bail, Result};
use chrono::Utc;
use croner::Cron;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::core::db::Database;
use crate::core::models::{Dataset, DecisionConstraint, Node, Volume};

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
// Enriched sync regime data
// ---------------------------------------------------------------------------

/// A sync regime enriched with source/target volume names and dataset name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRegimeWithContext {
    pub id: String,
    pub dataset_id: String,
    pub dataset_name: String,
    pub source_volume_id: String,
    pub source_volume_name: String,
    pub target_volume_id: String,
    pub target_volume_name: String,
    pub sync_type: String,
    pub schedule: Option<String>,
    pub direction: String,
    pub name: String,
}

/// Load all sync regimes for a topology with volume and dataset names via JOIN.
pub fn load_sync_regimes_with_context(
    db: &Database,
    topology_id: &str,
) -> Result<Vec<SyncRegimeWithContext>> {
    let results = {
        let mut stmt = db.conn().prepare(
            "SELECT
                sr.id,
                sr.dataset_id,
                d.name AS dataset_name,
                sr.source_volume_id,
                sv.name AS source_volume_name,
                sr.target_volume_id,
                tv.name AS target_volume_name,
                sr.sync_type,
                sr.schedule,
                sr.direction,
                sr.name
            FROM sync_regimes sr
            JOIN datasets d ON sr.dataset_id = d.id
            JOIN volumes sv ON sr.source_volume_id = sv.id
            JOIN volumes tv ON sr.target_volume_id = tv.id
            WHERE sr.topology_id = ?1",
        )?;

        let rows = stmt.query_map(params![topology_id], |row| {
            Ok(SyncRegimeWithContext {
                id: row.get("id")?,
                dataset_id: row.get("dataset_id")?,
                dataset_name: row.get("dataset_name")?,
                source_volume_id: row.get("source_volume_id")?,
                source_volume_name: row.get("source_volume_name")?,
                target_volume_id: row.get("target_volume_id")?,
                target_volume_name: row.get("target_volume_name")?,
                sync_type: row.get("sync_type")?,
                schedule: row.get("schedule")?,
                direction: row.get("direction")?,
                name: row.get("name")?,
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
            suggestion = Some(format!("add {} placement(s)", dataset.min_copies));
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
// RPO analysis
// ---------------------------------------------------------------------------

/// Scored report on RPO (Recovery Point Objective) compliance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpoReport {
    pub score: f64,
    pub issues: Vec<RpoIssue>,
    pub datasets_analyzed: usize,
    pub datasets_ok: usize,
    pub datasets_skipped: Vec<String>,
}

/// A single dataset that fails RPO requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpoIssue {
    pub dataset_name: String,
    pub criticality: String,
    pub max_rpo_hours: i32,
    pub best_sync_interval_hours: Option<f64>,
    pub problem: String,
    pub suggestion: Option<String>,
}

/// Parse a cron expression and return the interval in hours between successive
/// occurrences from the current time. Returns None if the expression is invalid
/// or has no upcoming occurrences.
pub fn cron_interval_hours(schedule: &str) -> Option<f64> {
    let cron = Cron::from_str(schedule).ok()?;
    let now = Utc::now();
    let first = cron.find_next_occurrence(&now, false).ok()?;
    let second = cron.find_next_occurrence(&first, false).ok()?;
    let gap = second.signed_duration_since(first);
    Some(gap.num_seconds() as f64 / 3600.0)
}

/// Analyze RPO compliance for datasets with max_rpo_hours set.
///
/// For each dataset with a max_rpo_hours value, finds the best (smallest)
/// sync interval across all sync regimes for that dataset. Compares against
/// the required max RPO to identify violations.
///
/// Score = (datasets_ok / datasets_analyzed) * 100. If no datasets have
/// max_rpo_hours, score is 100.0.
pub fn analyze_rpo(
    datasets: &[Dataset],
    placements: &[PlacementWithContext],
    sync_regimes: &[SyncRegimeWithContext],
) -> RpoReport {
    let _ = placements; // Reserved for future use (e.g., checking placement coverage)

    let mut issues = Vec::new();
    let mut datasets_analyzed = 0usize;
    let mut datasets_skipped = Vec::new();

    for dataset in datasets {
        let max_rpo = match dataset.max_rpo_hours {
            Some(rpo) => rpo,
            None => {
                datasets_skipped.push(dataset.name.clone());
                continue;
            }
        };

        datasets_analyzed += 1;

        // Find all sync regimes for this dataset
        let ds_regimes: Vec<&SyncRegimeWithContext> = sync_regimes
            .iter()
            .filter(|sr| sr.dataset_id == dataset.id)
            .collect();

        if ds_regimes.is_empty() {
            issues.push(RpoIssue {
                dataset_name: dataset.name.clone(),
                criticality: dataset.criticality.clone(),
                max_rpo_hours: max_rpo,
                best_sync_interval_hours: None,
                problem: "no sync regime configured".to_string(),
                suggestion: Some(format!(
                    "add sync regime with schedule meeting {}h RPO",
                    max_rpo
                )),
            });
            continue;
        }

        // Find best (smallest) sync interval across all regimes
        let mut best_interval: Option<f64> = None;
        let mut has_schedule = false;

        for regime in &ds_regimes {
            if let Some(ref schedule) = regime.schedule {
                has_schedule = true;
                if let Some(interval) = cron_interval_hours(schedule) {
                    best_interval = Some(match best_interval {
                        Some(current) => current.min(interval),
                        None => interval,
                    });
                }
            }
        }

        if !has_schedule {
            issues.push(RpoIssue {
                dataset_name: dataset.name.clone(),
                criticality: dataset.criticality.clone(),
                max_rpo_hours: max_rpo,
                best_sync_interval_hours: None,
                problem: "no scheduled sync (manual only)".to_string(),
                suggestion: Some(format!(
                    "add sync regime with schedule meeting {}h RPO",
                    max_rpo
                )),
            });
            continue;
        }

        if let Some(interval) = best_interval {
            if interval > max_rpo as f64 {
                issues.push(RpoIssue {
                    dataset_name: dataset.name.clone(),
                    criticality: dataset.criticality.clone(),
                    max_rpo_hours: max_rpo,
                    best_sync_interval_hours: Some(interval),
                    problem: format!(
                        "sync interval ({:.1}h) exceeds max RPO ({}h)",
                        interval, max_rpo
                    ),
                    suggestion: Some(format!(
                        "increase sync frequency to at most every {}h",
                        max_rpo
                    )),
                });
            }
        }
    }

    let datasets_ok = datasets_analyzed - issues.len();
    let score = if datasets_analyzed == 0 {
        100.0
    } else {
        (datasets_ok as f64 / datasets_analyzed as f64) * 100.0
    };

    RpoReport {
        score,
        issues,
        datasets_analyzed,
        datasets_ok,
        datasets_skipped,
    }
}

// ---------------------------------------------------------------------------
// Failure simulation
// ---------------------------------------------------------------------------

/// Severity tier for a dataset affected by node failure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum FailureSeverity {
    /// All copies lost -- dataset is gone
    Lost,
    /// Some copies lost but dataset still exists
    Degraded,
    /// Copies meet min_copies but location requirements broken
    AtRisk,
}

/// Report on simulated node failure impact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureReport {
    pub failed_nodes: Vec<String>,
    pub volume_impact: Vec<VolumeImpact>,
    pub dataset_impact: Vec<DatasetImpact>,
    pub summary: FailureSummary,
}

/// A volume on a failed node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeImpact {
    pub volume_name: String,
    pub node_name: String,
    pub capacity_bytes: i64,
    pub datasets_hosted: usize,
}

/// A dataset affected by node failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetImpact {
    pub dataset_name: String,
    pub criticality: String,
    pub severity: FailureSeverity,
    pub total_copies: i32,
    pub remaining_copies: i32,
    pub total_locations: i32,
    pub remaining_locations: i32,
    pub lost_volumes: Vec<String>,
}

/// Summary counts for failure simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureSummary {
    pub total_volumes_lost: usize,
    pub datasets_lost: usize,
    pub datasets_degraded: usize,
    pub datasets_at_risk: usize,
}

impl std::fmt::Display for FailureSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailureSeverity::Lost => write!(f, "LOST"),
            FailureSeverity::Degraded => write!(f, "DEGRADED"),
            FailureSeverity::AtRisk => write!(f, "AT RISK"),
        }
    }
}

/// Simulate one or more node failures and compute impact on volumes and datasets.
///
/// Resolves node names to IDs, finds all volumes on failed nodes, and for each
/// dataset determines how many copies and locations remain after the failure.
/// Only datasets with at least one placement on a failed node are included.
pub fn simulate_failure(
    failed_node_names: &[String],
    nodes: &[Node],
    datasets: &[Dataset],
    placements: &[PlacementWithContext],
) -> Result<FailureReport> {
    // Resolve node names to IDs
    let mut failed_node_ids = HashSet::new();
    for name in failed_node_names {
        let node = nodes.iter().find(|n| n.name == *name);
        match node {
            Some(n) => {
                failed_node_ids.insert(n.id.clone());
            }
            None => {
                bail!("node '{}' not found", name);
            }
        }
    }

    // Find volumes on failed nodes (from placements)
    let mut volume_info: HashMap<String, (&str, &str, i64)> = HashMap::new(); // volume_id -> (volume_name, node_name, capacity)
    let mut volume_dataset_count: HashMap<String, HashSet<String>> = HashMap::new(); // volume_id -> dataset_ids

    for p in placements {
        if failed_node_ids.contains(&p.node_id) {
            volume_info.entry(p.volume_id.clone()).or_insert((
                &p.volume_name,
                &p.node_name,
                p.capacity_bytes,
            ));
            volume_dataset_count
                .entry(p.volume_id.clone())
                .or_default()
                .insert(p.dataset_id.clone());
        }
    }

    let mut volume_impact: Vec<VolumeImpact> = volume_info
        .iter()
        .map(|(vid, (vname, nname, cap))| VolumeImpact {
            volume_name: vname.to_string(),
            node_name: nname.to_string(),
            capacity_bytes: *cap,
            datasets_hosted: volume_dataset_count.get(vid).map(|s| s.len()).unwrap_or(0),
        })
        .collect();
    volume_impact.sort_by(|a, b| a.volume_name.cmp(&b.volume_name));

    // Dataset impact
    let mut dataset_impact = Vec::new();

    for dataset in datasets {
        let ds_placements: Vec<&PlacementWithContext> = placements
            .iter()
            .filter(|p| p.dataset_id == dataset.id)
            .collect();

        if ds_placements.is_empty() {
            continue;
        }

        // Check if any placements are on failed nodes
        let lost_placements: Vec<&&PlacementWithContext> = ds_placements
            .iter()
            .filter(|p| failed_node_ids.contains(&p.node_id))
            .collect();

        if lost_placements.is_empty() {
            continue; // Not affected
        }

        let total_copies = ds_placements.len() as i32;
        let total_locations = count_distinct_locations(&ds_placements.to_vec()) as i32;

        let remaining_placements: Vec<&PlacementWithContext> = ds_placements
            .iter()
            .filter(|p| !failed_node_ids.contains(&p.node_id))
            .copied()
            .collect();

        let remaining_copies = remaining_placements.len() as i32;
        let remaining_locations = if remaining_placements.is_empty() {
            0i32
        } else {
            count_distinct_locations(&remaining_placements.to_vec()) as i32
        };

        let lost_volumes: Vec<String> = lost_placements
            .iter()
            .map(|p| p.volume_name.clone())
            .collect();

        let severity = if remaining_copies == 0 {
            FailureSeverity::Lost
        } else if remaining_copies < dataset.min_copies
            || remaining_locations < dataset.min_locations
        {
            // Check if this is "at risk" (copies still meet min but locations don't)
            // vs "degraded" (copies dropped below minimum)
            if remaining_copies >= dataset.min_copies && remaining_locations < dataset.min_locations
            {
                FailureSeverity::AtRisk
            } else {
                FailureSeverity::Degraded
            }
        } else if remaining_copies < total_copies {
            // Lost copies but still meeting all minimums -- still degraded
            FailureSeverity::Degraded
        } else {
            continue; // Not meaningfully affected
        };

        dataset_impact.push(DatasetImpact {
            dataset_name: dataset.name.clone(),
            criticality: dataset.criticality.clone(),
            severity,
            total_copies,
            remaining_copies,
            total_locations,
            remaining_locations,
            lost_volumes,
        });
    }

    // Sort by severity (Lost first)
    dataset_impact.sort_by(|a, b| a.severity.cmp(&b.severity));

    let summary = FailureSummary {
        total_volumes_lost: volume_impact.len(),
        datasets_lost: dataset_impact
            .iter()
            .filter(|d| d.severity == FailureSeverity::Lost)
            .count(),
        datasets_degraded: dataset_impact
            .iter()
            .filter(|d| d.severity == FailureSeverity::Degraded)
            .count(),
        datasets_at_risk: dataset_impact
            .iter()
            .filter(|d| d.severity == FailureSeverity::AtRisk)
            .count(),
    };

    Ok(FailureReport {
        failed_nodes: failed_node_names.to_vec(),
        volume_impact,
        dataset_impact,
        summary,
    })
}

// ---------------------------------------------------------------------------
// Constraint checking
// ---------------------------------------------------------------------------

/// Status of a single constraint check.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConstraintStatus {
    Pass,
    Warn,
    Fail,
}

impl std::fmt::Display for ConstraintStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstraintStatus::Pass => write!(f, "PASS"),
            ConstraintStatus::Warn => write!(f, "WARN"),
            ConstraintStatus::Fail => write!(f, "FAIL"),
        }
    }
}

/// Result of checking one constraint against actual values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintResult {
    pub constraint_type: String,
    pub limit: f64,
    pub actual: f64,
    pub status: ConstraintStatus,
    pub margin: f64,
    pub margin_pct: f64,
}

/// Aggregate constraint checking report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintReport {
    pub score: f64,
    pub results: Vec<ConstraintResult>,
    pub has_failures: bool,
}

/// Check decision constraints against a set of nodes.
///
/// For each constraint, sums the relevant field across all nodes and compares
/// against the constraint's max_value. Returns pass/warn/fail per constraint.
///
/// Status logic:
/// - actual > limit => Fail
/// - actual > limit * 0.9 => Warn
/// - else => Pass
///
/// Score = (passing_count / total_count) * 100.0 (100.0 if no constraints).
pub fn check_constraints(constraints: &[DecisionConstraint], nodes: &[Node]) -> ConstraintReport {
    if constraints.is_empty() {
        return ConstraintReport {
            score: 100.0,
            results: vec![],
            has_failures: false,
        };
    }

    let mut results = Vec::new();

    for constraint in constraints {
        let actual: f64 = match constraint.constraint_type.as_str() {
            "budget" => nodes.iter().filter_map(|n| n.cost_estimate).sum(),
            "noise" => nodes.iter().filter_map(|n| n.noise_db).sum(),
            "power" => nodes.iter().filter_map(|n| n.power_draw_watts).sum(),
            "rack_units" => nodes.iter().filter_map(|n| n.rack_units).sum(),
            _ => 0.0,
        };

        let limit = constraint.max_value;
        let status = if actual > limit {
            ConstraintStatus::Fail
        } else if actual > limit * 0.9 {
            ConstraintStatus::Warn
        } else {
            ConstraintStatus::Pass
        };

        let margin = limit - actual;
        let margin_pct = if limit == 0.0 {
            if actual == 0.0 {
                0.0
            } else {
                -100.0
            }
        } else {
            (margin / limit) * 100.0
        };

        results.push(ConstraintResult {
            constraint_type: constraint.constraint_type.clone(),
            limit,
            actual,
            status,
            margin,
            margin_pct,
        });
    }

    let passing_count = results
        .iter()
        .filter(|r| r.status != ConstraintStatus::Fail)
        .count();
    let score = (passing_count as f64 / results.len() as f64) * 100.0;
    let has_failures = results.iter().any(|r| r.status == ConstraintStatus::Fail);

    ConstraintReport {
        score,
        results,
        has_failures,
    }
}

// ---------------------------------------------------------------------------
// Topology comparison
// ---------------------------------------------------------------------------

/// Aggregated metrics for a single topology.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyMetrics {
    pub name: String,
    pub id: String,
    pub node_count: usize,
    pub volume_count: usize,
    pub total_capacity_bytes: i64,
    pub total_usable_bytes: i64,
    pub dataset_count: usize,
    pub total_cost_estimate: f64,
    pub total_noise_db: f64,
    pub total_power_watts: f64,
    pub total_rack_units: f64,
    pub redundancy_score: f64,
    pub capacity_score: f64,
    pub rpo_score: f64,
}

/// Comparison of a single metric between two topologies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricComparison {
    pub metric: String,
    pub a: f64,
    pub b: f64,
    pub better: String,
    pub unit: String,
}

/// Full comparison report between two topologies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub topology_a: TopologyMetrics,
    pub topology_b: TopologyMetrics,
    pub metrics_comparison: Vec<MetricComparison>,
    pub constraints_a: Option<ConstraintReport>,
    pub constraints_b: Option<ConstraintReport>,
}

/// Compute aggregated metrics for a topology from its constituent data.
#[allow(clippy::too_many_arguments)]
pub fn compute_topology_metrics(
    name: &str,
    id: &str,
    nodes: &[Node],
    volumes: &[Volume],
    datasets: &[Dataset],
    placements: &[PlacementWithContext],
    sync_regimes: &[SyncRegimeWithContext],
    warn_months: i32,
) -> TopologyMetrics {
    let node_count = nodes.len();
    let volume_count = volumes.len();
    let total_capacity_bytes: i64 = volumes.iter().map(|v| v.capacity_bytes).sum();
    let total_usable_bytes: i64 = volumes
        .iter()
        .map(|v| v.usable_bytes.unwrap_or(v.capacity_bytes))
        .sum();
    let dataset_count = datasets.len();
    let total_cost_estimate: f64 = nodes.iter().filter_map(|n| n.cost_estimate).sum();
    let total_noise_db: f64 = nodes.iter().filter_map(|n| n.noise_db).sum();
    let total_power_watts: f64 = nodes.iter().filter_map(|n| n.power_draw_watts).sum();
    let total_rack_units: f64 = nodes.iter().filter_map(|n| n.rack_units).sum();
    let redundancy_score = analyze_redundancy(datasets, placements).score;
    let capacity_score = analyze_capacity(datasets, volumes, placements, warn_months).score;
    let rpo_score = analyze_rpo(datasets, placements, sync_regimes).score;

    TopologyMetrics {
        name: name.to_string(),
        id: id.to_string(),
        node_count,
        volume_count,
        total_capacity_bytes,
        total_usable_bytes,
        dataset_count,
        total_cost_estimate,
        total_noise_db,
        total_power_watts,
        total_rack_units,
        redundancy_score,
        capacity_score,
        rpo_score,
    }
}

/// Compare two topologies across all standard metrics.
///
/// For each metric, determines which topology is "better":
/// - Lower is better: cost, noise, power, rack_units
/// - Higher is better: capacity, usable, redundancy, capacity score, rpo
/// - Neutral (no better): nodes, volumes, datasets
pub fn compare_topologies(
    metrics_a: &TopologyMetrics,
    metrics_b: &TopologyMetrics,
    constraints_a: Option<ConstraintReport>,
    constraints_b: Option<ConstraintReport>,
) -> ComparisonReport {
    let mut comparisons = Vec::new();

    // Helper: lower is better
    let lower_better = |metric: &str, a: f64, b: f64, unit: &str| MetricComparison {
        metric: metric.to_string(),
        a,
        b,
        better: if (a - b).abs() < f64::EPSILON {
            "tie".to_string()
        } else if a < b {
            "a".to_string()
        } else {
            "b".to_string()
        },
        unit: unit.to_string(),
    };

    // Helper: higher is better
    let higher_better = |metric: &str, a: f64, b: f64, unit: &str| MetricComparison {
        metric: metric.to_string(),
        a,
        b,
        better: if (a - b).abs() < f64::EPSILON {
            "tie".to_string()
        } else if a > b {
            "a".to_string()
        } else {
            "b".to_string()
        },
        unit: unit.to_string(),
    };

    // Helper: neutral (no better)
    let neutral = |metric: &str, a: f64, b: f64, unit: &str| MetricComparison {
        metric: metric.to_string(),
        a,
        b,
        better: "tie".to_string(),
        unit: unit.to_string(),
    };

    comparisons.push(lower_better(
        "total_cost",
        metrics_a.total_cost_estimate,
        metrics_b.total_cost_estimate,
        "$",
    ));
    comparisons.push(lower_better(
        "total_noise",
        metrics_a.total_noise_db,
        metrics_b.total_noise_db,
        "dB",
    ));
    comparisons.push(lower_better(
        "total_power",
        metrics_a.total_power_watts,
        metrics_b.total_power_watts,
        "W",
    ));
    comparisons.push(lower_better(
        "total_rack_units",
        metrics_a.total_rack_units,
        metrics_b.total_rack_units,
        "U",
    ));
    comparisons.push(higher_better(
        "total_capacity",
        metrics_a.total_capacity_bytes as f64,
        metrics_b.total_capacity_bytes as f64,
        "bytes",
    ));
    comparisons.push(higher_better(
        "total_usable",
        metrics_a.total_usable_bytes as f64,
        metrics_b.total_usable_bytes as f64,
        "bytes",
    ));
    comparisons.push(higher_better(
        "redundancy",
        metrics_a.redundancy_score,
        metrics_b.redundancy_score,
        "%",
    ));
    comparisons.push(higher_better(
        "capacity",
        metrics_a.capacity_score,
        metrics_b.capacity_score,
        "%",
    ));
    comparisons.push(higher_better(
        "rpo",
        metrics_a.rpo_score,
        metrics_b.rpo_score,
        "%",
    ));
    comparisons.push(neutral(
        "nodes",
        metrics_a.node_count as f64,
        metrics_b.node_count as f64,
        "",
    ));
    comparisons.push(neutral(
        "volumes",
        metrics_a.volume_count as f64,
        metrics_b.volume_count as f64,
        "",
    ));
    comparisons.push(neutral(
        "datasets",
        metrics_a.dataset_count as f64,
        metrics_b.dataset_count as f64,
        "",
    ));

    ComparisonReport {
        topology_a: metrics_a.clone(),
        topology_b: metrics_b.clone(),
        metrics_comparison: comparisons,
        constraints_a,
        constraints_b,
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

    /// Helper to create a PlacementWithContext with a specific node (for failure sim tests)
    fn make_placement_on_node(
        dataset: &Dataset,
        volume: &Volume,
        node: &Node,
    ) -> PlacementWithContext {
        PlacementWithContext {
            placement_id: uuid::Uuid::new_v4().to_string(),
            dataset_id: dataset.id.clone(),
            dataset_name: dataset.name.clone(),
            volume_id: volume.id.clone(),
            volume_name: volume.name.clone(),
            node_id: node.id.clone(),
            node_name: node.name.clone(),
            node_location: node.location.clone(),
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

    /// Helper to create a SyncRegimeWithContext for testing
    fn make_sync_regime(dataset: &Dataset, schedule: Option<&str>) -> SyncRegimeWithContext {
        SyncRegimeWithContext {
            id: uuid::Uuid::new_v4().to_string(),
            dataset_id: dataset.id.clone(),
            dataset_name: dataset.name.clone(),
            source_volume_id: "src-vol".to_string(),
            source_volume_name: "pool-1".to_string(),
            target_volume_id: "tgt-vol".to_string(),
            target_volume_name: "pool-2".to_string(),
            sync_type: "rsync".to_string(),
            schedule: schedule.map(|s| s.to_string()),
            direction: "push".to_string(),
            name: format!("sync-{}", dataset.name),
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
            1_000_000_000_000,       // 1TB used
            Some(100_000_000_000.0), // 100GB/month growth
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

    // -----------------------------------------------------------------------
    // Cron interval tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_cron_interval_hours() {
        let interval = cron_interval_hours("0 */6 * * *").unwrap();
        assert!(
            (interval - 6.0).abs() < 0.1,
            "expected ~6h, got {}",
            interval
        );
    }

    // -----------------------------------------------------------------------
    // RPO tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_rpo_all_compliant() {
        let mut ds = make_dataset("t1", "photos", 500_000_000_000, None, "critical", 2, 2);
        ds.max_rpo_hours = Some(24);

        let sync = make_sync_regime(&ds, Some("0 */4 * * *")); // every 4h

        let report = analyze_rpo(&[ds], &[], &[sync]);
        assert_eq!(report.score, 100.0);
        assert!(report.issues.is_empty());
        assert_eq!(report.datasets_analyzed, 1);
        assert_eq!(report.datasets_ok, 1);
    }

    #[test]
    fn test_rpo_violation() {
        let mut ds = make_dataset("t1", "photos", 500_000_000_000, None, "critical", 2, 2);
        ds.max_rpo_hours = Some(4);

        // Daily at 2am -- ~24h gap, exceeds 4h RPO
        let sync = make_sync_regime(&ds, Some("0 2 * * *"));

        let report = analyze_rpo(&[ds], &[], &[sync]);
        assert_eq!(report.score, 0.0);
        assert_eq!(report.issues.len(), 1);
        assert!(report.issues[0].problem.contains("exceeds max RPO"));
        assert!(report.issues[0].best_sync_interval_hours.unwrap() > 4.0);
    }

    #[test]
    fn test_rpo_no_sync() {
        let mut ds = make_dataset("t1", "photos", 500_000_000_000, None, "critical", 2, 2);
        ds.max_rpo_hours = Some(24);

        // No sync regimes at all
        let report = analyze_rpo(&[ds], &[], &[]);
        assert_eq!(report.score, 0.0);
        assert_eq!(report.issues.len(), 1);
        assert!(report.issues[0]
            .problem
            .contains("no sync regime configured"));
    }

    #[test]
    fn test_rpo_no_schedule() {
        let mut ds = make_dataset("t1", "photos", 500_000_000_000, None, "critical", 2, 2);
        ds.max_rpo_hours = Some(24);

        let sync = make_sync_regime(&ds, None); // manual only

        let report = analyze_rpo(&[ds], &[], &[sync]);
        assert_eq!(report.score, 0.0);
        assert_eq!(report.issues.len(), 1);
        assert!(report.issues[0]
            .problem
            .contains("no scheduled sync (manual only)"));
    }

    #[test]
    fn test_rpo_skip_no_max_rpo() {
        let ds = make_dataset("t1", "photos", 500_000_000_000, None, "normal", 1, 1);
        // No max_rpo_hours set

        let report = analyze_rpo(std::slice::from_ref(&ds), &[], &[]);
        assert_eq!(report.score, 100.0);
        assert!(report.issues.is_empty());
        assert_eq!(report.datasets_analyzed, 0);
        assert_eq!(report.datasets_skipped.len(), 1);
        assert_eq!(report.datasets_skipped[0], "photos");
    }

    // -----------------------------------------------------------------------
    // Failure simulation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_failure_single_node() {
        let mut node1 = Node::new("t1", "nas-01", "nas");
        node1.location = "office".to_string();
        let mut node2 = Node::new("t1", "nas-02", "nas");
        node2.location = "closet".to_string();

        let ds = make_dataset("t1", "photos", 500_000_000_000, None, "critical", 2, 2);
        let vol1 = Volume::new("t1", &node1.id, "pool-1", 4_000_000_000_000);
        let vol2 = Volume::new("t1", &node2.id, "pool-2", 4_000_000_000_000);

        let placements = vec![
            make_placement_on_node(&ds, &vol1, &node1),
            make_placement_on_node(&ds, &vol2, &node2),
        ];

        let report =
            simulate_failure(&["nas-01".to_string()], &[node1, node2], &[ds], &placements).unwrap();

        assert_eq!(report.dataset_impact.len(), 1);
        assert_eq!(report.dataset_impact[0].severity, FailureSeverity::Degraded);
        assert_eq!(report.dataset_impact[0].remaining_copies, 1);
        assert_eq!(report.summary.datasets_degraded, 1);
    }

    #[test]
    fn test_failure_total_loss() {
        let mut node1 = Node::new("t1", "nas-01", "nas");
        node1.location = "office".to_string();

        let ds = make_dataset("t1", "photos", 500_000_000_000, None, "critical", 1, 1);
        let vol1 = Volume::new("t1", &node1.id, "pool-1", 4_000_000_000_000);

        let placements = vec![make_placement_on_node(&ds, &vol1, &node1)];

        let report =
            simulate_failure(&["nas-01".to_string()], &[node1], &[ds], &placements).unwrap();

        assert_eq!(report.dataset_impact.len(), 1);
        assert_eq!(report.dataset_impact[0].severity, FailureSeverity::Lost);
        assert_eq!(report.dataset_impact[0].remaining_copies, 0);
        assert_eq!(report.summary.datasets_lost, 1);
    }

    #[test]
    fn test_failure_multi_node() {
        let mut node1 = Node::new("t1", "nas-01", "nas");
        node1.location = "office".to_string();
        let mut node2 = Node::new("t1", "nas-02", "nas");
        node2.location = "closet".to_string();

        let ds = make_dataset("t1", "photos", 500_000_000_000, None, "critical", 2, 2);
        let vol1 = Volume::new("t1", &node1.id, "pool-1", 4_000_000_000_000);
        let vol2 = Volume::new("t1", &node2.id, "pool-2", 4_000_000_000_000);

        let placements = vec![
            make_placement_on_node(&ds, &vol1, &node1),
            make_placement_on_node(&ds, &vol2, &node2),
        ];

        let report = simulate_failure(
            &["nas-01".to_string(), "nas-02".to_string()],
            &[node1, node2],
            &[ds],
            &placements,
        )
        .unwrap();

        assert_eq!(report.dataset_impact.len(), 1);
        assert_eq!(report.dataset_impact[0].severity, FailureSeverity::Lost);
        assert_eq!(report.summary.datasets_lost, 1);
    }

    #[test]
    fn test_failure_at_risk() {
        // Dataset requires 2 locations, has copies on 3 nodes (2 in office, 1 in closet).
        // Fail the closet node -> remaining copies still >= min_copies(2),
        // but remaining locations drop to 1 (only office) < min_locations(2).
        let mut node1 = Node::new("t1", "nas-01", "nas");
        node1.location = "office".to_string();
        let mut node2 = Node::new("t1", "nas-02", "nas");
        node2.location = "office".to_string();
        let mut node3 = Node::new("t1", "nas-03", "nas");
        node3.location = "closet".to_string();

        let ds = make_dataset("t1", "photos", 500_000_000_000, None, "critical", 2, 2);
        let vol1 = Volume::new("t1", &node1.id, "pool-1", 4_000_000_000_000);
        let vol2 = Volume::new("t1", &node2.id, "pool-2", 4_000_000_000_000);
        let vol3 = Volume::new("t1", &node3.id, "pool-3", 4_000_000_000_000);

        let placements = vec![
            make_placement_on_node(&ds, &vol1, &node1),
            make_placement_on_node(&ds, &vol2, &node2),
            make_placement_on_node(&ds, &vol3, &node3),
        ];

        let report = simulate_failure(
            &["nas-03".to_string()],
            &[node1, node2, node3],
            &[ds],
            &placements,
        )
        .unwrap();

        assert_eq!(report.dataset_impact.len(), 1);
        assert_eq!(report.dataset_impact[0].severity, FailureSeverity::AtRisk);
        assert_eq!(report.dataset_impact[0].remaining_copies, 2);
        assert_eq!(report.dataset_impact[0].remaining_locations, 1);
        assert_eq!(report.summary.datasets_at_risk, 1);
    }

    #[test]
    fn test_failure_no_impact() {
        let mut node1 = Node::new("t1", "nas-01", "nas");
        node1.location = "office".to_string();
        let mut node2 = Node::new("t1", "nas-02", "nas");
        node2.location = "closet".to_string();

        let ds = make_dataset("t1", "photos", 500_000_000_000, None, "normal", 1, 1);
        let vol1 = Volume::new("t1", &node1.id, "pool-1", 4_000_000_000_000);

        // Dataset is only on node1, fail node2 -> no impact
        let placements = vec![make_placement_on_node(&ds, &vol1, &node1)];

        let report =
            simulate_failure(&["nas-02".to_string()], &[node1, node2], &[ds], &placements).unwrap();

        assert!(report.dataset_impact.is_empty());
        assert_eq!(report.summary.datasets_lost, 0);
        assert_eq!(report.summary.datasets_degraded, 0);
        assert_eq!(report.summary.datasets_at_risk, 0);
    }

    // -----------------------------------------------------------------------
    // Constraint checking tests
    // -----------------------------------------------------------------------

    /// Helper to create a DecisionConstraint for testing
    fn make_constraint(constraint_type: &str, max_value: f64) -> DecisionConstraint {
        DecisionConstraint::new("decision-1", constraint_type, max_value)
    }

    /// Helper to create a Node with cost/noise/power/rack_units for testing
    fn make_node_with_attrs(
        name: &str,
        cost: Option<f64>,
        noise: Option<f64>,
        power: Option<f64>,
        rack_units: Option<f64>,
    ) -> Node {
        let mut node = Node::new("t1", name, "desktop");
        node.cost_estimate = cost;
        node.noise_db = noise;
        node.power_draw_watts = power;
        node.rack_units = rack_units;
        node
    }

    #[test]
    fn test_check_constraints_pass() {
        let constraints = vec![
            make_constraint("budget", 1000.0),
            make_constraint("noise", 40.0),
        ];
        let nodes = vec![
            make_node_with_attrs("mac-mini", Some(599.0), Some(10.0), Some(39.0), None),
            make_node_with_attrs("enclosure", Some(150.0), Some(5.0), Some(0.0), None),
        ];

        let report = check_constraints(&constraints, &nodes);
        assert_eq!(report.score, 100.0);
        assert!(!report.has_failures);
        assert_eq!(report.results.len(), 2);
        assert_eq!(report.results[0].status, ConstraintStatus::Pass);
        assert_eq!(report.results[1].status, ConstraintStatus::Pass);
        // budget: 749 / 1000 = 251 margin
        assert!((report.results[0].actual - 749.0).abs() < 0.01);
        assert!((report.results[0].margin - 251.0).abs() < 0.01);
    }

    #[test]
    fn test_check_constraints_warn() {
        // Budget: actual 920 / limit 1000 -- within 10% threshold
        let constraints = vec![make_constraint("budget", 1000.0)];
        let nodes = vec![
            make_node_with_attrs("mac-mini", Some(620.0), None, None, None),
            make_node_with_attrs("enclosure", Some(300.0), None, None, None),
        ];

        let report = check_constraints(&constraints, &nodes);
        assert_eq!(report.results[0].status, ConstraintStatus::Warn);
        assert!(!report.has_failures);
        assert!((report.results[0].actual - 920.0).abs() < 0.01);
    }

    #[test]
    fn test_check_constraints_fail() {
        let constraints = vec![make_constraint("budget", 500.0)];
        let nodes = vec![
            make_node_with_attrs("mac-mini", Some(599.0), None, None, None),
            make_node_with_attrs("enclosure", Some(150.0), None, None, None),
        ];

        let report = check_constraints(&constraints, &nodes);
        assert_eq!(report.score, 0.0);
        assert!(report.has_failures);
        assert_eq!(report.results[0].status, ConstraintStatus::Fail);
        assert!(report.results[0].margin < 0.0);
    }

    #[test]
    fn test_check_constraints_empty() {
        let report = check_constraints(&[], &[]);
        assert_eq!(report.score, 100.0);
        assert!(!report.has_failures);
        assert!(report.results.is_empty());
    }

    #[test]
    fn test_check_constraints_power_uses_power_draw_watts() {
        let constraints = vec![make_constraint("power", 100.0)];
        let nodes = vec![
            make_node_with_attrs("mac-mini", None, None, Some(39.0), None),
            make_node_with_attrs("nas", None, None, Some(45.0), None),
        ];

        let report = check_constraints(&constraints, &nodes);
        assert_eq!(report.results[0].status, ConstraintStatus::Pass);
        assert!((report.results[0].actual - 84.0).abs() < 0.01);
    }

    // -----------------------------------------------------------------------
    // Topology comparison tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_compare_topologies_basic() {
        let metrics_a = TopologyMetrics {
            name: "sata-option".to_string(),
            id: "id-a".to_string(),
            node_count: 2,
            volume_count: 1,
            total_capacity_bytes: 4_000_000_000_000,
            total_usable_bytes: 3_600_000_000_000,
            dataset_count: 1,
            total_cost_estimate: 800.0,
            total_noise_db: 0.0,
            total_power_watts: 39.0,
            total_rack_units: 0.0,
            redundancy_score: 100.0,
            capacity_score: 100.0,
            rpo_score: 100.0,
        };

        let metrics_b = TopologyMetrics {
            name: "nvme-option".to_string(),
            id: "id-b".to_string(),
            node_count: 2,
            volume_count: 2,
            total_capacity_bytes: 8_000_000_000_000,
            total_usable_bytes: 7_200_000_000_000,
            dataset_count: 1,
            total_cost_estimate: 1200.0,
            total_noise_db: 5.0,
            total_power_watts: 50.0,
            total_rack_units: 2.0,
            redundancy_score: 75.0,
            capacity_score: 50.0,
            rpo_score: 100.0,
        };

        let report = compare_topologies(&metrics_a, &metrics_b, None, None);
        assert_eq!(report.metrics_comparison.len(), 12);

        // Cost: A is cheaper => better = "a"
        let cost = report
            .metrics_comparison
            .iter()
            .find(|m| m.metric == "total_cost")
            .unwrap();
        assert_eq!(cost.better, "a");

        // Capacity: B has more => better = "b"
        let cap = report
            .metrics_comparison
            .iter()
            .find(|m| m.metric == "total_capacity")
            .unwrap();
        assert_eq!(cap.better, "b");

        // Redundancy: A has higher score => better = "a"
        let red = report
            .metrics_comparison
            .iter()
            .find(|m| m.metric == "redundancy")
            .unwrap();
        assert_eq!(red.better, "a");

        // RPO: same score => tie
        let rpo = report
            .metrics_comparison
            .iter()
            .find(|m| m.metric == "rpo")
            .unwrap();
        assert_eq!(rpo.better, "tie");

        // Nodes: neutral => always tie
        let nodes = report
            .metrics_comparison
            .iter()
            .find(|m| m.metric == "nodes")
            .unwrap();
        assert_eq!(nodes.better, "tie");

        // Constraints should be None
        assert!(report.constraints_a.is_none());
        assert!(report.constraints_b.is_none());
    }
}
