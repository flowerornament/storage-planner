# Storage Planner

CLI tool for modeling and analyzing storage/backup topologies. Define your setup in YAML, get analysis and recommendations.

## Installation

```bash
# Clone and enter directory
cd storage-planner

# Create virtual environment and install
uv venv
source .venv/bin/activate
uv pip install -e .

# Verify installation
sp --help
```

## Quick Start

```bash
# Validate current deployment
sp validate current.yaml

# Run full analysis
sp analyze all current.yaml

# Run quick summaries (redundancy, RPO/RTO, capacity)
sp analyze quick current.yaml

# Simulate a failure
sp simulate macbook-m4 current.yaml

# Browse hardware catalog
sp catalog summary -c catalog
sp catalog list --use-case time-machine-target -c catalog

# Compare products
sp catalog compare samsung-870-qvo-4tb crucial-mx500-4tb -c catalog

# JSON output for agents/tools
sp analyze redundancy current.yaml --json
```

## Documentation

- **[CLAUDE.md](CLAUDE.md)** - Quick reference for agents/users
- **[docs/schema.md](docs/schema.md)** - YAML schema reference
- **[docs/analysis.md](docs/analysis.md)** - How analysis works
- **[docs/cli.md](docs/cli.md)** - CLI reference & JSON output
- **[docs/extending.md](docs/extending.md)** - Adding features
- **[docs/research-workflow.md](docs/research-workflow.md)** - Populating the catalog

## Project Structure

```
storage-planner/
├── current.yaml              # What's deployed NOW (the truth)
├── sessions/                 # Decision history (append-only)
│   └── 2026-01-30.yaml       # One file per decision session
├── catalog/                  # Hardware/software database (YAML)
│   ├── hardware.yaml         # Products with specs, tags, pros/cons (NO PRICES)
│   └── software.yaml         # Sync/backup tool definitions
├── examples/
│   └── topology.yaml         # Example topology
├── docs/                     # Documentation
├── src/storage_planner/      # Python source
└── tests/                    # Test suite
```

## Running Tests

```bash
source .venv/bin/activate
pytest                        # Run all tests
pytest --cov=storage_planner  # With coverage
```

## License

MIT
