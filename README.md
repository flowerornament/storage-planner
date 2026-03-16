# Storage Planner

Rust CLI tool for evaluating purchase decisions, with a focus on storage systems. Models topologies, tracks product prices, and guides structured decision-making. Uses SQLite as the source of truth.

## Installation

```bash
# Nix flake (recommended)
nix profile install github:flowerornament/storage-planner

# Or build from source
cargo install --path .
```

## Quick Start

```bash
# Initialize database
sp init

# Create a topology and set it as current
sp topology create --name home-server
sp current set home-server

# Add nodes, volumes, datasets
sp node add nas --desc "Synology DS1621+"
sp volume add nas/pool1 --capacity 16TB --type raid6
sp dataset add photos --size 2TB --replicas 2
sp placement add photos nas/pool1

# Track products and prices
sp catalog add --name "Samsung 870 EVO 4TB" --category ssd
sp catalog price <item-id> --price 199.99 --source Amazon

# Visualize and analyze
sp diagram
sp analyze capacity
sp status

# Decision workflow
sp decision create --title "NVMe vs SATA for NAS expansion"
sp decision compare <id>
sp decision choose <id> <option> --rationale "Best value per TB"
```

## Commands

| Command | Description |
|---------|-------------|
| `sp init` | Initialize database |
| `sp prime` | Full context dump (for AI agents) |
| `sp status` | Health overview with problems |
| `sp topology` | Manage named configurations |
| `sp node` | Compute nodes within a topology |
| `sp volume` | Storage volumes on nodes |
| `sp dataset` | Logical datasets with replication needs |
| `sp placement` | Map datasets to volumes |
| `sp link` | Network links between nodes |
| `sp sync` | Data sync regimes between volumes |
| `sp catalog` | Product catalog and price observations |
| `sp decision` | Purchase decision lifecycle |
| `sp analyze` | Run analysis reports |
| `sp diagram` | ASCII topology diagram |
| `sp export/import` | YAML topology export/import |
| `sp current` | Show or set current topology |
| `sp undo/redo` | Undo/redo last action |

## Project Structure

```
src/
├── main.rs               # CLI entry point
├── core/                 # Models, database, events
├── cli/                  # Command implementations
└── domains/storage/      # Storage-specific analysis
```

## Development

```bash
just check    # fmt + lint + test
just fmt      # Format code
just lint     # Run clippy
just test     # Run tests
```

## License

MIT
