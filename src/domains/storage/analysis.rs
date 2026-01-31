//! Storage domain analysis functions
//!
//! Pure functions for analyzing storage topologies.

use super::models::{Dataset, Node, SyncRegime, Volume};
use serde::Serialize;

/// Redundancy analysis result
#[derive(Debug, Serialize)]
pub struct RedundancyReport {
    pub single_points_of_failure: Vec<String>,
    pub unprotected_datasets: Vec<String>,
    pub redundancy_level: RedundancyLevel,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RedundancyLevel {
    None,
    Local,    // RAID or local copies
    Offsite,  // Copies on different node
    Geographic, // Copies in different locations
}

/// Analyze redundancy of a storage topology
pub fn analyze_redundancy(
    nodes: &[Node],
    volumes: &[Volume],
    datasets: &[Dataset],
    syncs: &[SyncRegime],
) -> RedundancyReport {
    let mut single_points_of_failure = Vec::new();
    let mut unprotected_datasets = Vec::new();

    // Check for single node failure
    if nodes.len() == 1 {
        single_points_of_failure.push(format!("Single node: {}", nodes[0].id));
    }

    // Check each dataset for protection
    for dataset in datasets {
        let mut copies = 0;
        let mut locations = std::collections::HashSet::new();

        // Count where this dataset exists
        for volume in volumes {
            if volume.datasets.contains(&dataset.id) {
                copies += 1;
                if let Some(node) = nodes.iter().find(|n| n.id == volume.node_id) {
                    locations.insert(&node.location);
                }
            }
        }

        // Also count sync targets
        for sync in syncs {
            if sync.datasets.contains(&dataset.id) {
                if let Some(target_vol) = volumes.iter().find(|v| v.id == sync.target_volume) {
                    if let Some(node) = nodes.iter().find(|n| n.id == target_vol.node_id) {
                        locations.insert(&node.location);
                    }
                }
            }
        }

        if copies < 2 {
            unprotected_datasets.push(dataset.id.clone());
        }
    }

    // Determine overall redundancy level
    let redundancy_level = if unprotected_datasets.len() == datasets.len() {
        RedundancyLevel::None
    } else {
        // Check if we have offsite copies
        let locations: std::collections::HashSet<_> = nodes.iter().map(|n| &n.location).collect();
        if locations.len() > 1 {
            RedundancyLevel::Geographic
        } else if syncs.iter().any(|s| {
            let source_node = volumes
                .iter()
                .find(|v| v.id == s.source_volume)
                .and_then(|v| nodes.iter().find(|n| n.id == v.node_id));
            let target_node = volumes
                .iter()
                .find(|v| v.id == s.target_volume)
                .and_then(|v| nodes.iter().find(|n| n.id == v.node_id));
            source_node.map(|n| &n.id) != target_node.map(|n| &n.id)
        }) {
            RedundancyLevel::Offsite
        } else {
            RedundancyLevel::Local
        }
    };

    RedundancyReport {
        single_points_of_failure,
        unprotected_datasets,
        redundancy_level,
    }
}

/// Capacity analysis result
#[derive(Debug, Serialize)]
pub struct CapacityReport {
    pub total_capacity_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub utilization_percent: f64,
    pub months_until_full: Option<u32>,
}

/// Analyze capacity of a storage topology
pub fn analyze_capacity(volumes: &[Volume], datasets: &[Dataset]) -> CapacityReport {
    let total_capacity_bytes: u64 = volumes.iter().map(|v| v.capacity_bytes).sum();
    let used_bytes: u64 = datasets.iter().map(|d| d.size_bytes).sum();
    let free_bytes = total_capacity_bytes.saturating_sub(used_bytes);

    let utilization_percent = if total_capacity_bytes > 0 {
        (used_bytes as f64 / total_capacity_bytes as f64) * 100.0
    } else {
        0.0
    };

    // Estimate months until full based on growth rate
    let total_growth_per_month: f64 = datasets
        .iter()
        .filter_map(|d| d.growth_rate)
        .sum();

    let months_until_full = if total_growth_per_month > 0.0 && free_bytes > 0 {
        Some((free_bytes as f64 / total_growth_per_month) as u32)
    } else {
        None
    };

    CapacityReport {
        total_capacity_bytes,
        used_bytes,
        free_bytes,
        utilization_percent,
        months_until_full,
    }
}

/// RPO/RTO compliance report
#[derive(Debug, Serialize)]
pub struct RpoRtoReport {
    pub compliant: bool,
    pub violations: Vec<RpoRtoViolation>,
}

#[derive(Debug, Serialize)]
pub struct RpoRtoViolation {
    pub dataset_id: String,
    pub dataset_name: String,
    pub required_rpo_hours: u32,
    pub actual_rpo_hours: Option<u32>,
    pub violation_type: String,
}

/// Analyze RPO/RTO compliance
pub fn analyze_rpo_rto(datasets: &[Dataset], syncs: &[SyncRegime]) -> RpoRtoReport {
    let mut violations = Vec::new();

    for dataset in datasets {
        if let Some(required_rpo) = dataset.rpo_hours {
            // Find syncs that cover this dataset
            let covering_syncs: Vec<_> = syncs
                .iter()
                .filter(|s| s.datasets.contains(&dataset.id))
                .collect();

            if covering_syncs.is_empty() {
                violations.push(RpoRtoViolation {
                    dataset_id: dataset.id.clone(),
                    dataset_name: dataset.name.clone(),
                    required_rpo_hours: required_rpo,
                    actual_rpo_hours: None,
                    violation_type: "No sync configured".to_string(),
                });
            }
            // In a real implementation, we'd parse cron schedules to estimate actual RPO
        }
    }

    RpoRtoReport {
        compliant: violations.is_empty(),
        violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::storage::models::{Criticality, NodeType, SyncType};

    #[test]
    fn test_redundancy_single_node() {
        let nodes = vec![Node {
            id: "desktop".into(),
            name: "Desktop".into(),
            node_type: NodeType::Desktop,
            location: "home".into(),
            volumes: vec!["vol1".into()],
        }];

        let volumes = vec![Volume {
            id: "vol1".into(),
            name: "Main SSD".into(),
            node_id: "desktop".into(),
            item_id: None,
            capacity_bytes: 1_000_000_000_000,
            raid_level: None,
            datasets: vec!["data".into()],
        }];

        let datasets = vec![Dataset {
            id: "data".into(),
            name: "Main Data".into(),
            size_bytes: 500_000_000_000,
            growth_rate: None,
            criticality: Criticality::Normal,
            rpo_hours: None,
            rto_hours: None,
        }];

        let report = analyze_redundancy(&nodes, &volumes, &datasets, &[]);

        assert!(!report.single_points_of_failure.is_empty());
        assert!(!report.unprotected_datasets.is_empty());
    }

    #[test]
    fn test_capacity_analysis() {
        let volumes = vec![
            Volume {
                id: "vol1".into(),
                name: "SSD 1".into(),
                node_id: "node1".into(),
                item_id: None,
                capacity_bytes: 4_000_000_000_000, // 4TB
                raid_level: None,
                datasets: vec![],
            },
            Volume {
                id: "vol2".into(),
                name: "SSD 2".into(),
                node_id: "node1".into(),
                item_id: None,
                capacity_bytes: 4_000_000_000_000, // 4TB
                raid_level: None,
                datasets: vec![],
            },
        ];

        let datasets = vec![Dataset {
            id: "data".into(),
            name: "Data".into(),
            size_bytes: 2_000_000_000_000, // 2TB
            growth_rate: Some(50_000_000_000.0), // 50GB/month
            criticality: Criticality::Normal,
            rpo_hours: None,
            rto_hours: None,
        }];

        let report = analyze_capacity(&volumes, &datasets);

        assert_eq!(report.total_capacity_bytes, 8_000_000_000_000);
        assert_eq!(report.used_bytes, 2_000_000_000_000);
        assert!(report.months_until_full.is_some());
    }
}
