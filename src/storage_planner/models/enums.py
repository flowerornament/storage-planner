"""Enumerations for storage planner models."""

from enum import Enum


class NodeType(str, Enum):
    """Type of compute node."""

    LAPTOP = "laptop"
    DESKTOP = "desktop"
    SERVER = "server"
    NAS = "nas"
    CLOUD = "cloud"


class VolumeType(str, Enum):
    """Type of storage volume."""

    INTERNAL_SSD = "internal_ssd"
    INTERNAL_HDD = "internal_hdd"
    EXTERNAL_SSD = "external_ssd"
    EXTERNAL_HDD = "external_hdd"
    NVME = "nvme"
    RAID_ARRAY = "raid_array"
    CLOUD = "cloud"


class LinkType(str, Enum):
    """Type of network/connection link."""

    LAN = "lan"
    WAN = "wan"
    VPN = "vpn"
    THUNDERBOLT = "thunderbolt"
    USB = "usb"
    INTERNAL = "internal"


class Criticality(str, Enum):
    """Data criticality level."""

    CRITICAL = "critical"
    IMPORTANT = "important"
    REPLACEABLE = "replaceable"


class ChangeRate(str, Enum):
    """How frequently data changes."""

    STATIC = "static"
    LOW = "low"
    MEDIUM = "medium"
    HIGH = "high"
    REALTIME = "realtime"


class SyncMethod(str, Enum):
    """Synchronization/backup method."""

    RESILIO_SYNC = "resilio_sync"
    SYNCTHING = "syncthing"
    TIME_MACHINE = "time_machine"
    RSYNC = "rsync"
    BORG = "borg"
    RCLONE = "rclone"
    POSTGRES_REPLICATION = "postgres_replication"
    LITESTREAM = "litestream"
    MANUAL = "manual"


class SyncDirection(str, Enum):
    """Direction of synchronization."""

    SOURCE_TO_TARGET = "source_to_target"
    BIDIRECTIONAL = "bidirectional"


class ProductCategory(str, Enum):
    """Hardware product category."""

    SSD = "ssd"
    HDD = "hdd"
    ENCLOSURE = "enclosure"
    NAS = "nas"
    CABLE = "cable"


class PowerProfile(str, Enum):
    """Power usage profile of a node."""

    MOBILE = "mobile"
    ALWAYS_ON = "always_on"
    SCHEDULED = "scheduled"
    ON_DEMAND = "on_demand"
