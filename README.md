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
# Validate a topology
sp validate examples/topology.yaml

# Run full analysis
sp analyze all examples/topology.yaml

# Run quick summaries (redundancy, RPO/RTO, capacity)
sp analyze quick examples/topology.yaml

# Compare analysis between two topologies
sp analyze diff topologies/a.yaml topologies/b.yaml

# Simulate a failure
sp simulate macbook-m4 examples/topology.yaml

# Compare failure impact between two topologies
sp simulate diff macbook-m4 topologies/a.yaml topologies/b.yaml

# Browse hardware catalog
sp catalog summary -c catalog
sp catalog list --use-case time-machine-target -c catalog

# Compare products
sp catalog compare samsung-870-qvo-4tb crucial-mx500-4tb -c catalog

# JSON output for agents/tools
sp analyze redundancy examples/topology.yaml --json
```

## Documentation

- **[AGENTS.md](AGENTS.md)** - Quick reference for agents/users
- **[docs/schema.md](docs/schema.md)** - YAML schema reference
- **[docs/analysis.md](docs/analysis.md)** - How analysis works
- **[docs/cli.md](docs/cli.md)** - CLI reference & JSON output
- **[docs/extending.md](docs/extending.md)** - Adding features
- **[docs/research-workflow.md](docs/research-workflow.md)** - Populating the catalog

## Project Structure

```
storage-planner/
├── catalog/                  # Hardware/software database (YAML)
│   ├── hardware.yaml         # Products with specs, tags, pros/cons
│   ├── software.yaml         # Sync/backup tool definitions
│   └── market-prices.yaml    # Used market valuations
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
