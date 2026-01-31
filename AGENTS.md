# Storage Planner

CLI tool for **evaluating storage decisions** by modeling and analyzing storage/backup topologies. Pure function of YAML inputs.

## What This Tool Is For

This tool helps you make informed storage decisions by:
1. **Encoding requirements** - Document constraints, redundancy needs, noise limits, budgets
2. **Modeling topologies** - Capture current and proposed storage configurations
3. **Evaluating options** - Model "what-if" scenarios before buying hardware
4. **Running analysis** - Identify gaps in redundancy, capacity runway, RPO misses
5. **Tracking decisions** - Record what was decided and why

The primary workflow is: **understand current state → model options → analyze → decide → document**

## Getting Started (New Session Workflow)

```bash
# 1. Understand current state
cat state.yaml                                    # Which topology is active?
cat topologies/$(cat state.yaml | grep active_topology | cut -d: -f2 | tr -d ' ').yaml
.venv/bin/sp analyze all topologies/synology-nas.yaml  # Analyze active topology

# 2. Review existing options
ls topologies/                                    # See available topologies
cat decisions/                                    # See past decisions

# 3. Model new options (if needed)
# Create topologies/<name>.yaml with status: proposed

# 4. Run analysis on options
.venv/bin/sp validate topologies/mac-mini-hub-nvme.yaml
.venv/bin/sp analyze all topologies/mac-mini-hub-nvme.yaml --json
.venv/bin/sp cost topologies/mac-mini-hub-nvme.yaml -c catalog

# 5. Compare options and decide
# Update decisions/<date>-<topic>.md with analysis and choice

# 6. Update state.yaml when deploying
# active_topology: <new-topology-name>
# Add entry to history
```

## Quick Reference

```bash
source .venv/bin/activate  # Always activate first

sp validate <topology.yaml>                # Check config validity
sp analyze all <topology.yaml>             # Full analysis
sp analyze redundancy <topology.yaml>      # Redundancy only
sp analyze bandwidth <topology.yaml>       # Bandwidth bottlenecks
sp analyze rpo-rto <topology.yaml>         # RPO/RTO compliance
sp analyze capacity <topology.yaml>        # Capacity projections
sp cost <topology.yaml> -c catalog         # Cost breakdown
sp simulate <node|volume> <topology>       # Failure impact
sp catalog list -c catalog                 # Browse hardware
sp catalog compare <id1> <id2> ...         # Compare products
sp suggest software <topology> -c catalog  # Get recommendations
```

## Architecture

All state lives in YAML files. The tool reads configs → runs analysis → outputs results.

| File | Purpose |
|------|---------|
| `topologies/*.yaml` | Storage configurations (current, proposed, alternatives) |
| `state.yaml` | Tracks which topology is currently deployed |
| `decisions/*.md` | Decision records with rationale and analysis |
| `catalog/hardware.yaml` | Product specs and retail prices |
| `catalog/software.yaml` | Sync/backup tool characteristics |
| `catalog/market-prices.yaml` | Used market valuations |

**Topology lifecycle:**
- `status: active` - Currently deployed configuration
- `status: proposed` - Under evaluation, not yet deployed
- `status: deprecated` - Superseded by another topology
- `supersedes: <topology-name>` - Links to the topology this replaces

## For Agents

**Before modifying this tool**, read:
- [docs/schema.md](docs/schema.md) - YAML schema reference
- [docs/analysis.md](docs/analysis.md) - How analysis algorithms work
- [docs/extending.md](docs/extending.md) - Adding features
- [docs/research-workflow.md](docs/research-workflow.md) - Populating the catalog

**Common tasks:**
- Add hardware product → edit `catalog/hardware.yaml` (see research-workflow.md)
- Add sync software → edit `catalog/software.yaml`
- Update used prices → edit `catalog/market-prices.yaml`
- Model new topology → create new YAML following schema

**Catalog research workflow:**
1. `sp catalog summary -c catalog` - See what's cached
2. Research products for a need (web search, reviews)
3. Add to `catalog/hardware.yaml` with tags, use_cases, pros/cons
4. Update `catalog/market-prices.yaml` with used prices
5. `sp suggest hardware` now uses cached data - no web searches

## Project Structure

```
storage-planner/
├── state.yaml           # Which topology is currently deployed
├── topologies/          # All storage configurations
│   ├── synology-nas.yaml         # Current active topology
│   ├── mac-mini-hub-sata.yaml    # Proposed: SATA option
│   └── mac-mini-hub-nvme.yaml    # Proposed: NVMe option
├── decisions/           # Decision records with rationale
│   └── 2026-01-local-storage.md
├── catalog/             # Hardware/software knowledge base
│   ├── hardware.yaml    # Product specs, pros/cons
│   ├── software.yaml    # Sync/backup tools
│   └── market-prices.yaml
├── examples/            # Reference examples
└── src/storage_planner/
    ├── cli/          # Typer commands
    ├── models/       # Pydantic models
    ├── analysis/     # Pure analysis functions
    ├── loaders/      # YAML loading + validation
    └── output/       # Rich console formatting
```

## Agent Workflow for Storage Decisions

When helping with storage decisions:

1. **Understand current state:**
   ```bash
   cat state.yaml                              # Which topology is deployed?
   cat topologies/synology-nas.yaml            # Read the active topology
   ls decisions/                               # What's been decided?
   ```

2. **Run analysis** on topologies:
   ```bash
   .venv/bin/sp analyze all topologies/synology-nas.yaml --json
   .venv/bin/sp simulate <node-id> topologies/synology-nas.yaml --json
   .venv/bin/sp cost topologies/synology-nas.yaml -c catalog --json
   ```

3. **Research hardware options** from catalog:
   ```bash
   .venv/bin/sp catalog list -c catalog --tag <relevant-tag> --json
   .venv/bin/sp catalog compare <product1> <product2> -c catalog --json
   ```

4. **Create/update topology files** to model options:
   - Each option gets its own file in `topologies/`
   - Include `hardware_cost:` section with pricing breakdown
   - Set `status: proposed` and `supersedes: <current-topology>`

5. **Document decisions** in `decisions/`:
   - Create `decisions/<date>-<topic>.md`
   - Include context, options evaluated, analysis, and final choice
   - Link to relevant topology files

6. **Update state.yaml** when deploying:
   - Change `active_topology:` to new topology name
   - Add entry to `history:` with date and notes

**File naming:**
- Topologies are named by what they ARE (e.g., `mac-mini-hub-nvme.yaml`)
- NOT by temporal status (avoid `current.yaml`, `target.yaml`)
- This allows multiple options to coexist and evolve over time

Always use `--json` output for structured data when parsing results.

## Development

```bash
uv venv && source .venv/bin/activate
uv pip install -e ".[dev]"
pytest
```

**Note for agents:** In non-interactive shells, `source .venv/bin/activate` may not put `sp` on PATH. Use `.venv/bin/sp` or `.venv/bin/python -m pytest` directly instead.

**Environment:** Python is installed via nix. See `~/.nix-config` for system configuration.

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd sync
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
