"""Capacity analysis for storage planner."""

from dataclasses import dataclass

from storage_planner.analysis.utils import format_size, parse_growth_rate, parse_size
from storage_planner.models import Topology


@dataclass
class CapacityResult:
    """Result of capacity analysis for a volume."""

    volume_id: str
    node_id: str
    capacity: str
    current_used: str
    projected_used: str
    projected_utilization_pct: float
    will_exceed_capacity: bool
    months_until_full: int | None


def analyze_capacity(topology: Topology, projection_months: int = 12) -> list[CapacityResult]:
    """Analyze capacity and project future usage.

    For each volume, calculates current utilization and projects future
    usage based on dataset growth rates.
    """
    results = []

    # Build volume -> datasets map
    volume_datasets: dict[str, list[str]] = {}
    for dataset in topology.datasets:
        for vol_id in dataset.stored_on:
            volume_datasets.setdefault(vol_id, []).append(dataset.id)

    for node in topology.nodes:
        for volume in node.volumes:
            # Add datasets that declare hosts_datasets
            for ds_id in volume.hosts_datasets:
                volume_datasets.setdefault(volume.id, []).append(ds_id)

    for node in topology.nodes:
        for volume in node.volumes:
            capacity_bytes = parse_size(volume.usable_capacity or volume.raw_capacity)
            if capacity_bytes is None:
                continue

            # Calculate current usage from datasets
            current_used_bytes = 0
            monthly_growth_bytes = 0

            ds_ids = set(volume_datasets.get(volume.id, []))
            for ds_id in ds_ids:
                dataset = topology.get_dataset(ds_id)
                if dataset:
                    size = parse_size(dataset.current_size)
                    if size:
                        current_used_bytes += size

                    # Parse growth rate
                    if dataset.growth_rate:
                        growth = parse_growth_rate(dataset.growth_rate)
                        if growth:
                            value, period, kind = growth
                            if kind == "percent":
                                if size:
                                    if period in ("month", "monthly"):
                                        monthly_growth_bytes += int(size * (value / 100))
                                    elif period in ("year", "yearly"):
                                        monthly_growth_bytes += int(size * (value / 100) / 12)
                            else:
                                if period in ("month", "monthly"):
                                    monthly_growth_bytes += int(value)
                                elif period in ("year", "yearly"):
                                    monthly_growth_bytes += int(value / 12)

            # Override with volume.used if specified
            if volume.used:
                used = parse_size(volume.used)
                if used:
                    current_used_bytes = used

            # Project future usage
            projected_bytes = current_used_bytes + (monthly_growth_bytes * projection_months)
            will_exceed = projected_bytes > capacity_bytes

            utilization = (projected_bytes / capacity_bytes) * 100 if capacity_bytes > 0 else 0

            # Calculate months until full
            months_until_full: int | None = None
            if monthly_growth_bytes > 0:
                remaining = capacity_bytes - current_used_bytes
                if remaining > 0:
                    months_until_full = int(remaining / monthly_growth_bytes)

            results.append(
                CapacityResult(
                    volume_id=volume.id,
                    node_id=node.id,
                    capacity=format_size(capacity_bytes),
                    current_used=format_size(current_used_bytes),
                    projected_used=format_size(projected_bytes),
                    projected_utilization_pct=utilization,
                    will_exceed_capacity=will_exceed,
                    months_until_full=months_until_full,
                )
            )

    return results
