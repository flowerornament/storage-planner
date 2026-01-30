"""Tests for YAML loaders."""

import pytest
from pathlib import Path
import tempfile
import yaml

from storage_planner.loaders import (
    load_topology,
    load_hardware_catalog,
    load_software_catalog,
    load_market_prices,
    load_all_catalogs,
    validate_topology_references,
    ValidationError,
)
from storage_planner.models import Topology


class TestLoadTopology:
    """Test topology loading."""

    def test_load_valid_topology(self, temp_yaml_dir):
        topo = load_topology(temp_yaml_dir / "topology.yaml")
        assert topo.name == "Full Test Topology"
        assert len(topo.nodes) == 3

    def test_load_missing_file(self):
        with pytest.raises(ValidationError) as exc:
            load_topology(Path("/nonexistent/file.yaml"))
        assert "not found" in str(exc.value)

    def test_load_invalid_yaml(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
            f.write("invalid: yaml: content: [")
            f.flush()
            with pytest.raises(ValidationError) as exc:
                load_topology(Path(f.name))
            assert "Invalid YAML" in str(exc.value)

    def test_load_invalid_schema(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".yaml", delete=False) as f:
            # Missing required 'name' field
            yaml.dump({"nodes": []}, f)
            f.flush()
            with pytest.raises(ValidationError) as exc:
                load_topology(Path(f.name))
            assert len(exc.value.errors) > 0


class TestLoadCatalogs:
    """Test catalog loading."""

    def test_load_hardware_catalog(self, temp_yaml_dir):
        catalog = load_hardware_catalog(temp_yaml_dir / "catalog" / "hardware.yaml")
        assert len(catalog.products) == 3

    def test_load_software_catalog(self, temp_yaml_dir):
        catalog = load_software_catalog(temp_yaml_dir / "catalog" / "software.yaml")
        assert len(catalog.software) == 3

    def test_load_market_prices(self, temp_yaml_dir):
        prices = load_market_prices(temp_yaml_dir / "catalog" / "market-prices.yaml")
        assert len(prices.prices) == 2

    def test_load_all_catalogs(self, temp_yaml_dir):
        hardware, software, prices = load_all_catalogs(temp_yaml_dir / "catalog")
        assert len(hardware.products) == 3
        assert len(software.software) == 3
        assert len(prices.prices) == 2

    def test_load_all_catalogs_missing_dir(self):
        hardware, software, prices = load_all_catalogs(Path("/nonexistent"))
        assert len(hardware.products) == 0
        assert len(software.software) == 0
        assert len(prices.prices) == 0


class TestValidateReferences:
    """Test referential integrity validation."""

    def test_valid_topology(self, full_topology):
        errors = validate_topology_references(full_topology)
        assert errors == []

    def test_invalid_link_node_a(self, full_topology):
        full_topology.links[0].node_a = "nonexistent"
        errors = validate_topology_references(full_topology)
        assert len(errors) == 1
        assert "node_a" in errors[0]

    def test_invalid_link_node_b(self, full_topology):
        full_topology.links[0].node_b = "nonexistent"
        errors = validate_topology_references(full_topology)
        assert len(errors) == 1
        assert "node_b" in errors[0]

    def test_invalid_dataset_stored_on(self, full_topology):
        full_topology.datasets[0].stored_on.append("nonexistent-volume")
        errors = validate_topology_references(full_topology)
        assert len(errors) == 1
        assert "stored_on" in errors[0]

    def test_invalid_dataset_accessible_from(self, full_topology):
        full_topology.datasets[0].accessible_from = ["nonexistent-node"]
        errors = validate_topology_references(full_topology)
        assert len(errors) == 1
        assert "accessible_from" in errors[0]

    def test_invalid_dataset_primary_volume(self, full_topology):
        full_topology.datasets[0].primary_volume = "nonexistent"
        errors = validate_topology_references(full_topology)
        assert len(errors) == 1
        assert "primary_volume" in errors[0]

    def test_invalid_dataset_fallback_volume(self, full_topology):
        full_topology.datasets[0].fallback_volume = "nonexistent"
        errors = validate_topology_references(full_topology)
        assert len(errors) == 1
        assert "fallback_volume" in errors[0]

    def test_invalid_volume_hosts_datasets(self, full_topology):
        full_topology.nodes[0].volumes[0].hosts_datasets.append("nonexistent-dataset")
        errors = validate_topology_references(full_topology)
        assert len(errors) == 1
        assert "hosts_datasets" in errors[0]

    def test_invalid_sync_regime_dataset(self, full_topology):
        full_topology.sync_regimes[0].dataset = "nonexistent"
        errors = validate_topology_references(full_topology)
        assert len(errors) == 1
        assert "dataset" in errors[0].lower()

    def test_invalid_sync_regime_source_volume(self, full_topology):
        full_topology.sync_regimes[0].source_volume = "nonexistent"
        errors = validate_topology_references(full_topology)
        assert len(errors) == 1
        assert "source_volume" in errors[0]

    def test_invalid_sync_regime_target_volume(self, full_topology):
        full_topology.sync_regimes[0].target_volumes.append("nonexistent")
        errors = validate_topology_references(full_topology)
        assert len(errors) == 1
        assert "target_volumes" in errors[0]

    def test_multiple_errors(self, full_topology):
        full_topology.links[0].node_a = "bad1"
        full_topology.links[0].node_b = "bad2"
        full_topology.datasets[0].stored_on.append("bad3")
        errors = validate_topology_references(full_topology)
        assert len(errors) == 3
