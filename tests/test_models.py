"""Tests for data models."""


from storage_planner.models import (
    ChangeRate,
    Criticality,
    Dataset,
    Link,
    LinkType,
    Node,
    NodeType,
    PowerProfile,
    ProductCategory,
    SyncDirection,
    SyncMethod,
    SyncRegime,
    Volume,
    VolumeType,
)


class TestEnums:
    """Test enum values."""

    def test_node_types(self):
        assert NodeType.LAPTOP.value == "laptop"
        assert NodeType.SERVER.value == "server"
        assert NodeType.NAS.value == "nas"

    def test_volume_types(self):
        assert VolumeType.INTERNAL_SSD.value == "internal_ssd"
        assert VolumeType.RAID_ARRAY.value == "raid_array"

    def test_criticality(self):
        assert Criticality.CRITICAL.value == "critical"
        assert Criticality.REPLACEABLE.value == "replaceable"

    def test_change_rate(self):
        assert ChangeRate.STATIC.value == "static"
        assert ChangeRate.REALTIME.value == "realtime"

    def test_sync_method(self):
        assert SyncMethod.RESILIO_SYNC.value == "resilio_sync"
        assert SyncMethod.BORG.value == "borg"


class TestVolume:
    """Test Volume model."""

    def test_minimal_volume(self):
        vol = Volume(id="test", type=VolumeType.INTERNAL_SSD, raw_capacity="1TB")
        assert vol.id == "test"
        assert vol.raw_capacity == "1TB"
        assert vol.usable_capacity is None
        assert vol.hosts_datasets == []

    def test_full_volume(self):
        vol = Volume(
            id="test",
            name="Test Volume",
            type=VolumeType.RAID_ARRAY,
            raw_capacity="10TB",
            usable_capacity="8TB",
            used="2TB",
            raid_level="raidz1",
            raid_disks=4,
            read_speed="500MB/s",
            write_speed="400MB/s",
            purchase_cost=1000.0,
            product_id="some-product",
            hosts_datasets=["dataset1", "dataset2"],
        )
        assert vol.raid_level == "raidz1"
        assert vol.raid_disks == 4
        assert len(vol.hosts_datasets) == 2


class TestNode:
    """Test Node model."""

    def test_minimal_node(self):
        node = Node(id="test", name="Test", type=NodeType.DESKTOP, volumes=[])
        assert node.id == "test"
        assert node.volumes == []

    def test_node_with_volumes(self):
        node = Node(
            id="test",
            name="Test",
            type=NodeType.SERVER,
            location="datacenter",
            power_profile=PowerProfile.ALWAYS_ON,
            power_watts_idle=50,
            monthly_cost=100.0,
            volumes=[
                Volume(id="vol1", type=VolumeType.NVME, raw_capacity="500GB"),
                Volume(id="vol2", type=VolumeType.RAID_ARRAY, raw_capacity="10TB"),
            ],
        )
        assert len(node.volumes) == 2
        assert node.monthly_cost == 100.0


class TestLink:
    """Test Link model."""

    def test_minimal_link(self):
        link = Link(id="test", node_a="a", node_b="b")
        assert link.type == LinkType.LAN  # default

    def test_full_link(self):
        link = Link(
            id="test",
            node_a="a",
            node_b="b",
            type=LinkType.WAN,
            bandwidth_up="100Mbps",
            bandwidth_down="500Mbps",
            latency_ms=30,
            availability_percent=99.9,
            cost_per_gb=0.01,
        )
        assert link.bandwidth_up == "100Mbps"
        assert link.cost_per_gb == 0.01


class TestDataset:
    """Test Dataset model."""

    def test_minimal_dataset(self):
        ds = Dataset(id="test", name="Test", current_size="10GB")
        assert ds.criticality == Criticality.IMPORTANT  # default
        assert ds.change_rate == ChangeRate.MEDIUM  # default
        assert ds.required_copies == 2  # default

    def test_full_dataset(self):
        ds = Dataset(
            id="test",
            name="Test",
            current_size="100GB",
            growth_rate="5GB/month",
            criticality=Criticality.CRITICAL,
            change_rate=ChangeRate.HIGH,
            data_type="documents",
            required_copies=3,
            required_locations=2,
            max_rpo="1h",
            max_rto="4h",
            stored_on=["vol1", "vol2"],
            accessible_from=["node1"],
            primary_volume="vol1",
            fallback_volume="vol2",
        )
        assert ds.max_rpo == "1h"
        assert len(ds.stored_on) == 2


