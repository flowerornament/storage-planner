"""YAML loading and validation for storage planner."""

from pathlib import Path

import yaml
from pydantic import ValidationError as PydanticValidationError

from storage_planner.models import (
    HardwareCatalog,
    MarketPrices,
    SoftwareCatalog,
    Topology,
)


class ValidationError(Exception):
    """Raised when validation fails."""

    def __init__(self, message: str, errors: list[str] | None = None):
        super().__init__(message)
        self.message = message
        self.errors = errors or []


def load_yaml(path: Path) -> dict:
    """Load a YAML file and return its contents."""
    if not path.exists():
        raise ValidationError(f"File not found: {path}")
    with open(path) as f:
        try:
            return yaml.safe_load(f) or {}
        except yaml.YAMLError as e:
            raise ValidationError(f"Invalid YAML in {path}: {e}") from e


def load_topology(path: Path) -> Topology:
    """Load and validate a topology file."""
    data = load_yaml(path)
    try:
        topology = Topology(**data)
    except PydanticValidationError as e:
        errors = [f"{'.'.join(str(x) for x in err['loc'])}: {err['msg']}" for err in e.errors()]
        raise ValidationError(f"Invalid topology in {path}", errors) from e
    return topology


def load_hardware_catalog(path: Path) -> HardwareCatalog:
    """Load and validate a hardware catalog file."""
    data = load_yaml(path)
    try:
        return HardwareCatalog(**data)
    except PydanticValidationError as e:
        errors = [f"{'.'.join(str(x) for x in err['loc'])}: {err['msg']}" for err in e.errors()]
        raise ValidationError(f"Invalid hardware catalog in {path}", errors) from e


def load_software_catalog(path: Path) -> SoftwareCatalog:
    """Load and validate a software catalog file."""
    data = load_yaml(path)
    try:
        return SoftwareCatalog(**data)
    except PydanticValidationError as e:
        errors = [f"{'.'.join(str(x) for x in err['loc'])}: {err['msg']}" for err in e.errors()]
        raise ValidationError(f"Invalid software catalog in {path}", errors) from e


def load_market_prices(path: Path) -> MarketPrices:
    """Load and validate a market prices file."""
    data = load_yaml(path)
    try:
        return MarketPrices(**data)
    except PydanticValidationError as e:
        errors = [f"{'.'.join(str(x) for x in err['loc'])}: {err['msg']}" for err in e.errors()]
        raise ValidationError(f"Invalid market prices in {path}", errors) from e


def load_all_catalogs(catalog_dir: Path) -> tuple[HardwareCatalog, SoftwareCatalog, MarketPrices]:
    """Load all catalog files from a directory."""
    hardware = HardwareCatalog()
    software = SoftwareCatalog()
    prices = MarketPrices()

    hardware_path = catalog_dir / "hardware.yaml"
    if hardware_path.exists():
        hardware = load_hardware_catalog(hardware_path)

    software_path = catalog_dir / "software.yaml"
    if software_path.exists():
        software = load_software_catalog(software_path)

    prices_path = catalog_dir / "market-prices.yaml"
    if prices_path.exists():
        prices = load_market_prices(prices_path)

    return hardware, software, prices


def validate_topology_references(topology: Topology) -> list[str]:
    """Validate referential integrity in a topology.

    Returns a list of error messages (empty if valid).
    """
    errors = []
    all_volume_ids = topology.get_all_volume_ids()
    all_node_ids = topology.get_all_node_ids()
    all_dataset_ids = {d.id for d in topology.datasets}

    def find_duplicates(values: list[str]) -> set[str]:
        seen: set[str] = set()
        dupes: set[str] = set()
        for value in values:
            if value in seen:
                dupes.add(value)
            else:
                seen.add(value)
        return dupes

    # Check for duplicate IDs
    node_ids = [n.id for n in topology.nodes]
    volume_ids = [v.id for n in topology.nodes for v in n.volumes]
    dataset_ids = [d.id for d in topology.datasets]
    link_ids = [link.id for link in topology.links]
    sync_ids = [s.id for s in topology.sync_regimes]

    for dup in sorted(find_duplicates(node_ids)):
        errors.append(f"Duplicate node ID: {dup}")
    for dup in sorted(find_duplicates(volume_ids)):
        errors.append(f"Duplicate volume ID: {dup}")
    for dup in sorted(find_duplicates(dataset_ids)):
        errors.append(f"Duplicate dataset ID: {dup}")
    for dup in sorted(find_duplicates(link_ids)):
        errors.append(f"Duplicate link ID: {dup}")
    for dup in sorted(find_duplicates(sync_ids)):
        errors.append(f"Duplicate sync_regime ID: {dup}")

    # Check links reference valid nodes
    for link in topology.links:
        if link.node_a not in all_node_ids:
            errors.append(f"Link '{link.id}' references unknown node_a: {link.node_a}")
        if link.node_b not in all_node_ids:
            errors.append(f"Link '{link.id}' references unknown node_b: {link.node_b}")

    # Check datasets reference valid volumes and nodes
    for dataset in topology.datasets:
        for vol_id in dataset.stored_on:
            if vol_id not in all_volume_ids:
                errors.append(f"Dataset '{dataset.id}' stored_on references unknown volume: {vol_id}")
        for node_id in dataset.accessible_from:
            if node_id not in all_node_ids:
                errors.append(f"Dataset '{dataset.id}' accessible_from references unknown node: {node_id}")
        if dataset.primary_volume and dataset.primary_volume not in all_volume_ids:
            errors.append(f"Dataset '{dataset.id}' primary_volume references unknown volume: {dataset.primary_volume}")
        if dataset.fallback_volume and dataset.fallback_volume not in all_volume_ids:
            errors.append(f"Dataset '{dataset.id}' fallback_volume references unknown volume: {dataset.fallback_volume}")

    # Check volumes reference valid datasets
    for node in topology.nodes:
        for volume in node.volumes:
            for ds_id in volume.hosts_datasets:
                if ds_id not in all_dataset_ids:
                    errors.append(f"Volume '{volume.id}' hosts_datasets references unknown dataset: {ds_id}")

    # Check sync regimes reference valid datasets and volumes
    for regime in topology.sync_regimes:
        if regime.dataset not in all_dataset_ids:
            errors.append(f"SyncRegime '{regime.id}' references unknown dataset: {regime.dataset}")
        if regime.source_volume not in all_volume_ids:
            errors.append(f"SyncRegime '{regime.id}' source_volume references unknown volume: {regime.source_volume}")
        for vol_id in regime.target_volumes:
            if vol_id not in all_volume_ids:
                errors.append(f"SyncRegime '{regime.id}' target_volumes references unknown volume: {vol_id}")

    return errors
