"""Bandwidth analysis for storage planner."""

from dataclasses import dataclass
from typing import Optional

from storage_planner.models import Topology
from storage_planner.analysis.utils import parse_size, parse_bandwidth, format_duration


@dataclass
class BandwidthResult:
    """Result of bandwidth analysis for a sync regime."""

    sync_regime_id: str
    dataset_id: str
    dataset_size: str
    source_volume: str
    target_volumes: list[str]
    link_id: Optional[str]  # Link used for transfer
    effective_bandwidth: Optional[str]  # Available bandwidth
    estimated_sync_time: Optional[str]  # Time for full sync
    is_bottleneck: bool


def analyze_bandwidth(topology: Topology) -> list[BandwidthResult]:
    """Analyze bandwidth for all sync regimes.

    For each sync regime, determines the network path and estimates
    transfer times based on dataset size and link bandwidth.
    """
    results = []

    # Build node -> volumes map
    volume_to_node: dict[str, str] = {}
    for node in topology.nodes:
        for volume in node.volumes:
            volume_to_node[volume.id] = node.id

    # Build link lookup (simple: assumes one link between any two nodes)
    link_map: dict[tuple[str, str], dict] = {}
    for link in topology.links:
        link_map[(link.node_a, link.node_b)] = {
            "id": link.id,
            "up": link.bandwidth_up,
            "down": link.bandwidth_down,
        }
        link_map[(link.node_b, link.node_a)] = {
            "id": link.id,
            "up": link.bandwidth_down,  # Reversed direction
            "down": link.bandwidth_up,
        }

    for regime in topology.sync_regimes:
        dataset = topology.get_dataset(regime.dataset)
        if not dataset:
            continue

        source_node = volume_to_node.get(regime.source_volume)
        dataset_size = dataset.current_size

        # For each target, estimate transfer
        for target_vol in regime.target_volumes:
            target_node = volume_to_node.get(target_vol)

            link_id: Optional[str] = None
            effective_bandwidth: Optional[str] = None
            estimated_time: Optional[str] = None
            is_bottleneck = False

            if source_node and target_node:
                if source_node == target_node:
                    # Same node - internal transfer, no network bottleneck
                    link_id = "(internal)"
                    effective_bandwidth = "(local)"
                    # Estimate based on SSD speeds
                    estimated_time = "fast"
                else:
                    # Find link between nodes
                    link_info = link_map.get((source_node, target_node))
                    if link_info:
                        link_id = link_info["id"]
                        effective_bandwidth = link_info["up"]

                        # Calculate transfer time
                        size_bytes = parse_size(dataset_size)
                        bw_bps = parse_bandwidth(effective_bandwidth) if effective_bandwidth else None

                        if size_bytes and bw_bps:
                            # Convert bandwidth to bytes per second
                            bw_bytes_per_sec = bw_bps / 8
                            transfer_seconds = int(size_bytes / bw_bytes_per_sec)
                            estimated_time = format_duration(transfer_seconds)

                            # Flag as bottleneck if transfer would take > 1 hour
                            if transfer_seconds > 3600:
                                is_bottleneck = True

            results.append(
                BandwidthResult(
                    sync_regime_id=regime.id,
                    dataset_id=regime.dataset,
                    dataset_size=dataset_size,
                    source_volume=regime.source_volume,
                    target_volumes=regime.target_volumes,
                    link_id=link_id,
                    effective_bandwidth=effective_bandwidth,
                    estimated_sync_time=estimated_time,
                    is_bottleneck=is_bottleneck,
                )
            )

    return results
