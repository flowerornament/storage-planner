# Research Workflow

This guide explains how to research and add products to the catalog.

## Overview

1. **Research products** - Find specs, reviews, pricing
2. **Add to catalog** - Use `sp item add` with specs
3. **Record prices** - Use `sp price add` with current prices
4. **Build configurations** - Combine items for comparison

## Researching Products

### Finding Specs

Look for these key specifications:

**Storage (SSD/HDD):**
- Capacity (e.g., 4TB)
- Interface (SATA, NVMe, USB)
- Read/write speeds
- Form factor (2.5", M.2, etc.)
- Endurance (TBW)
- Noise level (dB) - especially for HDDs

**Enclosures:**
- Drive compatibility
- Interface (USB-C, Thunderbolt)
- Number of bays
- RAID support
- Noise level

### Sources

- Manufacturer spec sheets
- Amazon product pages
- AnandTech, Tom's Hardware reviews
- Reddit r/DataHoarder discussions

## Adding Products

```bash
# Add an SSD
sp item add samsung-870-evo-4tb \
  --name="Samsung 870 EVO 4TB" \
  --category=ssd \
  --brand=Samsung \
  --specs='{
    "capacity": "4TB",
    "read_speed": "560MB/s",
    "write_speed": "530MB/s",
    "interface": "SATA",
    "form_factor": "2.5inch",
    "endurance_tbw": 2400,
    "noise_db": 0
  }' \
  --tags=sata,ssd,2.5inch,quiet

# Add an enclosure
sp item add owc-dual-mini \
  --name="OWC Dual Drive Dock" \
  --category=enclosure \
  --brand=OWC \
  --specs='{
    "interface": "USB-C",
    "drives": 2,
    "form_factor": "2.5inch",
    "raid_support": false,
    "noise_db": 0
  }' \
  --tags=enclosure,usb-c,portable,quiet
```

### Spec Guidelines

- Use consistent units: TB (not TiB), MB/s, dB
- Include `noise_db: 0` for silent devices (SSDs)
- Use lowercase for interface names (sata, nvme, usb-c)
- Tags should be lowercase, comma-separated

## Recording Prices

```bash
# New retail price
sp price add samsung-870-evo-4tb --price=689 --condition=new --source=amazon

# Used/refurbished prices
sp price add samsung-870-evo-4tb --price=289 --condition=used --source=ebay

# With URL for reference
sp price add samsung-870-evo-4tb \
  --price=289 \
  --condition=new \
  --source=bestbuy \
  --url="https://www.bestbuy.com/..."
```

### Price Guidelines

- Record prices in USD
- Note the condition (new, used, refurbished, open_box)
- Include source for traceability
- Prices are append-only - add new observations, don't update old ones
- Check staleness with `sp prime` (warns if >7 days old)

## Building Configurations

After adding items and prices:

```bash
# Create a configuration
sp config create "Budget SATA Setup"

# Add items
sp config add-item "Budget SATA Setup" samsung-870-evo-4tb --qty=2
sp config add-item "Budget SATA Setup" owc-dual-mini --qty=1

# Review
sp config show "Budget SATA Setup"
```

## Comparison Workflow

```bash
# Create multiple configurations
sp config create "SATA Option"
sp config add-item "SATA Option" samsung-870-evo-4tb --qty=2
sp config add-item "SATA Option" owc-dual-mini --qty=1

sp config create "NVMe Option"
sp config add-item "NVMe Option" lexar-nm790-4tb --qty=2
sp config add-item "NVMe Option" orico-nvme-enclosure --qty=1

# Compare items
sp item compare samsung-870-evo-4tb lexar-nm790-4tb

# Compare prices
sp price compare samsung-870-evo-4tb lexar-nm790-4tb

# Create decision
sp decide create --purpose="Choose storage upgrade"
sp decide add-option sata --config="SATA Option"
sp decide add-option nvme --config="NVMe Option"
sp decide compare

# Analyze each
sp analyze "SATA Option"
sp analyze "NVMe Option"
```

## Migrating from Legacy YAML

If you have products in the old `catalog/hardware.yaml`:

```bash
# View old catalog
cat catalog/hardware.yaml

# Manually add to new system
sp item add <id> --name="..." --category=... --specs='...'
sp price add <id> --price=... --condition=new
```

The migration is manual because:
- Old format had different structure
- Prices need fresh observations
- Opportunity to clean up data

## Best Practices

1. **Use descriptive IDs** - `samsung-870-evo-4tb` not `ssd-1`
2. **Include all relevant specs** - More data = better analysis
3. **Tag consistently** - Enables filtering (`--tags=quiet,ssd`)
4. **Update prices regularly** - Stale prices lead to bad decisions
5. **Document sources** - Use `--url` for traceability
