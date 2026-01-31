# YAML Schema Reference

All configuration is YAML. This documents the schema for each file type.

## Topology Schema

**File:** `current.yaml` (deployed state) or embedded in `sessions/<date>.yaml` (options)

```yaml
name: "Topology Name"           # Required
deployed: "2024-01-01"          # Optional, when this was deployed
from_session: "2026-01-30"      # Optional, session that led to this config
version: "1.0"                  # Optional, default "1.0"
description: "..."              # Optional, multiline description

constraints:                    # Optional, global constraints
  max_monthly_cost: 150.00
  min_critical_data_copies: 3   # Default: 3
  min_important_data_copies: 2  # Default: 2
  min_locations_for_critical: 2 # Default: 2
  max_noise_db_home: 35         # For quiet setups

nodes: []        # List of Node
links: []        # List of Link
datasets: []     # List of Dataset
sync_regimes: [] # List of SyncRegime
```

### Node

```yaml
- id: macbook-m4              # Required, unique identifier
  name: "MacBook Pro M4"      # Required, display name
  type: laptop                # Required: laptop|desktop|server|nas|cloud
  status: active              # Optional: active|deprecated|planned
  location: home-office       # Optional, for redundancy location counting
  power_profile: mobile       # Optional: mobile|always_on|scheduled|on_demand
  uptime: "24/7"              # Optional, descriptive
  noise_db: 0                 # Optional, for noise constraints
  power_watts_idle: 5         # Optional, for cost estimates
  power_watts_active: 25      # Optional
  monthly_cost: 89.00         # Optional, hosting/operational cost
  volumes: []                 # List of Volume
  problems:                   # Optional, node-specific issues
    - "Too loud for office"
    - "Running out of space"
```

### Volume

```yaml
- id: macbook-internal        # Required, unique across all nodes
  name: "Internal SSD"        # Optional
  type: internal_ssd          # Required: internal_ssd|internal_hdd|external_ssd|
                              #           external_hdd|nvme|raid_array|cloud
  raw_capacity: "1TB"         # Required, e.g., "500GB", "8TB"
  usable_capacity: "900GB"    # Optional
  used: "400GB"               # Optional, current usage
  raid_level: zfs_raidz1      # Optional, e.g., "raid5", "zfs_raidz1"
  raid_disks: 4               # Optional
  read_speed: "560MB/s"       # Optional
  write_speed: "530MB/s"      # Optional
  product_id: samsung-870-qvo-8tb  # Optional, references catalog for specs
  hosts_datasets:             # Optional, datasets stored here
    - working-docs
    - source-code
```

### Link

```yaml
- id: home-lan                # Required
  node_a: macbook-m4          # Required, node ID
  node_b: mac-mini-m4         # Required, node ID
  type: lan                   # Optional: lan|wan|vpn|thunderbolt|usb|internal
  bandwidth_up: "10Gbps"      # Optional (bits/s: Mbps/Gbps, bytes/s: MB/s/GB/s)
  bandwidth_down: "10Gbps"    # Optional (bits/s: Mbps/Gbps, bytes/s: MB/s/GB/s)
  latency_ms: 1               # Optional
  availability_percent: 99.9  # Optional
  cost_per_gb: 0.0            # Optional, for metered connections
```

### Dataset

```yaml
- id: working-docs            # Required
  name: "Active Documents"    # Required
  current_size: "50GB"        # Required
  growth_rate: "2GB/month"    # Optional, e.g., "10%/year"
  criticality: critical       # Optional: critical|important|replaceable
  change_rate: high           # Optional: static|low|medium|high|realtime
  data_type: documents        # Optional, for software matching
  required_copies: 3          # Optional, default 2
  required_locations: 2       # Optional, default 1
  max_rpo: "1h"               # Optional, e.g., "30m", "7d"
  max_rto: "4h"               # Optional
  stored_on:                  # Volume IDs where data lives
    - macbook-internal
    - eu-nvme-boot
  accessible_from:            # Node IDs that should access this data
    - macbook-m4
    - mac-mini-m4
  primary_volume: macbook-internal   # Optional, for selective sync
  fallback_volume: macbook-external  # Optional
```

### SyncRegime

```yaml
- id: docs-resilio            # Required
  dataset: working-docs       # Required, dataset ID
  source_volume: macbook-internal  # Required, volume ID
  target_volumes:             # Required, list of volume IDs
    - mini-external-array
    - eu-nvme-boot
  method: resilio_sync        # Required: resilio_sync|syncthing|time_machine|
                              #           rsync|borg|rclone|postgres_replication|manual
  software_id: resilio_sync   # Optional, references software catalog
  direction: bidirectional    # Optional: source_to_target|bidirectional
  schedule: "0 2 * * *"       # Optional, cron or description
  continuous: true            # Optional, default false
  bandwidth_limit: "100MB/s"  # Optional (bits/s: Mbps/Gbps, bytes/s: MB/s/GB/s)
  achieved_rpo: "30s"         # Optional, actual RPO achieved
```

## Session Schema

**File:** `sessions/<date>.yaml`

