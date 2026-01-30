"""Failure simulation for storage planner."""

from dataclasses import dataclass, field
from typing import Optional

from storage_planner.models import Topology, Criticality


@dataclass
class DatasetImpact:
    """Impact of a failure on a dataset."""

    dataset_id: str
    dataset_name: str
    criticality: Criticality
    lost_copies: int
    remaining_copies: int
    remaining_volumes: list[str]
    recovery_sources: list[str]  # Volumes from which data can be recovered
    is_recoverable: bool
    notes: list[str] = field(default_factory=list)


@dataclass
class FailureSimResult:
    """Result of simulating a node/volume failure."""

    failed_entity: str
    failed_type: str  # "node" or "volume"
    affected_volumes: list[str]
    affected_datasets: list[DatasetImpact]
    data_loss_risk: bool  # True if any critical data has no recovery path
    summary: str


def simulate_node_failure(topology: Topology, node_id: str) -> FailureSimResult:
    """Simulate the failure of a node.

    Analyzes impact on all datasets and identifies recovery paths.
    """
    node = topology.get_node(node_id)
    if not node:
        return FailureSimResult(
            failed_entity=node_id,
            failed_type="node",
            affected_volumes=[],
            affected_datasets=[],
            data_loss_risk=False,
            summary=f"Node '{node_id}' not found",
        )

    # Get all volumes on this node
    affected_volumes = [v.id for v in node.volumes]

    return _analyze_volume_failure(topology, affected_volumes, node_id, "node")


def simulate_volume_failure(topology: Topology, volume_id: str) -> FailureSimResult:
    """Simulate the failure of a specific volume."""
    vol_info = topology.get_volume(volume_id)
    if not vol_info:
        return FailureSimResult(
            failed_entity=volume_id,
            failed_type="volume",
            affected_volumes=[],
            affected_datasets=[],
            data_loss_risk=False,
            summary=f"Volume '{volume_id}' not found",
        )

    return _analyze_volume_failure(topology, [volume_id], volume_id, "volume")


def _analyze_volume_failure(
    topology: Topology,
    failed_volumes: list[str],
    entity_id: str,
    entity_type: str,
) -> FailureSimResult:
    """Analyze the impact of losing one or more volumes."""
    failed_set = set(failed_volumes)
    dataset_impacts: list[DatasetImpact] = []
    data_loss_risk = False

    # Build volume -> datasets map (both directions)
    volume_datasets: dict[str, set[str]] = {}
    for dataset in topology.datasets:
        for vol_id in dataset.stored_on:
            volume_datasets.setdefault(vol_id, set()).add(dataset.id)

    for node in topology.nodes:
        for volume in node.volumes:
            for ds_id in volume.hosts_datasets:
                volume_datasets.setdefault(volume.id, set()).add(ds_id)

    # Find affected datasets
    affected_ds_ids: set[str] = set()
    for vol_id in failed_volumes:
        affected_ds_ids.update(volume_datasets.get(vol_id, set()))

    all_volume_ids = topology.get_all_volume_ids()

    for ds_id in affected_ds_ids:
        dataset = topology.get_dataset(ds_id)
        if not dataset:
            continue

        # Find all volumes that have this dataset
        hosting_volumes: set[str] = set(dataset.stored_on)
        for node in topology.nodes:
            for volume in node.volumes:
                if dataset.id in volume.hosts_datasets:
                    hosting_volumes.add(volume.id)

        # Count remaining copies
        lost = hosting_volumes & failed_set
        remaining = hosting_volumes - failed_set
        remaining_copies = len(remaining)

        # Identify recovery sources (remaining volumes)
        recovery_sources = list(remaining)

        # Check if recoverable
        is_recoverable = remaining_copies > 0

        notes: list[str] = []
        if not is_recoverable:
            notes.append("NO RECOVERY PATH - data loss if failure occurs")
            if dataset.criticality == Criticality.CRITICAL:
                data_loss_risk = True
        elif remaining_copies < dataset.required_copies:
            notes.append(
                f"Below required copies ({remaining_copies} < {dataset.required_copies})"
            )

        dataset_impacts.append(
            DatasetImpact(
                dataset_id=dataset.id,
                dataset_name=dataset.name,
                criticality=dataset.criticality,
                lost_copies=len(lost),
                remaining_copies=remaining_copies,
                remaining_volumes=list(remaining),
                recovery_sources=recovery_sources,
                is_recoverable=is_recoverable,
                notes=notes,
            )
        )

    # Generate summary
    unrecoverable = [d for d in dataset_impacts if not d.is_recoverable]
    critical_affected = [
        d for d in dataset_impacts if d.criticality == Criticality.CRITICAL
    ]

    if unrecoverable:
        summary = f"CRITICAL: {len(unrecoverable)} dataset(s) would be unrecoverable"
    elif critical_affected:
        summary = f"Warning: {len(critical_affected)} critical dataset(s) affected but recoverable"
    elif dataset_impacts:
        summary = f"{len(dataset_impacts)} dataset(s) affected, all recoverable"
    else:
        summary = "No datasets affected"

    return FailureSimResult(
        failed_entity=entity_id,
        failed_type=entity_type,
        affected_volumes=failed_volumes,
        affected_datasets=dataset_impacts,
        data_loss_risk=data_loss_risk,
        summary=summary,
    )
