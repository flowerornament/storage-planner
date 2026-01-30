"""Tests for analysis algorithms."""

import pytest

from storage_planner.analysis.redundancy import analyze_redundancy
from storage_planner.analysis.rpo_rto import analyze_rpo_rto
from storage_planner.analysis.bandwidth import analyze_bandwidth
from storage_planner.analysis.capacity import analyze_capacity
from storage_planner.analysis.cost import analyze_cost
from storage_planner.analysis.failure_sim import (
    simulate_node_failure,
    simulate_volume_failure,
)
from storage_planner.models import Criticality


class TestRedundancyAnalysis:
    """Test redundancy analysis."""

    def test_analyze_redundancy(self, full_topology):
        results = analyze_redundancy(full_topology)
        assert len(results) == 4  # 4 datasets

        # Find docs dataset result
        docs = next(r for r in results if r.dataset_id == "docs")
        assert docs.criticality == Criticality.CRITICAL
        assert docs.required_copies == 3  # From global constraint
        assert docs.actual_copies == 2  # laptop-ssd, server-raid
        assert docs.meets_requirements is False  # 2 < 3

    def test_redundancy_counts_locations(self, full_topology):
        results = analyze_redundancy(full_topology)

        docs = next(r for r in results if r.dataset_id == "docs")
        assert docs.actual_locations == 2  # home, datacenter
        assert docs.required_locations == 2

    def test_redundancy_replaceable_low_requirements(self, full_topology):
        results = analyze_redundancy(full_topology)

        media = next(r for r in results if r.dataset_id == "media")
        assert media.criticality == Criticality.REPLACEABLE
        assert media.required_copies == 1
        assert media.meets_requirements is True

    def test_redundancy_considers_hosts_datasets(self, full_topology):
        results = analyze_redundancy(full_topology)

        # backups is in server-raid and nas-array via hosts_datasets
        backups = next(r for r in results if r.dataset_id == "backups")
        assert backups.actual_copies == 2


class TestRpoRtoAnalysis:
    """Test RPO/RTO analysis."""

    def test_analyze_rpo_rto(self, full_topology):
        results = analyze_rpo_rto(full_topology)
        assert len(results) == 4

    def test_continuous_sync_meets_rpo(self, full_topology):
        results = analyze_rpo_rto(full_topology)

        docs = next(r for r in results if r.dataset_id == "docs")
        assert docs.max_rpo == "1h"
        assert docs.achieved_rpo == "30s"
        assert docs.rpo_met is True

    def test_scheduled_sync_rpo(self, full_topology):
        results = analyze_rpo_rto(full_topology)

        backups = next(r for r in results if r.dataset_id == "backups")
        assert backups.achieved_rpo == "24h"
        assert backups.rpo_met is True  # 24h <= 24h

    def test_no_sync_regime_unknown_rpo(self, full_topology):
        results = analyze_rpo_rto(full_topology)

        media = next(r for r in results if r.dataset_id == "media")
        assert media.achieved_rpo is None
        assert media.rpo_met is None  # Can't determine


class TestBandwidthAnalysis:
    """Test bandwidth analysis."""

    def test_analyze_bandwidth(self, full_topology):
        results = analyze_bandwidth(full_topology)
        assert len(results) > 0

    def test_identifies_link(self, full_topology):
        results = analyze_bandwidth(full_topology)

        # docs-sync goes from laptop to server via WAN
        docs_sync = next((r for r in results if r.sync_regime_id == "docs-sync"), None)
        assert docs_sync is not None
        assert docs_sync.link_id == "home-wan"
        assert docs_sync.effective_bandwidth == "100Mbps"

    def test_estimates_transfer_time(self, full_topology):
        results = analyze_bandwidth(full_topology)

        docs_sync = next(r for r in results if r.sync_regime_id == "docs-sync")
        # 50GB at 100Mbps = 50*1024^3*8 / 100*10^6 = ~4295 seconds
        assert docs_sync.estimated_sync_time is not None

    def test_flags_bottlenecks(self, full_topology):
        # Make backup dataset larger to trigger bottleneck (>1 hour transfer)
        full_topology.datasets[2].current_size = "2TB"  # 2TB over 500Mbps = ~9 hours
        results = analyze_bandwidth(full_topology)

        backup_sync = next(r for r in results if r.sync_regime_id == "backup-sync")
        assert backup_sync.is_bottleneck is True


