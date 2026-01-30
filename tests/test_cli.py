"""Tests for CLI commands."""

import pytest
from typer.testing import CliRunner
from pathlib import Path

from storage_planner.cli.main import app

runner = CliRunner()


class TestValidateCommand:
    """Test validate command."""

    def test_validate_valid_topology(self, temp_yaml_dir):
        result = runner.invoke(app, ["validate", str(temp_yaml_dir / "topology.yaml")])
        assert result.exit_code == 0
        assert "Valid" in result.stdout

    def test_validate_verbose(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["validate", str(temp_yaml_dir / "topology.yaml"), "-v"]
        )
        assert result.exit_code == 0
        assert "Nodes:" in result.stdout
        assert "Datasets:" in result.stdout

    def test_validate_missing_file(self):
        result = runner.invoke(app, ["validate", "/nonexistent/file.yaml"])
        assert result.exit_code != 0

    def test_validate_invalid_references(self, temp_yaml_dir):
        # Modify topology to have invalid reference
        import yaml

        topo_path = temp_yaml_dir / "topology.yaml"
        with open(topo_path) as f:
            data = yaml.safe_load(f)
        data["datasets"][0]["stored_on"].append("nonexistent-volume")
        with open(topo_path, "w") as f:
            yaml.dump(data, f)

        result = runner.invoke(app, ["validate", str(topo_path)])
        assert result.exit_code == 1
        assert "referential integrity" in result.stdout.lower() or "unknown" in result.stdout.lower()


class TestAnalyzeCommand:
    """Test analyze commands."""

    def test_analyze_all(self, temp_yaml_dir):
        result = runner.invoke(app, ["analyze", "all", str(temp_yaml_dir / "topology.yaml")])
        assert result.exit_code == 0
        assert "Redundancy" in result.stdout
        assert "RPO" in result.stdout
        assert "Bandwidth" in result.stdout
        assert "Capacity" in result.stdout

    def test_analyze_redundancy(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["analyze", "redundancy", str(temp_yaml_dir / "topology.yaml")]
        )
        assert result.exit_code == 0
        assert "Redundancy" in result.stdout

    def test_analyze_bandwidth(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["analyze", "bandwidth", str(temp_yaml_dir / "topology.yaml")]
        )
        assert result.exit_code == 0
        assert "Bandwidth" in result.stdout

    def test_analyze_rpo_rto(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["analyze", "rpo-rto", str(temp_yaml_dir / "topology.yaml")]
        )
        assert result.exit_code == 0
        assert "RPO" in result.stdout

    def test_analyze_capacity(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["analyze", "capacity", str(temp_yaml_dir / "topology.yaml")]
        )
        assert result.exit_code == 0
        assert "Capacity" in result.stdout

    def test_analyze_capacity_custom_months(self, temp_yaml_dir):
        result = runner.invoke(
            app,
            ["analyze", "capacity", str(temp_yaml_dir / "topology.yaml"), "-m", "24"],
        )
        assert result.exit_code == 0
        assert "24 months" in result.stdout

    def test_analyze_quick(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["analyze", "quick", str(temp_yaml_dir / "topology.yaml")]
        )
        assert result.exit_code == 0
        assert "Quick Analysis" in result.stdout
        assert "Insight Summary" in result.stdout

    def test_analyze_json(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["analyze", "redundancy", str(temp_yaml_dir / "topology.yaml"), "--json"]
        )
        assert result.exit_code == 0
        assert "\"summary\"" in result.stdout


class TestCostCommand:
    """Test cost command."""

    def test_cost(self, temp_yaml_dir):
        result = runner.invoke(app, ["cost", str(temp_yaml_dir / "topology.yaml")])
        assert result.exit_code == 0
        assert "Cost" in result.stdout
        assert "Monthly" in result.stdout or "monthly" in result.stdout

    def test_cost_with_catalog(self, temp_yaml_dir):
        result = runner.invoke(
            app,
            [
                "cost",
                str(temp_yaml_dir / "topology.yaml"),
                "-c",
                str(temp_yaml_dir / "catalog"),
            ],
        )
        assert result.exit_code == 0

    def test_cost_custom_power_rate(self, temp_yaml_dir):
        result = runner.invoke(
            app,
            [
                "cost",
                str(temp_yaml_dir / "topology.yaml"),
                "--power-cost",
                "0.20",
            ],
        )
        assert result.exit_code == 0


class TestSimulateCommand:
    """Test simulate command."""

    def test_simulate_node(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["simulate", "laptop", str(temp_yaml_dir / "topology.yaml")]
        )
        assert result.exit_code == 0
        assert "Failure Simulation" in result.stdout

    def test_simulate_volume(self, temp_yaml_dir):
        result = runner.invoke(
            app,
            [
                "simulate",
                "laptop-ssd",
                str(temp_yaml_dir / "topology.yaml"),
                "-t",
                "volume",
            ],
        )
        assert result.exit_code == 0

    def test_simulate_auto_detect(self, temp_yaml_dir):
        # Should auto-detect "laptop" as a node
        result = runner.invoke(
            app, ["simulate", "laptop", str(temp_yaml_dir / "topology.yaml")]
        )
        assert result.exit_code == 0
        assert "node" in result.stdout.lower()

    def test_simulate_nonexistent(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["simulate", "nonexistent", str(temp_yaml_dir / "topology.yaml")]
        )
        assert result.exit_code == 1

    def test_simulate_diff(self, temp_yaml_dir):
        topo = str(temp_yaml_dir / "topology.yaml")
        result = runner.invoke(app, ["simulate", "diff", "laptop", topo, topo])
        assert result.exit_code == 0
        assert "Simulation Diff" in result.stdout


