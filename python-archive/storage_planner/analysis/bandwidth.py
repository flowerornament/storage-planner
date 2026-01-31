"""Bandwidth analysis for storage planner."""

import heapq
from dataclasses import dataclass

import networkx as nx

from storage_planner.analysis.utils import (
    format_bandwidth,
    format_duration,
    parse_bandwidth,
    parse_size,
)
from storage_planner.models import Topology


@dataclass
class BandwidthResult:
    """Result of bandwidth analysis for a sync regime."""

    sync_regime_id: str
    dataset_id: str
    dataset_size: str
    source_volume: str
    target_volume: str
    link_id: str | None  # Link used for transfer
    effective_bandwidth: str | None  # Available bandwidth
    estimated_sync_time: str | None  # Time for full sync
    estimated_sync_time_seconds: int | None  # Raw seconds, if available
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

    graph = _build_bandwidth_graph(topology)

    for regime in topology.sync_regimes:
        dataset = topology.get_dataset(regime.dataset)
        if not dataset:
            continue

        source_node = volume_to_node.get(regime.source_volume)
        dataset_size = dataset.current_size

        # For each target, estimate transfer
        for target_vol in regime.target_volumes:
            target_node = volume_to_node.get(target_vol)

            link_id: str | None = None
            effective_bandwidth: str | None = None
            estimated_time: str | None = None
            estimated_time_seconds: int | None = None
            is_bottleneck = False

            if source_node and target_node:
                if source_node == target_node:
                    # Same node - internal transfer, no network bottleneck
                    link_id = "(internal)"
                    effective_bandwidth = "(local)"
                    # Estimate based on SSD speeds
                    estimated_time = "fast"
                else:
                    # Find widest path between nodes
                    path_info = _find_widest_path(graph, source_node, target_node)
                    if path_info:
                        path_links = path_info["links"]
                        link_id = " -> ".join(path_links)

                        # Prefer original bandwidth string for direct links
                        if len(path_links) == 1:
                            link_info = link_map.get((source_node, target_node))
                            effective_bandwidth = link_info["up"] if link_info else None
                        if not effective_bandwidth:
                            effective_bandwidth = format_bandwidth(path_info["capacity"])

                        # Calculate transfer time
                        size_bytes = parse_size(dataset_size)
                        bw_bps = path_info["capacity"]

                        if size_bytes and bw_bps:
                            # Convert bandwidth to bytes per second
                            bw_bytes_per_sec = bw_bps / 8
                            transfer_seconds = int(size_bytes / bw_bytes_per_sec)
                            estimated_time_seconds = transfer_seconds
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
                    target_volume=target_vol,
                    link_id=link_id,
                    effective_bandwidth=effective_bandwidth,
                    estimated_sync_time=estimated_time,
                    estimated_sync_time_seconds=estimated_time_seconds,
                    is_bottleneck=is_bottleneck,
                )
            )

    return results


def _build_bandwidth_graph(topology: Topology) -> nx.DiGraph:
    """Build a directed graph with bandwidth capacities for each link direction."""
    graph = nx.DiGraph()
    for link in topology.links:
        if link.bandwidth_up:
            capacity = parse_bandwidth(link.bandwidth_up)
            if capacity:
                graph.add_edge(
                    link.node_a,
                    link.node_b,
                    capacity=capacity,
                    link_id=link.id,
                )
        if link.bandwidth_down:
            capacity = parse_bandwidth(link.bandwidth_down)
            if capacity:
                graph.add_edge(
                    link.node_b,
                    link.node_a,
                    capacity=capacity,
                    link_id=link.id,
                )
    return graph


def _find_widest_path(
    graph: nx.DiGraph, source: str, target: str
) -> dict[str, list[str] | int] | None:
    """Find path maximizing the minimum bandwidth (widest path)."""
    if source not in graph or target not in graph:
        return None

    best_capacity: dict[str, int] = {source: 10**18}
    prev_node: dict[str, str] = {}
    prev_link: dict[str, str] = {}
    heap: list[tuple[int, str]] = [(-best_capacity[source], source)]

    while heap:
        neg_cap, node = heapq.heappop(heap)
        capacity = -neg_cap
        if node == target:
            break
        if capacity < best_capacity.get(node, 0):
            continue
        for neighbor, attrs in graph[node].items():
            edge_cap = attrs.get("capacity")
            if edge_cap is None:
                continue
            new_cap = min(capacity, edge_cap)
            if new_cap > best_capacity.get(neighbor, 0):
                best_capacity[neighbor] = new_cap
                prev_node[neighbor] = node
                prev_link[neighbor] = attrs.get("link_id")
                heapq.heappush(heap, (-new_cap, neighbor))

    if target not in best_capacity:
        return None

    # Reconstruct path
    nodes: list[str] = [target]
    links: list[str] = []
    while nodes[-1] != source:
        node = nodes[-1]
        prev = prev_node.get(node)
        if prev is None:
            return None
        links.append(prev_link.get(node, ""))
        nodes.append(prev)

    nodes.reverse()
    links.reverse()

    return {"capacity": best_capacity[target], "nodes": nodes, "links": links}
