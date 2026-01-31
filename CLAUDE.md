# Storage Planner

CLI tool for **evaluating storage decisions** by modeling and analyzing storage/backup topologies. Pure function of YAML inputs.

## First Thing: Understand the Current Decision

Run these commands immediately to understand what we're working on:

```bash
# 1. What's deployed now?
cat current.yaml | head -20

# 2. What decision session is active?
ls sessions/
cat sessions/2026-01-30.yaml | head -60

# 3. Analyze current problems
.venv/bin/sp analyze all current.yaml 2>/dev/null || echo "CLI not installed - read YAML directly"
```

**Current situation (as of 2026-01-30):**
- Deployed: Synology DS224+ NAS with 2×8TB IronWolf HDDs
- Problem: NAS is too loud (32dB, limit is 30dB for home office)
- Goal: Replace with silent SSD-based storage
- Budget: ~$1000 hardware
- Session: `sessions/2026-01-30.yaml` has 5 options being evaluated

**Key questions to ask the user:**
1. "Which option are you leaning toward?" (review session options first)
2. "Are there any new constraints or requirements?"
3. "Have prices changed significantly?" (session prices from 2026-01-30)
4. "Ready to make a decision, or still exploring?"

## What This Tool Is For

This tool helps you make informed storage decisions by:
1. **Encoding requirements** - Document constraints, redundancy needs, noise limits, budgets
2. **Modeling topologies** - Capture current and proposed storage configurations
3. **Evaluating options** - Model "what-if" scenarios before buying hardware
4. **Running analysis** - Identify gaps in redundancy, capacity runway, RPO misses
5. **Tracking decisions** - Record what was decided and why

The primary workflow is: **understand current state → create session → model options → analyze → decide → update current**

## Core Concepts

### File Structure

```
storage-planner/
├── current.yaml          # What's deployed NOW (the truth)
├── sessions/             # Decision history (append-only)
│   └── 2026-01-30.yaml   # One file per decision session
├── catalog/              # Product reference (NO PRICES)
│   ├── hardware.yaml     # Product specs, pros/cons
│   └── software.yaml     # Sync/backup tools
├── archive/              # Old structure (for reference)
└── src/storage_planner/  # CLI implementation
```

### Key Rules

1. **One truth:** `current.yaml` = what's actually deployed
2. **Sessions:** Each decision point gets a dated session file
3. **Prices captured:** Embedded in session at decision time (not referenced)
4. **Append-only:** Never edit old sessions, create new ones
5. **Self-contained:** Each session has everything needed to understand it
6. **No prices in catalog:** Catalog has specs only; prices live in sessions

## Getting Started (New Session Workflow)

```bash
# 1. Understand current state
cat current.yaml                              # What's deployed now
.venv/bin/sp analyze all current.yaml         # Analyze current setup

# 2. Review existing sessions
ls sessions/                                  # Past decision sessions
cat sessions/2026-01-30.yaml                  # Active session with options

# 3. Run analysis on options
# Options are defined within the session file under 'options:'
# Analyze by extracting topology to temp file or using CLI

# 4. Make decision
# Update session file: decision.chosen, decision.rationale, decision.date

# 5. Update current.yaml
# Replace current.yaml with chosen option's topology
# Set from_session: "2026-01-30"
```

## File Formats

### `current.yaml` - What's Deployed

```yaml
name: "Synology NAS Setup"
deployed: "2024-01-01"
from_session: null  # or "2026-01-30" after a decision

nodes:
  - id: synology-ds224
    # ... full topology
```

### `sessions/2026-01-30.yaml` - A Decision Point

