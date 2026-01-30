"""Topology models for storage planner."""

from typing import Optional
from pydantic import BaseModel, Field

from storage_planner.models.enums import (
    NodeType,
    VolumeType,
    LinkType,
    Criticality,
    ChangeRate,
    SyncMethod,
    SyncDirection,
    PowerProfile,
)


class Volume(BaseModel):
    """A storage volume attached to a node."""

    id: str
    name: Optional[str] = None
    type: VolumeType
    raw_capacity: str  # e.g., "8TB", "500GB"
    usable_capacity: Optional[str] = None
    used: Optional[str] = None
    raid_level: Optional[str] = None  # e.g., "zfs_raidz1", "raid5"
    raid_disks: Optional[int] = None
    read_speed: Optional[str] = None  # e.g., "560MB/s"
    write_speed: Optional[str] = None
    purchase_cost: Optional[float] = None
    purchase_date: Optional[str] = None
    product_id: Optional[str] = None  # Reference to hardware catalog
    hosts_datasets: list[str] = Field(default_factory=list)


class Node(BaseModel):
    """A compute node (device) in the topology."""

    id: str
    name: str
    type: NodeType
    location: Optional[str] = None
    power_profile: Optional[PowerProfile] = None
    uptime: Optional[str] = None  # e.g., "24/7", "8h/day"
    noise_db: Optional[float] = None
    power_watts_idle: Optional[float] = None
    power_watts_active: Optional[float] = None
    monthly_cost: Optional[float] = None  # Hosting/operational cost
    volumes: list[Volume] = Field(default_factory=list)


class Link(BaseModel):
    """A connection between two nodes."""

    id: str
    node_a: str  # Node ID
    node_b: str  # Node ID
    type: LinkType = LinkType.LAN
    bandwidth_up: Optional[str] = None  # e.g., "10Gbps", "500Mbps"
    bandwidth_down: Optional[str] = None
    latency_ms: Optional[float] = None
    availability_percent: Optional[float] = None
    cost_per_gb: Optional[float] = None  # For metered connections


class Dataset(BaseModel):
    """A logical group of data that needs protection."""

    id: str
    name: str
    current_size: str  # e.g., "50GB"
    growth_rate: Optional[str] = None  # e.g., "1GB/month", "10%/year"
    criticality: Criticality = Criticality.IMPORTANT
    change_rate: ChangeRate = ChangeRate.MEDIUM
    data_type: Optional[str] = None  # e.g., "documents", "photos", "database"
    required_copies: int = 2
    required_locations: int = 1
    max_rpo: Optional[str] = None  # e.g., "1h", "24h", "7d"
    max_rto: Optional[str] = None  # Recovery time objective
    stored_on: list[str] = Field(default_factory=list)  # Volume IDs
    accessible_from: list[str] = Field(default_factory=list)  # Node IDs
    primary_volume: Optional[str] = None  # For transparent switching
    fallback_volume: Optional[str] = None


class SyncRegime(BaseModel):
    """Defines how data moves between volumes."""

    id: str
    dataset: str  # Dataset ID
    source_volume: str  # Volume ID
    target_volumes: list[str]  # Volume IDs
    method: SyncMethod
    software_id: Optional[str] = None  # Reference to software catalog
    direction: SyncDirection = SyncDirection.SOURCE_TO_TARGET
    schedule: Optional[str] = None  # Cron expression or description
    continuous: bool = False
    bandwidth_limit: Optional[str] = None  # e.g., "100MB/s"
    achieved_rpo: Optional[str] = None  # Actual RPO achieved


class Constraints(BaseModel):
    """Global constraints for the topology.

    Note: Fields with defaults embed assumptions. Use `sp validate --strict`
    to identify fields that should be explicitly set for accurate analysis.
    """

    # Cost constraints
    max_monthly_cost: Optional[float] = None
    power_cost_per_kwh: Optional[float] = None  # e.g., 0.12 for $0.12/kWh

    # Redundancy constraints (defaults shown - override for your requirements)
    min_critical_data_copies: Optional[int] = None  # No default - must be explicit
    min_important_data_copies: Optional[int] = None  # No default - must be explicit
    min_locations_for_critical: Optional[int] = None  # No default - must be explicit

    # Environment constraints
    max_noise_db_home: Optional[float] = None


class Topology(BaseModel):
    """Complete storage topology definition."""

    name: str
    version: str = "1.0"
    description: Optional[str] = None
    constraints: Constraints = Field(default_factory=Constraints)
    nodes: list[Node] = Field(default_factory=list)
    links: list[Link] = Field(default_factory=list)
    datasets: list[Dataset] = Field(default_factory=list)
    sync_regimes: list[SyncRegime] = Field(default_factory=list)

    def get_node(self, node_id: str) -> Optional[Node]:
        """Get a node by ID."""
        for node in self.nodes:
            if node.id == node_id:
                return node
        return None

    def get_volume(self, volume_id: str) -> Optional[tuple[Node, Volume]]:
        """Get a volume by ID, returns (node, volume) tuple."""
        for node in self.nodes:
            for volume in node.volumes:
                if volume.id == volume_id:
                    return (node, volume)
        return None

    def get_dataset(self, dataset_id: str) -> Optional[Dataset]:
        """Get a dataset by ID."""
        for dataset in self.datasets:
            if dataset.id == dataset_id:
                return dataset
        return None

    def get_link(self, link_id: str) -> Optional[Link]:
        """Get a link by ID."""
        for link in self.links:
            if link.id == link_id:
                return link
        return None

    def get_all_volume_ids(self) -> set[str]:
        """Get all volume IDs in the topology."""
        ids = set()
        for node in self.nodes:
            for volume in node.volumes:
                ids.add(volume.id)
        return ids

    def get_all_node_ids(self) -> set[str]:
        """Get all node IDs in the topology."""
        return {node.id for node in self.nodes}