class TestSyncRegime:
    """Test SyncRegime model."""

    def test_minimal_sync_regime(self):
        sr = SyncRegime(
            id="test",
            dataset="ds1",
            source_volume="vol1",
            target_volumes=["vol2"],
            method=SyncMethod.RSYNC,
        )
        assert sr.direction == SyncDirection.SOURCE_TO_TARGET  # default
        assert sr.continuous is False  # default

    def test_continuous_sync_regime(self):
        sr = SyncRegime(
            id="test",
            dataset="ds1",
            source_volume="vol1",
            target_volumes=["vol2", "vol3"],
            method=SyncMethod.RESILIO_SYNC,
            direction=SyncDirection.BIDIRECTIONAL,
            continuous=True,
            achieved_rpo="30s",
        )
        assert sr.continuous is True
        assert len(sr.target_volumes) == 2


class TestTopology:
    """Test Topology model."""

    def test_minimal_topology(self, minimal_topology):
        assert minimal_topology.name == "Test Topology"
        assert len(minimal_topology.nodes) == 1

    def test_get_node(self, full_topology):
        node = full_topology.get_node("laptop")
        assert node is not None
        assert node.name == "Laptop"

        missing = full_topology.get_node("nonexistent")
        assert missing is None

    def test_get_volume(self, full_topology):
        result = full_topology.get_volume("laptop-ssd")
        assert result is not None
        node, volume = result
        assert node.id == "laptop"
        assert volume.id == "laptop-ssd"

        missing = full_topology.get_volume("nonexistent")
        assert missing is None

    def test_get_dataset(self, full_topology):
        ds = full_topology.get_dataset("docs")
        assert ds is not None
        assert ds.name == "Documents"

    def test_get_link(self, full_topology):
        link = full_topology.get_link("home-lan")
        assert link is not None
        assert link.node_a == "laptop"

    def test_get_all_volume_ids(self, full_topology):
        ids = full_topology.get_all_volume_ids()
        assert "laptop-ssd" in ids
        assert "server-raid" in ids
        assert "nas-array" in ids
        assert len(ids) == 3

    def test_get_all_node_ids(self, full_topology):
        ids = full_topology.get_all_node_ids()
        assert ids == {"laptop", "server", "nas"}


class TestHardwareCatalog:
    """Test HardwareCatalog model."""

    def test_get_product(self, hardware_catalog):
        product = hardware_catalog.get_product("ssd-4tb")
        assert product is not None
        assert product.name == "Test SSD 4TB"

        missing = hardware_catalog.get_product("nonexistent")
        assert missing is None

    def test_get_by_category(self, hardware_catalog):
        ssds = hardware_catalog.get_by_category(ProductCategory.SSD)
        assert len(ssds) == 2

        enclosures = hardware_catalog.get_by_category(ProductCategory.ENCLOSURE)
        assert len(enclosures) == 1

    def test_search(self, hardware_catalog):
        results = hardware_catalog.search("4TB")
        assert len(results) == 1
        assert results[0].id == "ssd-4tb"

        results = hardware_catalog.search("testbrand")
        assert len(results) == 3  # All products


class TestMarketPrices:
    """Test MarketPrices model."""

    def test_get_for_product(self, market_prices):
        prices = market_prices.get_for_product("ssd-4tb")
        assert len(prices) == 1
        assert prices[0].price_mid == 220

    def test_get_best_price(self, market_prices):
        best = market_prices.get_best_price("ssd-4tb")
        assert best is not None
        assert best.price_mid == 220

        missing = market_prices.get_best_price("nonexistent")
        assert missing is None


class TestSoftwareCatalog:
    """Test SoftwareCatalog model."""

    def test_get_software(self, software_catalog):
        sw = software_catalog.get_software("resilio")
        assert sw is not None
        assert sw.name == "Resilio Sync"

    def test_get_by_type(self, software_catalog):
        sync_tools = software_catalog.get_by_type("sync")
        assert len(sync_tools) == 2

        backup_tools = software_catalog.get_by_type("backup")
        assert len(backup_tools) == 1
