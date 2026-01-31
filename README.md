# Storage Planner

Rust CLI tool for evaluating purchase decisions, with a focus on storage systems. Uses SQLite as the source of truth, with all mutations going through CLI commands.

## Installation

```bash
# Build from source
cargo build --release

# The binary is at ./target/release/sp
./target/release/sp --help

# Or install to your path
cargo install --path .
```

## Quick Start

```bash
# Initialize database
sp init

# Add items to catalog
sp item add samsung-870-evo-4tb \
  --name="Samsung 870 EVO 4TB" \
  --category=ssd \
  --brand=Samsung \
  --specs='{"capacity":"4TB","read_speed":"560MB/s"}'

# Record prices
sp price add samsung-870-evo-4tb --price=289 --condition=new

# Create and compare configurations
sp config create "SATA Setup"
sp config add-item "SATA Setup" samsung-870-evo-4tb --qty=2

# Make decisions
sp decide create --purpose="Replace NAS with SSD"
sp decide add-option sata --config="SATA Setup"
sp decide compare
sp decide choose sata --rationale="Best value per TB"
sp decide deploy

# Check status
sp prime                    # Full context for agents
sp doctor                   # Health check
sp analyze                  # Run analysis on current config
```

## Key Commands

| Command | Description |
|---------|-------------|
| `sp init` | Initialize database |
| `sp prime` | Output full context (for agents) |
| `sp doctor` | Health check |
| `sp item *` | Manage catalog items |
| `sp price *` | Manage price observations |
| `sp config *` | Manage configurations |
| `sp decide *` | Decision workflow |
| `sp analyze` | Run analysis |
| `sp sync` | Export to YAML |
| `sp events` | View audit log |

## Documentation

- **[CLAUDE.md](CLAUDE.md)** - Quick reference for agents/users
- **[docs/](docs/)** - Additional documentation

## Project Structure

```
storage-planner/
├── .sp/                      # Database (gitignored)
│   └── decisions.db          # SQLite - source of truth
├── export/                   # Read-only YAML exports
├── src/                      # Rust implementation
│   ├── core/                 # Domain-agnostic models
│   ├── cli/                  # Command implementations
│   ├── domains/storage/      # Storage-specific analysis
│   └── pricing/              # Price API stubs
├── catalog/                  # Legacy YAML (for migration)
└── python-archive/           # Old Python implementation
```

## Design Principles

1. **SQLite is truth** - Database is the source of truth, not YAML files
2. **Append-only events** - All changes recorded in audit log
3. **Atomic operations** - Commands complete fully or not at all
4. **No direct editing** - Use `sp` commands, not file edits

## Development

```bash
cargo build                   # Build
cargo test                    # Run tests
cargo build --release         # Release build
```

## License

MIT