class TestCatalogCommand:
    """Test catalog commands."""

    def test_catalog_list(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["catalog", "list", "-c", str(temp_yaml_dir / "catalog")]
        )
        assert result.exit_code == 0
        assert "ssd-4tb" in result.stdout or "Test SSD" in result.stdout

    def test_catalog_list_by_category(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["catalog", "list", "ssd", "-c", str(temp_yaml_dir / "catalog")]
        )
        assert result.exit_code == 0
        # Should only show SSDs
        assert "enclosure" not in result.stdout.lower() or "ssd" in result.stdout.lower()

    def test_catalog_show(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["catalog", "show", "ssd-4tb", "-c", str(temp_yaml_dir / "catalog")]
        )
        assert result.exit_code == 0
        assert "Test SSD 4TB" in result.stdout
        assert "300" in result.stdout  # retail price

    def test_catalog_show_with_market_price(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["catalog", "show", "ssd-4tb", "-c", str(temp_yaml_dir / "catalog")]
        )
        assert result.exit_code == 0
        assert "220" in result.stdout  # mid market price

    def test_catalog_show_nonexistent(self, temp_yaml_dir):
        result = runner.invoke(
            app,
            ["catalog", "show", "nonexistent", "-c", str(temp_yaml_dir / "catalog")],
        )
        assert result.exit_code == 1

    def test_catalog_search(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["catalog", "search", "4TB", "-c", str(temp_yaml_dir / "catalog")]
        )
        assert result.exit_code == 0
        assert "ssd-4tb" in result.stdout or "4TB" in result.stdout

    def test_catalog_search_no_results(self, temp_yaml_dir):
        result = runner.invoke(
            app,
            ["catalog", "search", "nonexistent", "-c", str(temp_yaml_dir / "catalog")],
        )
        assert result.exit_code == 0
        assert "No products" in result.stdout

    def test_catalog_compare(self, temp_yaml_dir):
        result = runner.invoke(
            app,
            [
                "catalog",
                "compare",
                "ssd-4tb",
                "ssd-8tb",
                "-c",
                str(temp_yaml_dir / "catalog"),
            ],
        )
        assert result.exit_code == 0
        assert "Comparison" in result.stdout
        assert "4TB" in result.stdout
        assert "8TB" in result.stdout

    def test_catalog_software(self, temp_yaml_dir):
        result = runner.invoke(
            app, ["catalog", "software", "-c", str(temp_yaml_dir / "catalog")]
        )
        assert result.exit_code == 0
        assert "Resilio" in result.stdout or "resilio" in result.stdout


class TestSuggestCommand:
    """Test suggest commands."""

    def test_suggest_hardware(self, temp_yaml_dir):
        result = runner.invoke(
            app,
            [
                "suggest",
                "hardware",
                str(temp_yaml_dir / "topology.yaml"),
                "-c",
                str(temp_yaml_dir / "catalog"),
            ],
        )
        assert result.exit_code == 0
        assert "Hardware" in result.stdout or "Redundancy" in result.stdout

    def test_suggest_software(self, temp_yaml_dir):
        result = runner.invoke(
            app,
            [
                "suggest",
                "software",
                str(temp_yaml_dir / "topology.yaml"),
                "-c",
                str(temp_yaml_dir / "catalog"),
            ],
        )
        assert result.exit_code == 0
        assert "Documents" in result.stdout or "Source Code" in result.stdout

    def test_suggest_optimize(self, temp_yaml_dir):
        result = runner.invoke(
            app,
            ["suggest", "optimize", str(temp_yaml_dir / "topology.yaml")],
        )
        assert result.exit_code == 0
        assert "Optimization" in result.stdout or "Always-on" in result.stdout

    def test_suggest_optimize_minimize_devices(self, temp_yaml_dir):
        result = runner.invoke(
            app,
            [
                "suggest",
                "optimize",
                str(temp_yaml_dir / "topology.yaml"),
                "--minimize-devices",
            ],
        )
        assert result.exit_code == 0
        assert "Consolidation" in result.stdout or "devices" in result.stdout.lower()


class TestHelpAndVersion:
    """Test help output."""

    def test_main_help(self):
        result = runner.invoke(app, ["--help"])
        assert result.exit_code == 0
        assert "validate" in result.stdout
        assert "analyze" in result.stdout
        assert "cost" in result.stdout
        assert "simulate" in result.stdout
        assert "catalog" in result.stdout
        assert "suggest" in result.stdout

    def test_analyze_help(self):
        result = runner.invoke(app, ["analyze", "--help"])
        assert result.exit_code == 0
        assert "redundancy" in result.stdout
        assert "bandwidth" in result.stdout

    def test_catalog_help(self):
        result = runner.invoke(app, ["catalog", "--help"])
        assert result.exit_code == 0
        assert "list" in result.stdout
        assert "show" in result.stdout
        assert "compare" in result.stdout
