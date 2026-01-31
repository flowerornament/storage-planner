# CLI Reference

The `sp` command is the primary interface to the storage planner. All mutations go through CLI commands - the SQLite database is the source of truth.

## Global Options

```
-d, --dir <DIR>        Database directory (default: .sp/)
    --format <FORMAT>  Output format: text, json, yaml (default: text)
-h, --help             Print help
-V, --version          Print version
```

## Initialization

```bash
sp init                 # Create database at .sp/decisions.db
sp init --force         # Reinitialize (drops existing data)
```

## Context & Health

```bash
sp prime                # Full context dump for agents
sp prime --format=json  # JSON output for parsing
sp doctor               # Health check
sp doctor --integrity   # Include SQLite integrity check
sp events -n 20         # View last 20 events
sp events -e <id>       # Events for specific entity
```

## Item Catalog

### Add Items

```bash
sp item add samsung-870-evo-4tb \
  --name="Samsung 870 EVO 4TB" \
  --category=ssd \
  --brand=Samsung \
  --specs='{"capacity":"4TB","read_speed":"560MB/s","write_speed":"530MB/s","interface":"SATA"}' \
  --tags=sata,ssd,2.5inch
```

### Query Items

```bash
sp item list                          # All items
sp item list --category=ssd           # Filter by category
sp item list --tags=nvme,quiet        # Filter by tags (any match)
sp item show <id>                     # Item details
sp item show <id> --prices            # Include price history
sp item search "samsung evo"          # Full-text search
sp item compare <id1> <id2> <id3>     # Side-by-side comparison
```

### Modify Items

```bash
sp item update <id> --specs='{"capacity":"8TB"}'  # Merge specs
sp item update <id> --tags=new,tags               # Replace tags
sp item archive <id>                              # Soft delete
```

## Price Management

### Record Prices

```bash
sp price add samsung-870-evo-4tb --price=289 --condition=new --source=manual
sp price add samsung-870-evo-4tb --price=180 --condition=used --source=ebay --url="https://..."
```

Conditions: `new`, `used`, `refurbished`, `open_box`
Sources: `manual`, `ebay`, `bestbuy`, `amazon`, or any custom source name

### Query Prices

```bash
sp price show <item-id>               # Current prices by condition
sp price history <item-id> -n 20      # Price trend
sp price compare <id1> <id2>          # Compare prices across items
sp price fetch --stale                # Refresh items with old prices (requires API keys)
```

## Configuration Management

### Create Configurations

```bash
sp config create "SATA Option"                    # New empty config
sp config create "NVMe Option" --domain=storage   # Specify domain
sp config clone "SATA Option" --name="SATA v2"    # Clone existing
```

### Build Configurations

```bash
sp config add-item "SATA Option" samsung-870-evo-4tb --qty=2
sp config add-item "SATA Option" owc-dual-mini --qty=1 --price=75
sp config remove-item "SATA Option" owc-dual-mini
```

### Query Configurations

```bash
sp config current                     # Show deployed configuration
sp config list                        # All configurations
sp config list --domain=storage       # Filter by domain
sp config show "SATA Option"          # Details with cost breakdown
```

### Deploy

```bash
sp config set-current "SATA Option"   # Set as current (without decision)
sp config archive "Old Config"        # Soft delete
```

## Decision Workflow

### Create Decision Session

```bash
sp decide create --purpose="Replace NAS with silent SSD storage"
```

Only one decision can be active at a time.

### Add Options

```bash
sp decide add-option sata --config="SATA Option"
sp decide add-option nvme --config="NVMe Option"
```

### Compare & Choose

```bash
sp decide compare                     # Side-by-side comparison
sp decide choose sata --rationale="Better value per TB with RAID1 redundancy"
```

### Deploy

```bash
sp decide deploy                      # Set chosen config as current
```

### Other

```bash
sp decide history -n 10               # Past decisions
sp decide show <id>                   # Decision details
sp decide abandon --reason="..."      # Cancel active decision
```

## Analysis

```bash
sp analyze                            # Analyze current configuration
sp analyze "SATA Option"              # Analyze specific config
sp analyze --check=redundancy,cost    # Run specific checks only
sp analyze --format=json              # JSON output
```

Available checks: `cost`, `capacity`, `noise`, `redundancy`

## Export

```bash
sp sync                               # Export to export/ directory
sp sync --output=backup/              # Custom output directory
sp sync --catalog-only                # Only export catalog
```

Exports are read-only YAML snapshots. The database remains the source of truth.

## JSON Output

All query commands support `--format=json` for structured output:

```bash
sp prime --format=json
sp item list --format=json
sp price show <id> --format=json
sp config show <name> --format=json
sp analyze --format=json
```

## Environment Variables

```bash
SP_DIR                  # Database directory (default: .sp)
SP_EBAY_APP_ID          # eBay API app ID
SP_EBAY_CERT_ID         # eBay API cert ID
SP_BESTBUY_API_KEY      # Best Buy API key
```
