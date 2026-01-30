"""Tests for example files."""

import pytest
from pathlib import Path

from storage_planner.loaders import (
    load_topology,
    load_all_catalogs,
    validate_topology_references,
)
from storage_planner.analysis.redundancy import analyze_redundancy
from storage_planner.analysis.rpo_rto import analyze_rpo_rto
from storage_planner.analysis.bandwidth import analyze_bandwidth
from storage_planner.analysis.capacity import analyze_capacity
from storage_planner.analysis.cost import analyze_cost
from storage_planner.analysis.failure_sim import simulate_node_failure


# Path to examples directory (relative to project root)
EXAMPLES_DIR = Path(__file__).parent.parent / "examples"
CATALOG_DIR = Path(__file__).parent.parent / "catalog"


@pytest.fixture
def example_topology():
    """Load the example topology."""
    return load_topology(EXAMPLES_DIR / "topology.yaml")


@pytest.fixture
def example_catalogs():
    """Load the example catalogs."""
    return load_all_catalogs(CATALOG_DIR)


class TestExampleTopology:
    """Test the example topology file."""

    def test_loads_successfully(self, example_topology):
        assert example_topology.name == "Home/Server Backup Topology"

    def test_has_expected_nodes(self, example_topology):
        node_ids = {n.id for n in example_topology.nodes}
        assert "macbook-m4" in node_ids
        assert "mac-mini-m4" in node_ids
        assert "eu-server" in node_ids

    def test_has_expected_datasets(self, example_topology):
        dataset_ids = {d.id for d in example_topology.datasets}
        assert "working-docs" in dataset_ids
        assert "source-code" in dataset_ids
        assert "photos-archive" in dataset_ids

    def test_referential_integrity(self, example_topology):
        errors = validate_topology_references(example_topology)
        assert errors == [], f"Reference errors: {errors}"

    def test_redundancy_analysis_runs(self, example_topology):
        results = analyze_redundancy(example_topology)
        assert len(results) == len(example_topology.datasets)

    def test_rpo_rto_analysis_runs(self, example_topology):
        results = analyze_rpo_rto(example_topology)
        assert len(results) == len(example_topology.datasets)

    def test_bandwidth_analysis_runs(self, example_topology):
        results = analyze_bandwidth(example_topology)
        assert len(results) > 0

    def test_capacity_analysis_runs(self, example_topology):
        results = analyze_capacity(example_topology)
        assert len(results) > 0

    def test_cost_analysis_runs(self, example_topology):
        summary = analyze_cost(example_topology)
        assert summary.total_monthly >= 0

    def test_failure_simulation_runs(self, example_topology):
        result = simulate_node_failure(example_topology, "macbook-m4")
        assert result.failed_entity == "macbook-m4"
        assert len(result.affected_datasets) > 0


class TestExampleCatalogs:
    """Test the example catalog files."""

    def test_hardware_catalog_loads(self, example_catalogs):
        hardware, _, _ = example_catalogs
        assert len(hardware.products) > 0

    def test_hardware_has_ssds(self, example_catalogs):
        hardware, _, _ = example_catalogs
        from storage_planner.models import ProductCategory

        ssds = hardware.get_by_category(ProductCategory.SSD)
        assert len(ssds) > 0

    def test_hardware_has_enclosures(self, example_catalogs):
        hardware, _, _ = example_catalogs
        from storage_planner.models import ProductCategory

        enclosures = hardware.get_by_category(ProductCategory.ENCLOSURE)
        assert len(enclosures) > 0

    def test_software_catalog_loads(self, example_catalogs):
        _, software, _ = example_catalogs
        assert len(software.software) > 0

    def test_software_has_sync_tools(self, example_catalogs):
        _, software, _ = example_catalogs
        sync_tools = software.get_by_type("sync")
        assert len(sync_tools) > 0

    def test_software_has_backup_tools(self, example_catalogs):
        _, software, _ = example_catalogs
        backup_tools = software.get_by_type("backup")
        assert len(backup_tools) > 0

    def test_market_prices_load(self, example_catalogs):
        _, _, prices = example_catalogs
        assert len(prices.prices) > 0

    def test_market_prices_reference_valid_products(self, example_catalogs):
        hardware, _, prices = example_catalogs
        for price in prices.prices:
            product = hardware.get_product(price.product_id)
            assert product is not None, f"Price references unknown product: {price.product_id}"


class TestExampleIntegration:
    """Integration tests using example files."""

    def test_cost_with_catalog(self, example_topology, example_catalogs):
        hardware, _, prices = example_catalogs
        summary = analyze_cost(example_topology, hardware, prices)
        assert summary.total_monthly > 0

    def test_all_sync_regimes_have_valid_datasets(self, example_topology):
        dataset_ids = {d.id for d in example_topology.datasets}
        for regime in example_topology.sync_regimes:
            assert regime.dataset in dataset_ids, f"Regime {regime.id} references unknown dataset"

    def test_all_datasets_have_storage(self, example_topology):
        for dataset in example_topology.datasets:
            all_storage = set(dataset.stored_on)
            # Also check volumes that list this dataset
            for node in example_topology.nodes:
                for vol in node.volumes:
                    if dataset.id in vol.hosts_datasets:
                        all_storage.add(vol.id)
            assert len(all_storage) > 0, f"Dataset {dataset.id} has no storage locations"