class TestCapacityAnalysis:
    """Test capacity analysis."""

    def test_analyze_capacity(self, full_topology):
        results = analyze_capacity(full_topology, projection_months=12)
        assert len(results) == 3  # 3 volumes

    def test_projects_growth(self, full_topology):
        results = analyze_capacity(full_topology, projection_months=12)

        # Find laptop-ssd: has docs (50GB, 1GB/month) and code (10GB, no growth)
        laptop_ssd = next(r for r in results if r.volume_id == "laptop-ssd")
        # Starting: 200GB used (from volume.used override)
        assert "200" in laptop_ssd.current_used

    def test_detects_capacity_issues(self, full_topology):
        # Modify to create capacity issue
        full_topology.nodes[0].volumes[0].used = "450GB"  # Near full
        full_topology.datasets[0].growth_rate = "10GB/month"  # Fast growth

        results = analyze_capacity(full_topology, projection_months=12)
        laptop_ssd = next(r for r in results if r.volume_id == "laptop-ssd")
        # Should project over capacity
        assert laptop_ssd.projected_utilization_pct > 90


class TestCostAnalysis:
    """Test cost analysis."""

    def test_analyze_cost(self, full_topology):
        summary = analyze_cost(full_topology)
        assert summary.total_monthly > 0
        assert len(summary.nodes) == 3

    def test_includes_hosting_cost(self, full_topology):
        summary = analyze_cost(full_topology)

        server = next(n for n in summary.nodes if n.node_id == "server")
        assert server.monthly_hosting == 50.0

    def test_calculates_power_cost(self, full_topology):
        summary = analyze_cost(full_topology, power_cost_per_kwh=0.12)

        server = next(n for n in summary.nodes if n.node_id == "server")
        # 50W * 24 * 30 / 1000 * 0.12 = ~$4.32
        assert server.monthly_power > 0

    def test_five_year_projection(self, full_topology):
        summary = analyze_cost(full_topology)

        # 5 year = 60 months of operational
        expected_min = summary.total_monthly * 60
        assert summary.five_year_projection >= expected_min

    def test_uses_catalog_prices(self, full_topology, hardware_catalog, market_prices):
        # Add product reference to volume
        full_topology.nodes[0].volumes[0].product_id = "ssd-4tb"

        summary = analyze_cost(full_topology, hardware_catalog, market_prices)

        laptop = next(n for n in summary.nodes if n.node_id == "laptop")
        assert laptop.hardware_cost == 300.0  # From catalog


class TestFailureSimulation:
    """Test failure simulation."""

    def test_simulate_node_failure(self, full_topology):
        result = simulate_node_failure(full_topology, "laptop")

        assert result.failed_entity == "laptop"
        assert result.failed_type == "node"
        assert "laptop-ssd" in result.affected_volumes

    def test_node_failure_identifies_affected_datasets(self, full_topology):
        result = simulate_node_failure(full_topology, "laptop")

        affected_ids = {d.dataset_id for d in result.affected_datasets}
        assert "docs" in affected_ids
        assert "code" in affected_ids

    def test_node_failure_recovery_sources(self, full_topology):
        result = simulate_node_failure(full_topology, "laptop")

        docs = next(d for d in result.affected_datasets if d.dataset_id == "docs")
        assert docs.is_recoverable is True
        assert "server-raid" in docs.recovery_sources

    def test_node_failure_counts_remaining(self, full_topology):
        result = simulate_node_failure(full_topology, "laptop")

        docs = next(d for d in result.affected_datasets if d.dataset_id == "docs")
        assert docs.lost_copies == 1
        assert docs.remaining_copies == 1  # server-raid still has it

    def test_simulate_volume_failure(self, full_topology):
        result = simulate_volume_failure(full_topology, "laptop-ssd")

        assert result.failed_type == "volume"
        assert len(result.affected_volumes) == 1

    def test_simulate_nonexistent_node(self, full_topology):
        result = simulate_node_failure(full_topology, "nonexistent")
        assert "not found" in result.summary

    def test_simulate_nonexistent_volume(self, full_topology):
        result = simulate_volume_failure(full_topology, "nonexistent")
        assert "not found" in result.summary

    def test_unrecoverable_data(self, full_topology):
        # Simulate failure of NAS which has media (only copy)
        result = simulate_node_failure(full_topology, "nas")

        media = next(
            (d for d in result.affected_datasets if d.dataset_id == "media"), None
        )
        assert media is not None
        assert media.is_recoverable is False
        assert media.remaining_copies == 0

    def test_data_loss_risk_flag(self, full_topology):
        # media is replaceable, so even unrecoverable shouldn't trigger risk
        result = simulate_node_failure(full_topology, "nas")
        # Only critical unrecoverable data triggers data_loss_risk
        assert result.data_loss_risk is False

    def test_critical_data_loss_risk(self, full_topology):
        # Make media critical and only on nas
        full_topology.datasets[3].criticality = Criticality.CRITICAL

        result = simulate_node_failure(full_topology, "nas")
        assert result.data_loss_risk is True
