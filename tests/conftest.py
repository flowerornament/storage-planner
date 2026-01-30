"""Shared test fixtures."""

import tempfile
from pathlib import Path

import pytest
import yaml

from storage_planner.models import (
    ChangeRate,
    Constraints,
    Criticality,
    Dataset,
    HardwareCatalog,
    Link,
    LinkType,
    MarketPrice,
    MarketPrices,
    Node,
    NodeType,
    PowerProfile,
    Product,
    ProductCategory,
    Software,
    SoftwareBestFor,
    SoftwareCatalog,
    SyncDirection,
    SyncMethod,
    SyncRegime,
    Topology,
    Volume,
    VolumeType,
)


@pytest.fixture
def minimal_topology() -> Topology:
    """Minimal valid topology with one node and volume."""
    return Topology(
        name="Test Topology",
        nodes=[
            Node(
                id="node1",
                name="Test Node",
                type=NodeType.DESKTOP,
                volumes=[
                    Volume(
                        id="vol1",
                        type=VolumeType.INTERNAL_SSD,
                        raw_capacity="1TB",
                    )
                ],
            )
        ],
    )


@pytest.fixture
def full_topology() -> Topology:
    """Complete topology with multiple nodes, links, datasets, sync regimes."""
    return Topology(
        name="Full Test Topology",
        version="1.0",
        constraints=Constraints(
            max_monthly_cost=100.0,
            power_cost_per_kwh=0.12,  # Required for power cost calculations
            min_critical_data_copies=3,
            min_important_data_copies=2,
            min_locations_for_critical=2,
            max_noise_db_home=35,
        ),
        nodes=[
            Node(
                id="laptop",
                name="Laptop",
                type=NodeType.LAPTOP,
                location="home",
                power_profile=PowerProfile.MOBILE,
                volumes=[
                    Volume(
                        id="laptop-ssd",
                        type=VolumeType.INTERNAL_SSD,
                        raw_capacity="512GB",
                        usable_capacity="480GB",
                        used="200GB",
                        hosts_datasets=["docs", "code"],
                    ),
                ],
            ),
            Node(
                id="server",
                name="Server",
                type=NodeType.SERVER,
                location="datacenter",
                power_profile=PowerProfile.ALWAYS_ON,
                power_watts_idle=50,
                monthly_cost=50.0,
                volumes=[
                    Volume(
                        id="server-raid",
                        type=VolumeType.RAID_ARRAY,
                        raw_capacity="10TB",
                        usable_capacity="8TB",
                        raid_level="raidz1",
                        hosts_datasets=["docs", "code", "backups"],
                    ),
                ],
            ),
            Node(
                id="nas",
                name="NAS",
                type=NodeType.NAS,
                location="home",
                power_profile=PowerProfile.ALWAYS_ON,
                power_watts_idle=15,
                noise_db=40,
                volumes=[
                    Volume(
                        id="nas-array",
                        type=VolumeType.RAID_ARRAY,
                        raw_capacity="8TB",
                        usable_capacity="6TB",
                        hosts_datasets=["backups", "media"],
                    ),
                ],
            ),
        ],
        links=[
            Link(
                id="home-lan",
                node_a="laptop",
                node_b="nas",
                type=LinkType.LAN,
                bandwidth_up="1Gbps",
                bandwidth_down="1Gbps",
                latency_ms=1,
            ),
            Link(
                id="home-wan",
                node_a="laptop",
                node_b="server",
                type=LinkType.WAN,
                bandwidth_up="100Mbps",
                bandwidth_down="500Mbps",
                latency_ms=30,
            ),
            Link(
                id="nas-wan",
                node_a="nas",
                node_b="server",
                type=LinkType.WAN,
                bandwidth_up="100Mbps",
                bandwidth_down="500Mbps",
                latency_ms=30,
            ),
        ],
        datasets=[
            Dataset(
                id="docs",
                name="Documents",
                current_size="50GB",
                growth_rate="1GB/month",
                criticality=Criticality.CRITICAL,
                change_rate=ChangeRate.HIGH,
                required_copies=3,
                required_locations=2,
                max_rpo="1h",
                max_rto="4h",
                stored_on=["laptop-ssd", "server-raid"],
            ),
            Dataset(
                id="code",
                name="Source Code",
                current_size="10GB",
                criticality=Criticality.CRITICAL,
                change_rate=ChangeRate.HIGH,
                required_copies=3,
                required_locations=2,
                max_rpo="1h",
                stored_on=["laptop-ssd", "server-raid"],
            ),
            Dataset(
                id="backups",
                name="Backups",
                current_size="200GB",
                growth_rate="10GB/month",
                criticality=Criticality.IMPORTANT,
                change_rate=ChangeRate.MEDIUM,
                required_copies=2,
                required_locations=2,
                max_rpo="24h",
                stored_on=["server-raid", "nas-array"],
            ),
            Dataset(
                id="media",
                name="Media",
                current_size="500GB",
                criticality=Criticality.REPLACEABLE,
                change_rate=ChangeRate.LOW,
                required_copies=1,
                stored_on=["nas-array"],
            ),
        ],
        sync_regimes=[
            SyncRegime(
                id="docs-sync",
                dataset="docs",
                source_volume="laptop-ssd",
                target_volumes=["server-raid"],
                method=SyncMethod.RESILIO_SYNC,
                direction=SyncDirection.BIDIRECTIONAL,
                continuous=True,
                achieved_rpo="30s",
            ),
            SyncRegime(
                id="code-sync",
                dataset="code",
                source_volume="laptop-ssd",
                target_volumes=["server-raid"],
                method=SyncMethod.RESILIO_SYNC,
                continuous=True,
                achieved_rpo="30s",
            ),
            SyncRegime(
                id="backup-sync",
                dataset="backups",
                source_volume="server-raid",
                target_volumes=["nas-array"],
                method=SyncMethod.RSYNC,
                schedule="0 2 * * *",
                achieved_rpo="24h",
            ),
        ],
    )