Sessions capture point-in-time decision contexts with prices and options.

```yaml
created: "2026-01-30"           # Required, ISO date
purpose: "Replace NAS"          # Required, what decision is being made
status: active                  # Required: active|decided|abandoned

context: |                      # Optional, background information
  Detailed explanation of why this session exists...

# Prices captured for this session (point-in-time, do not update)
prices:
  captured: "2026-01-30"        # When prices were looked up
  notes: "Market conditions"    # Optional context

  product-id:                   # One entry per product researched
    retail: 299                 # Current retail price
    used_low: 180               # Low end of used market
    used_high: 250              # High end of used market
    notes: "Micro Center price" # Optional

# Baseline: what current.yaml looked like when session started
baseline:
  name: "Current Setup Name"
  deployed: "2024-01-01"
  # ... summary of current state

# Options evaluated
options:
  option-a:                     # Option identifier
    name: "Option A Name"       # Display name
    summary: "Brief description"

    hardware:                   # Hardware list with prices
      - product: product-id     # References catalog
        qty: 1
        unit_price: 299         # Price at time of session
        source: retail          # retail|used|owned
        subtotal: 299

    total_cost: 299             # Sum of hardware

    pros:
      - Advantage 1
      - Advantage 2
    cons:
      - Trade-off 1

    noise_db: 0                 # Expected noise level
    local_capacity: "8TB"       # Summary of capacity

  option-b:
    # ... another option

# Decision (filled in when decided)
decision:
  chosen: null                  # "option-a" | "option-b" | etc
  rationale: null               # Why this option was chosen
  date: null                    # When decision was made
```

## Hardware Catalog Schema

**File:** `catalog/hardware.yaml`

The catalog contains product specs only. **Prices live in session files.**

```yaml
products:
  - id: samsung-870-qvo-8tb   # Required, unique
    name: "Samsung 870 QVO 8TB"  # Required
    brand: Samsung            # Required
    model: "MZ-77Q8T0B/AM"    # Optional

    category: ssd             # Required: ssd|hdd|enclosure|nas|cable

    # Discovery & filtering
    tags:                     # For filtering (sp catalog list --tag quiet)
      - high-capacity
      - quiet
      - sata
      - qlc
    use_cases:                # For matching to topology needs
      - time-machine-target
      - media-archive
      - cold-storage

    # Evaluation
    pros:                     # Key advantages
      - Highest capacity SATA SSD
      - Silent operation
    cons:                     # Trade-offs
      - QLC has lower write endurance

    # Specs (category-specific, see below)
    specs:
      capacity: "8TB"
      interface: "SATA"
      form_factor: "2.5in"
      read_speed: "560MB/s"
      write_speed: "530MB/s"
      tbw: "2880TB"
      warranty_years: 3
      nand_type: "QLC"

    # Metadata
    noise_db: 0               # Optional, for quiet requirements
    aesthetic_notes: "Space gray"  # Optional
    discontinued: false       # Mark when no longer sold new
    last_verified: "2025-01-15"  # When info was last checked
    notes: "QLC, good for archive"  # Optional
```

### Drive Specs (SSD/HDD)

```yaml
specs:
  capacity: "8TB"
  interface: "SATA"           # SATA, NVMe, USB-C, Thunderbolt
  form_factor: "2.5in"        # 2.5in, 3.5in, M.2, portable
  read_speed: "560MB/s"
  write_speed: "530MB/s"
  tbw: "2880TB"               # Total bytes written (endurance)
  warranty_years: 3
  nand_type: "QLC"            # QLC, TLC, MLC
```

### Enclosure Specs

```yaml
specs:
  bays: 2
  interface: "USB-C 3.1"
  max_capacity_per_bay: "16TB"
  form_factor: "2.5in/3.5in"
  stackable: true
  m4_mini_compatible: true
  raid_support:
    - JBOD
    - RAID0
    - RAID1
  power_delivery_watts: 98
```

## Software Catalog Schema

**File:** `catalog/software.yaml`

```yaml
software:
  - id: resilio_sync          # Required
    name: "Resilio Sync"      # Required
    type: sync                # Required: sync|backup|replication
    strengths:                # What it's good at
      - continuous
      - bidirectional
      - peer-to-peer
    weaknesses:               # Limitations
      - no-versioning
      - proprietary
    best_for:                 # Matching criteria
      change_rate:            # ChangeRate values
        - high
        - realtime
      direction: bidirectional  # SyncDirection
      criticality:            # Criticality values
        - critical
      data_type:              # Matches dataset.data_type
        - documents
      target: local-network   # local-network|remote-server|cloud
      max_rpo: "1h"           # Recommended when RPO is this strict
    platforms:
      - macos
      - linux
      - windows
    url: "https://..."
    notes: "..."
```

## Size/Duration Formats

| Format | Examples |
|--------|----------|
| Size | `500GB`, `8TB`, `256MB` |
| Bandwidth | `10Gbps`, `500Mbps`, `100MB/s` |
| Duration | `30s`, `5m`, `1h`, `7d`, `4w` |
| Growth | `2GB/month`, `10%/year` |
