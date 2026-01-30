# Storage Planner

CLI tool for modeling and analyzing storage/backup topologies. Pure function of YAML inputs.

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
| `topology.yaml` | User's storage setup (nodes, volumes, datasets, sync regimes) |
| `catalog/hardware.yaml` | Product specs and retail prices |
| `catalog/software.yaml` | Sync/backup tool characteristics |
| `catalog/market-prices.yaml` | Used market valuations |

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
├── system.yaml          # THE source of truth - user's current system
├── catalog/             # Hardware/software knowledge base
│   ├── hardware.yaml    # Product specs, pros/cons
│   ├── software.yaml    # Sync/backup tools
│   └── market-prices.yaml
├── proposals/           # "What-if" topologies to evaluate
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

1. **Read the system file** to understand current state:
   ```bash
   cat system.yaml
   ```
   - Which nodes are `active` vs `deprecated` vs `planned`
   - What problems are documented (system-level and per-node)
   - What constraints apply (noise, cost, redundancy requirements)
   - What decisions have been made and why

2. **Run analysis** on the current state:
   ```bash
   .venv/bin/sp analyze all system.yaml --json
   .venv/bin/sp simulate <deprecated-node> system.yaml --json
   .venv/bin/sp cost system.yaml -c catalog --json
   ```

3. **Research hardware options** from catalog:
   ```bash
   .venv/bin/sp catalog list -c catalog --tag <relevant-tag> --json
   .venv/bin/sp catalog compare <product1> <product2> -c catalog --json
   ```

4. **Reason about tradeoffs** based on:
   - User's documented constraints (in `system.yaml`)
   - Analysis results (redundancy gaps, capacity projections)
   - Hardware specs and prices from catalog
   - Node-specific problems that need solving

5. **Update system.yaml** when decisions are made:
   - Add new nodes with `status: planned`
   - Mark old nodes with `status: deprecated`
   - Record decisions in the `decisions` section
   - Update `problems` status when solved

**System evolution pattern:**
- `updated` field tracks when system.yaml was last accurate
- Problems: `active` → `solved` (with decision recorded)
- Nodes: `planned` → `active` → `deprecated`
- Git history provides full version control

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