@pytest.fixture
def hardware_catalog() -> HardwareCatalog:
    """Sample hardware catalog."""
    return HardwareCatalog(
        products=[
            Product(
                id="ssd-4tb",
                name="Test SSD 4TB",
                brand="TestBrand",
                category=ProductCategory.SSD,
                specs={
                    "capacity": "4TB",
                    "interface": "SATA",
                    "form_factor": "2.5in",
                    "read_speed": "560MB/s",
                    "write_speed": "530MB/s",
                },
                retail_price=300.0,
                noise_db=0,
            ),
            Product(
                id="ssd-8tb",
                name="Test SSD 8TB",
                brand="TestBrand",
                category=ProductCategory.SSD,
                specs={
                    "capacity": "8TB",
                    "interface": "SATA",
                },
                retail_price=700.0,
            ),
            Product(
                id="enclosure-2bay",
                name="Test Enclosure 2-Bay",
                brand="TestBrand",
                category=ProductCategory.ENCLOSURE,
                specs={
                    "bays": 2,
                    "interface": "USB-C",
                    "m4_mini_compatible": True,
                },
                retail_price=50.0,
            ),
        ]
    )


@pytest.fixture
def software_catalog() -> SoftwareCatalog:
    """Sample software catalog."""
    return SoftwareCatalog(
        software=[
            Software(
                id="resilio",
                name="Resilio Sync",
                type="sync",
                strengths=["continuous", "bidirectional", "peer-to-peer"],
                weaknesses=["no-versioning"],
                best_for=SoftwareBestFor(
                    change_rate=[ChangeRate.HIGH, ChangeRate.REALTIME],
                    direction=SyncDirection.BIDIRECTIONAL,
                ),
                platforms=["macos", "linux", "windows"],
            ),
            Software(
                id="borg",
                name="Borg Backup",
                type="backup",
                strengths=["deduplication", "encryption", "versioning"],
                weaknesses=["cli-only"],
                best_for=SoftwareBestFor(
                    change_rate=[ChangeRate.LOW, ChangeRate.MEDIUM],
                    criticality=["critical"],
                ),
                platforms=["macos", "linux"],
            ),
            Software(
                id="rsync",
                name="rsync",
                type="sync",
                strengths=["efficient-delta", "reliable"],
                weaknesses=["one-direction"],
                best_for=SoftwareBestFor(
                    change_rate=[ChangeRate.STATIC, ChangeRate.LOW],
                    direction=SyncDirection.SOURCE_TO_TARGET,
                ),
                platforms=["macos", "linux"],
            ),
        ]
    )


@pytest.fixture
def market_prices() -> MarketPrices:
    """Sample market prices."""
    return MarketPrices(
        prices=[
            MarketPrice(
                product_id="ssd-4tb",
                source="ebay",
                price_low=180,
                price_mid=220,
                price_high=260,
                last_updated="2025-01-15",
                sample_size=10,
            ),
            MarketPrice(
                product_id="ssd-8tb",
                source="ebay",
                price_low=500,
                price_mid=600,
                price_high=700,
                last_updated="2025-01-10",
                sample_size=5,
            ),
        ]
    )


@pytest.fixture
def temp_yaml_dir(full_topology, hardware_catalog, software_catalog, market_prices):
    """Create temporary directory with YAML files."""
    with tempfile.TemporaryDirectory() as tmpdir:
        tmppath = Path(tmpdir)

        # Write topology
        topo_path = tmppath / "topology.yaml"
        with open(topo_path, "w") as f:
            yaml.dump(full_topology.model_dump(mode="json"), f)

        # Create catalog directory
        catalog_dir = tmppath / "catalog"
        catalog_dir.mkdir()

        # Write hardware catalog
        with open(catalog_dir / "hardware.yaml", "w") as f:
            yaml.dump(hardware_catalog.model_dump(mode="json"), f)

        # Write software catalog
        with open(catalog_dir / "software.yaml", "w") as f:
            yaml.dump(software_catalog.model_dump(mode="json"), f)

        # Write market prices
        with open(catalog_dir / "market-prices.yaml", "w") as f:
            yaml.dump(market_prices.model_dump(mode="json"), f)

        yield tmppath
