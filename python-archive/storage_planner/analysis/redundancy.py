"""Redundancy analysis for storage planner."""

from dataclasses import dataclass

from storage_planner.models import Criticality, Topology


@dataclass
class RedundancyResult:
    """Result of redundancy analysis for a dataset."""

    dataset_id: str
    dataset_name: str
    criticality: Criticality
    required_copies: int
    required_locations: int
    actual_copies: int
    actual_locations: int
    volumes: list[str]  # Volume IDs where data is stored
    locations: set[str]  # Unique locations
    meets_requirements: bool


class RedundancyConfigError(Exception):
    """Raised when redundancy analysis lacks required configuration."""

    pass


def analyze_redundancy(topology: Topology) -> list[RedundancyResult]:
    """Analyze redundancy for all datasets in a topology.

    For each dataset, counts:
    - How many volumes store the data (copies)
    - How many unique locations those volumes are in

    Compares against dataset requirements and global constraints.

    Raises:
        RedundancyConfigError: If required constraints are not explicitly set.
    """
    results = []

    # Build a map of volume_id -> location
    volume_locations: dict[str, str] = {}
    for node in topology.nodes:
        location = node.location or node.id
        for volume in node.volumes:
            volume_locations[volume.id] = location

    for dataset in topology.datasets:
        # Determine required copies based on criticality and constraints
        if dataset.criticality == Criticality.CRITICAL:
            # Require explicit constraints for critical data
            if topology.constraints.min_critical_data_copies is None:
                raise RedundancyConfigError(
                    f"Dataset '{dataset.id}' is critical but constraints.min_critical_data_copies "
                    "is not set. Add this to your topology's constraints section."
                )
            if topology.constraints.min_locations_for_critical is None:
                raise RedundancyConfigError(
                    f"Dataset '{dataset.id}' is critical but constraints.min_locations_for_critical "
                    "is not set. Add this to your topology's constraints section."
                )
            min_copies = max(
                dataset.required_copies,
                topology.constraints.min_critical_data_copies,
            )
            min_locations = max(
                dataset.required_locations,
                topology.constraints.min_locations_for_critical,
            )
        elif dataset.criticality == Criticality.IMPORTANT:
            if topology.constraints.min_important_data_copies is None:
                raise RedundancyConfigError(
                    f"Dataset '{dataset.id}' is important but constraints.min_important_data_copies "
                    "is not set. Add this to your topology's constraints section."
                )
            min_copies = max(
                dataset.required_copies,
                topology.constraints.min_important_data_copies,
            )
            min_locations = dataset.required_locations
        else:
            min_copies = dataset.required_copies
            min_locations = dataset.required_locations

        # Find all volumes that host this dataset
        # Check both dataset.stored_on and volume.hosts_datasets
        hosting_volumes = set(dataset.stored_on)

        for node in topology.nodes:
            for volume in node.volumes:
                if dataset.id in volume.hosts_datasets:
                    hosting_volumes.add(volume.id)

        # Get unique locations
        locations = {volume_locations.get(v, "unknown") for v in hosting_volumes}

        actual_copies = len(hosting_volumes)
        actual_locations = len(locations)

        meets_requirements = (
            actual_copies >= min_copies and actual_locations >= min_locations
        )

        results.append(
            RedundancyResult(
                dataset_id=dataset.id,
                dataset_name=dataset.name,
                criticality=dataset.criticality,
                required_copies=min_copies,
                required_locations=min_locations,
                actual_copies=actual_copies,
                actual_locations=actual_locations,
                volumes=list(hosting_volumes),
                locations=locations,
                meets_requirements=meets_requirements,
            )
        )

    return results
