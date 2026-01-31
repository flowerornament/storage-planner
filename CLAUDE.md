# Storage Planner

Rust CLI tool for **evaluating purchase decisions** with a focus on storage systems. All mutations go through `sp` commands; the SQLite database is the source of truth.

## First Thing: Get Context

Run this command immediately to understand the current state:

```bash
sp prime
```

This shows:
- Database stats (items, prices, configurations, decisions)
- Current deployed configuration
- Active decision session (if any)
- Items with stale prices
- Recent events

For JSON output (better for parsing): `sp prime --format=json`

## What This Tool Is For

This tool helps you make informed purchase decisions by:
1. **Managing a catalog** - Items with specs, tags, and price history
2. **Tracking prices** - Append-only price observations with staleness tracking
3. **Building configurations** - Compositions of items with costs
4. **Running analysis** - Redundancy, capacity, noise, cost checks
5. **Making decisions** - Structured workflow with comparison and rationale

**Key principle:** Agents can't break structure. All mutations go through `sp` commands, not direct file editing.

## Quick Reference

```bash
# Context and health
sp prime                          # Agent context dump
sp doctor                         # Health check
sp events -n 10                   # Recent audit log

# Catalog management
sp item add <id> --name=... --category=... --specs='{...}'
sp item list [--category=ssd] [--tags=nvme]
sp item show <id> --prices        # Include price history
sp item compare <id1> <id2>       # Side-by-side comparison
sp item search <query>            # Full-text search

# Price management
sp price add <item-id> --price=299 --condition=new --source=manual
sp price show <item-id>           # Current prices by condition
sp price history <item-id>        # Price trend
sp price compare <id1> <id2>      # Compare prices

# Configuration management
sp config current                 # Show deployed configuration
sp config create <name>           # New empty configuration
sp config add-item <config> <item-id> --qty=2
sp config show <config>           # Details with cost
sp config set-current <config>    # Deploy configuration

# Decision workflow
sp decide create --purpose="..."  # Start decision session
sp decide add-option <name> --config=<config>
sp decide compare                 # Compare all options
sp decide choose <option> --rationale="..."
sp decide deploy                  # Set chosen config as current
sp decide history                 # Past decisions

# Analysis
sp analyze                        # Analyze current configuration
sp analyze <config>               # Analyze specific configuration

# Export (read-only snapshots)
sp sync                           # Export DB to YAML in export/
```

## Agent Workflow

### Starting a Session

```bash
sp prime                          # Get full context
```

### Adding Products to Catalog

```bash
sp item add samsung-870-evo-4tb \
  --name="Samsung 870 EVO 4TB" \
  --category=ssd \
  --brand=Samsung \
  --specs='{"capacity":"4TB","read_speed":"560MB/s","interface":"SATA"}' \
  --tags=sata,ssd,2.5inch
```

### Recording Prices

```bash
sp price add samsung-870-evo-4tb --price=289 --condition=new --source=manual
sp price add samsung-870-evo-4tb --price=180 --condition=used --source=ebay
```

### Making a Decision

```bash
# 1. Create configurations for each option
sp config create "SATA Option"
sp config add-item "SATA Option" samsung-870-evo-4tb --qty=2
sp config add-item "SATA Option" owc-dual-mini --qty=1

sp config create "NVMe Option"
sp config add-item "NVMe Option" lexar-nm790-4tb --qty=2

# 2. Create decision session
sp decide create --purpose="Replace NAS with silent SSD storage"

# 3. Add options
sp decide add-option sata --config="SATA Option"
sp decide add-option nvme --config="NVMe Option"

# 4. Compare
sp decide compare

# 5. Choose and deploy
sp decide choose sata --rationale="Better value per TB with RAID1 redundancy"
sp decide deploy
```

## File Structure

```
storage-planner/
├── .sp/                      # Database (gitignored)
│   └── decisions.db          # SQLite - source of truth
├── export/                   # Read-only YAML exports
│   ├── current.yaml          # Current deployed state
│   ├── catalog/              # Items by category
│   └── history/              # Decision snapshots
├── src/                      # Rust implementation
│   ├── main.rs               # CLI entry point
│   ├── core/                 # Domain-agnostic models
│   ├── cli/                  # Command implementations
│   ├── domains/storage/      # Storage-specific analysis
│   └── pricing/              # Price API integrations (stubs)
├── catalog/                  # Legacy YAML catalog (to migrate to DB)
├── sessions/                 # Legacy decision sessions (to migrate)
├── archive/                  # Legacy proposals/topologies (to migrate)
├── current.yaml              # Legacy current config (to migrate)
└── system.yaml               # Legacy system config (to migrate)
```

## Development

```bash
cargo build                   # Build
cargo test                    # Run tests
cargo build --release         # Release build
./target/release/sp --help    # Run CLI
```

## Key Design Principles

1. **SQLite is truth** - Database is the source of truth, not YAML files
2. **Append-only events** - All changes recorded in audit log
3. **Atomic operations** - Commands complete fully or not at all
4. **No direct editing** - Agents use `sp` commands, not file edits
5. **Self-documenting** - `sp <command> --help` explains everything

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below:

1. **File issues for remaining work** - `bd create --type=task --title="..."`
2. **Run quality gates** - `cargo test && cargo build`
3. **Update issue status** - `bd close <id1> <id2> ...`
4. **Sync and push**:
   ```bash
   bd sync --flush-only
   git add -A && git commit -m "..." && git push
   ```
5. **Verify** - `git status` shows clean, up to date with origin