```yaml
created: "2026-01-30"
purpose: "Replace NAS with silent SSD storage"
status: active  # active | decided | abandoned

# Prices captured for this session
prices:
  captured: "2026-01-30"
  samsung-870-evo-4tb: { retail: 689, used_low: 289 }
  # ...

# Baseline: snapshot of current.yaml when session started
baseline:
  name: "Synology NAS Setup"
  # ...

# Options evaluated
options:
  sata:
    name: "SATA: 2x 870 EVO 4TB"
    hardware:
      - { product: owc-dual-mini, qty: 1, unit_price: 75 }
    total_cost: 715
    # ...

  nvme:
    name: "NVMe: 2x Lexar NM790 4TB"
    # ...

# Decision (filled in when decided)
decision:
  chosen: null  # "sata" or "nvme"
  rationale: null
  date: null
```

### `catalog/hardware.yaml` - Product Reference (NO PRICES)

```yaml
products:
  - id: samsung-870-evo-4tb
    name: "Samsung 870 EVO 4TB"
    category: ssd
    interface: SATA
    capacity: "4TB"
    specs:
      read_speed: "560MB/s"
      # ...
    # NO prices - those go in sessions
```

## Quick Reference

```bash
source .venv/bin/activate  # Always activate first

sp validate current.yaml               # Check config validity
sp analyze all current.yaml            # Full analysis
sp analyze redundancy current.yaml     # Redundancy only
sp analyze bandwidth current.yaml      # Bandwidth bottlenecks
sp analyze rpo-rto current.yaml        # RPO/RTO compliance
sp analyze capacity current.yaml       # Capacity projections
sp simulate <node|volume> current.yaml # Failure impact
sp catalog list -c catalog             # Browse hardware
sp catalog compare <id1> <id2> ...     # Compare products
```

## For Agents

**Before modifying this tool**, read:
- [docs/schema.md](docs/schema.md) - YAML schema reference
- [docs/analysis.md](docs/analysis.md) - How analysis algorithms work
- [docs/extending.md](docs/extending.md) - Adding features
- [docs/research-workflow.md](docs/research-workflow.md) - Populating the catalog

**Common tasks:**
- Add hardware product → edit `catalog/hardware.yaml` (no prices!)
- Research prices → capture in session file when needed
- Model new option → add to active session's `options:`
- Make decision → update session's `decision:` block, then `current.yaml`

## Agent Workflow for Storage Decisions

When helping with storage decisions:

1. **Understand current state:**
   ```bash
   cat current.yaml                           # What's deployed
   .venv/bin/sp analyze all current.yaml      # Analysis
   ls sessions/                               # Past decisions
   ```

2. **Check for active session:**
   ```bash
   cat sessions/2026-01-30.yaml               # Current decision session
   ```

3. **Research hardware options** from catalog:
   ```bash
   .venv/bin/sp catalog list -c catalog --tag <relevant-tag> --json
   .venv/bin/sp catalog compare <product1> <product2> -c catalog --json
   ```

4. **Capture prices in session:**
   - Look up current retail/used prices
   - Add to session's `prices:` block with capture date
   - Prices > 7 days old should be re-checked

5. **Model options in session:**
   - Add options under `options:` in session file
   - Each option has hardware list with prices referencing session's `prices:`

6. **Make and document decision:**
   - Fill in `decision.chosen`, `decision.rationale`, `decision.date`
   - Update `current.yaml` with chosen topology
   - Set `from_session: "YYYY-MM-DD"` in current.yaml

Always use `--json` output for structured data when parsing results.

## Project Structure

```
storage-planner/
├── current.yaml          # What's deployed NOW (the truth)
├── sessions/             # Decision history (append-only)
│   └── 2026-01-30.yaml   # Active: Replace NAS with SSD
├── catalog/              # Hardware/software knowledge base
│   ├── hardware.yaml     # Product specs (NO PRICES)
│   └── software.yaml     # Sync/backup tools
├── archive/              # Old structure preserved for reference
│   ├── state.yaml
│   ├── topologies/
│   ├── proposals/
│   └── decisions/
├── examples/             # Reference examples
└── src/storage_planner/
    ├── cli/              # Typer commands
    ├── models/           # Pydantic models
    ├── analysis/         # Pure analysis functions
    ├── loaders/          # YAML loading + validation
    └── output/           # Rich console formatting
```

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
