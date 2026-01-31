# Catalog Research Workflow

The catalog is a pre-researched database of hardware products. This document describes how to populate and maintain it.

## Why Cache Products?

Instead of web searching every time you need a recommendation:
1. **Batch research** - Separate sessions to find and evaluate products
2. **Instant decisions** - `sp suggest hardware` uses cached data, no web searches
3. **Curated quality** - Products vetted with pros/cons, not just specs
4. **Specs separate from prices** - Specs rarely change; prices are captured per-session

## Product Fields

Each product should have:

```yaml
- id: brand-model-capacity     # Lowercase, hyphenated
  name: "Full Product Name"
  brand: BrandName
  category: ssd                # ssd|hdd|enclosure|nas|cable

  # Discovery & filtering
  tags:                        # For filtering (sp catalog list --tag quiet)
    - high-capacity
    - quiet
    - value
    - sata/nvme/thunderbolt
  use_cases:                   # For matching to topology needs
    - time-machine-target
    - working-drive
    - portable-backup

  # Evaluation
  pros:
    - Key advantage 1
    - Key advantage 2
  cons:
    - Trade-off or limitation

  # Specs
  specs:
    capacity: "4TB"
    interface: "SATA"
    # ... category-specific fields

  # Metadata
  noise_db: 0                  # For quiet requirements
  discontinued: false          # Mark when no longer sold new
  last_verified: "2025-01-15"  # When info was checked
  notes: "Summary notes"
```

**Note:** Prices do NOT go in the catalog. They are captured in session files when making decisions.

## Standard Tags

Use consistent tags for filtering:

**Capacity:**
- `high-capacity` - 4TB+
- `portable` - Travel-friendly form factor

**Performance:**
- `fast` - Above-average speeds
- `nvme` / `sata` / `thunderbolt` - Interface type
- `pcie5` - PCIe 5.0 drives

**Use Pattern:**
- `quiet` - 0 dB or near-silent
- `endurance` - High TBW, write-heavy workloads
- `value` - Good price/performance

**Technology:**
- `qlc` / `tlc` / `mlc` - NAND type
- `rugged` / `ip65` - Durability rated

**Ecosystem:**
- `mac-optimized` - Known Mac compatibility
- `m4-compatible` - Tested with M4 Macs

## Standard Use Cases

Match products to these use cases:

**Backup Targets:**
- `time-machine-target` - macOS Time Machine destination
- `cold-storage` - Rarely accessed archive
- `media-archive` - Photos, videos, music

**Working Storage:**
- `working-drive` - Active project files
- `video-editing` - Large file, high bandwidth
- `portable-working-drive` - Working files on the go

**Portability:**
- `travel-backup` - Offsite/travel backup
- `portable-backup` - General portable use
- `offsite-rotation` - Rotating offsite copies

**Infrastructure:**
- `nas-replacement` - Replace spinning disk NAS
- `mac-mini-expansion` - Mac mini hub storage
- `desktop-hub` - Dock/hub use

## Research Session Template

When researching a category:

```markdown
## Research: [Category] for [Use Case]

### Requirements
- Capacity: X TB
- Interface: USB-C / Thunderbolt / etc.
- Noise: Silent preferred
- Budget: $X-Y

### Search Strategy
1. Check existing catalog: `sp catalog list --use-case [use-case]`
2. Search [Amazon/NewEgg/B&H] for [query]
3. Check reviews on [sites]
4. Check used prices on eBay/r/hardwareswap

### Products Found
[Add specs to catalog/hardware.yaml - NO PRICES]

### Price Research
[Capture prices in sessions/<date>.yaml when making decisions]

### Summary
- Best value: X
- Best performance: Y
- Best for quiet: Z
```

## Capturing Prices

Prices are captured in **session files**, not the catalog:

```yaml
# sessions/2026-01-30.yaml
prices:
  captured: "2026-01-30"

  samsung-870-qvo-8tb:
    retail: 800
    used_low: 719
    used_high: 850
    notes: "eBay completed sales"
```

This keeps prices point-in-time and tied to specific decisions.

## CLI Commands

```bash
# See what's cached
sp catalog summary -c catalog

# Filter by tags
sp catalog list --tag quiet --tag high-capacity -c catalog

# Filter by use case
sp catalog list --use-case time-machine-target -c catalog

# Compare options
sp catalog compare samsung-870-qvo-4tb crucial-mx500-4tb -c catalog

# Get recommendations from cached data
sp suggest hardware current.yaml -c catalog
```
