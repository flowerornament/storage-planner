# Storage Planner Design Exploration

> **Purpose**: This document captures the design exploration for `sp` (storage-planner), a CLI tool that helps AI agents make structured purchase decisions, particularly for storage hardware. This is reference material for continuing the design conversation.

---

## Table of Contents

1. [The Big Picture](#the-big-picture)
2. [How We Got Here](#how-we-got-here)
3. [The Core Problem](#the-core-problem)
4. [The Model We Designed](#the-model-we-designed)
5. [Simulated Usage](#simulated-usage)
6. [Analysis Capabilities](#analysis-capabilities)
7. [Key Design Decisions](#key-design-decisions)
8. [Open Questions](#open-questions)
9. [Technical Notes](#technical-notes)
10. [Reference Material](#reference-material)

---

## The Big Picture

### What Is This For?

`sp` is a CLI tool that serves as a **decision-support assistant for AI coding agents**. The immediate use case is storage hardware decisions (replacing a NAS, planning backup topologies), but the patterns could generalize to other purchase/configuration decisions.

### The Core Insight

AI agents are good at reasoning but bad at:
- **Remembering context across sessions** (context window limits, new sessions)
- **Tracking volatile data** (prices change, training data is stale)
- **Validating complex constraints** (does this topology meet redundancy requirements?)

The tool provides:
- **Structured state** that persists across sessions
- **Catalog of current prices/specs** that the agent can query
- **Analysis functions** that validate configurations against requirements
- **Decision tracking** so the agent knows where it left off

### The "Pure Function" Principle

The tool should be a **pure function of inputs**:
- No hidden state inside the tool
- All state lives in the database/files (versioned, explicit)
- Given the same inputs, you get the same outputs
- The agent manages conversation context; the tool manages persistent data

This separation of concerns is key. The tool is a calculator, not a brain.

---

## How We Got Here

### Starting Point

The project had an existing Rust CLI with:
- Items (products in a catalog)
- Prices (with history)
- Configurations (combinations of products)
- Decisions (comparing configurations)
- Events (audit log)

Plus archived YAML files showing a vision for topology modeling:
- Nodes (devices), volumes (storage), datasets (data with requirements)
- Sync regimes (how data moves between volumes)
- Links (network connections)

And a Python archive (removed in git history) with sophisticated analysis:
- Failure simulation
- Redundancy checking
- RPO/RTO validation
- Bandwidth analysis
- Capacity projection

### The Problem with the Old Approach

The old Python version modeled topologies as YAML files, but:
- YAML files needed manual maintenance (dates fell out of sync)
- No session management (starting a new Claude instance = lost context)
- The tool expected a human to keep it current

### Key Realization: Design for AI Agents

The user's insight: this is a **prototype for general decision-assistance tools for coding agents**.

What makes a tool agent-friendly?
- **Simple commands** — agent can remember and use correctly
- **Rich structured output** — provides context for agent to reason
- **Explicit state** — agent can read current state, know what's open
- **Session continuity** — `sp prime` tells agent where we left off

### The bd Inspiration

The `bd` (beads) issue tracker provided patterns:
- Hierarchy (epics contain issues)
- Dependencies (blocked_by)
- Status tracking
- JSONL sync for git
- `bd prime` for AI-optimized context loading

We considered wrapping bd, but decided to reimplement the simple parts we need:
- One tool, one database, one mental model
- Decision tracking is different from issue tracking (options, not tasks)
- Avoids external dependency and two-database complexity

---

## The Core Problem

### The Real Workflow

```
User: "I have a problem. My NAS is too loud and slow."

Agent: "Let me understand your current setup..." [reads topology]
Agent: "What are your constraints?" [captures requirements]
Agent: "Let me research options..." [queries catalog, web searches]
Agent: "Here are two options..." [builds configurations]
Agent: "Analyzing each..." [runs validation]
Agent: "I recommend X because..."

[Session ends]

[Next day, new session]

User: "Let's pick up where we left off"

Agent: [reads decision state] "We were comparing SATA vs NVMe..."
Agent: [continues]
```

### What the Tool Must Provide

1. **Catalog** — Products with specs, prices (current, not training data)
2. **Topology modeling** — Graph of devices, storage, data, sync regimes
3. **Analysis** — Validate topology against requirements
4. **Decision tracking** — Where are we, what's open, what's decided
5. **Session continuity** — `sp prime` gives agent full context

---

## The Model We Designed

### Three Core Entities

#### 1. Catalog

Products you can buy or use:
- Devices (Mac mini, Synology NAS)
- Drives (Samsung 870 EVO 4TB, Lexar NM790)
- Enclosures (OWC Express 4M2, OWC Mercury Dual)
- Software (Resilio Sync, borg, rsync)
- Services (Google Workspace 2TB — has monthly cost)

Prices:
- Observations over time (source, price, condition, date)
- Multiple sources (Amazon, eBay, r/hardwareswap)
- Enables price tracking and alerts

#### 2. Topologies (= Proposals)

A **versioned graph** modeling a storage system:

**Nodes** = Equipment instances
- Reference catalog products
- Have location (home-office, eu-datacenter)
- Contain volumes

**Volumes** = Storage attached to equipment
- Capacity, type (SSD, HDD, RAID, cloud)
- Can reference catalog product (the specific drive)

**Datasets** = Logical data groups
- Size, criticality (critical, important, replaceable)
- Requirements: required_copies, required_locations, max_rpo

**Sync Regimes** = How data moves (edges in the graph)
- Source volume → target volume(s)
- Method (reference to catalog: resilio, borg, rsync)
- Schedule (cron or "continuous")
- Direction (one-way, bidirectional)

**Versioning**:
- Each topology has `parent_id` for branching
- `status`: draft, exploring, approved, rejected
- Linked to the decision that created it

#### 3. Decisions (bd-like hierarchy)

A **tree of decisions, questions, and requirements**:

```
Decision: "Replace NAS" (root goal)
├── Requirement: "budget < $1000"
├── Requirement: "noise = 0dB"
├── Requirement: "capacity >= 8TB"
├── Question: "SATA vs NVMe?" → resolved: SATA
│   ├── [SATA branch] Question: "Which drives?" → open
│   └── [NVMe branch — pruned]
└── Links to: current topology, proposed topologies
```

**The coupling**: Decisions and topologies are linked:
- `decision.topology_id` — which topology this decision is about
- `decision.creates_topology_id` — resolving creates/selects a topology branch
- `topology.created_by_decision_id` — which decision created this version

**Session = reading the decision tree**:
```
Active topology: topo-v3-sata
Path: current → v2-sata → v3-sata-owc
Resolved: SATA vs NVMe → SATA, Enclosure → OWC
Open: Which drives?
```

### The Coupling: Decisions Create Topology Branches

When you resolve a decision, you commit to a topology path:

```
Decision "SATA vs NVMe"
  context: current-topology
  answer: SATA
  creates: topo-v2-sata

Decision "Which enclosure"
  context: topo-v2-sata
  answer: OWC Mercury
  creates: topo-v3-owc
```

The decision tree and topology tree are **isomorphic** — each decision branch corresponds to a topology branch.

---

## Simulated Usage

### Full Session Simulation

This simulation helped us validate the design:

```
SESSION 1: Problem Definition

$ sp prime
No active decisions.
Catalog: 67 products
Topologies: 1 (synology-nas, current)

$ sp decide "Replace NAS - too loud, too slow"
Created decision: dec-001 "Replace NAS"
Context topology: synology-nas

$ sp require "budget < 1000" "noise = 0" "capacity >= 8TB"
Added 3 requirements to dec-001

$ sp question "SATA vs NVMe?"
Added question q-001 to dec-001

[Agent researches via catalog and web]

$ sp catalog query "4tb sata ssd"
samsung-870-evo-4tb    $220 (eBay used)
crucial-mx500-4tb      $165 (r/hardwareswap)
samsung-pm893-3840gb   $240 (datacenter pull)

$ sp catalog query "4tb nvme"
lexar-nm790-4tb        $219 (Amazon)
teamgroup-mp44-4tb     $239 (Amazon)
samsung-990-pro-4tb    $299 (eBay/new)

[Agent builds two proposals]

$ sp propose sata --from=synology-nas \
    --remove=synology-ds224 \
    --add-volume=mac-mini:sata-storage \
    --products=owc-mercury-dual,samsung-870-evo-4tb:2
Created topology: topo-002-sata

$ sp propose nvme --from=synology-nas \
    --remove=synology-ds224 \
    --add-volume=mac-mini:nvme-storage \
    --products=owc-express-4m2,lexar-nm790-4tb:2
Created topology: topo-003-nvme

$ sp evaluate topo-002-sata
✓ Redundancy: all datasets compliant
✓ RPO: all sync regimes meet requirements
✓ Budget: $649 < $1000
✓ Noise: 0dB

$ sp evaluate topo-003-nvme
✓ Redundancy: all datasets compliant
✓ RPO: all sync regimes meet requirements
✓ Budget: $699 < $1000 (but less headroom)
✓ Noise: 0dB

$ sp compare topo-002-sata topo-003-nvme
                    SATA        NVMe
Cost                $649        $699
Speed               560 MB/s    3200 MB/s
Expansion           None        +16TB
Warranty            Used        5 years

Both pass requirements. SATA cheaper, NVMe faster + expandable.

[Session ends]

---

SESSION 2: Next Day

$ sp prime
Decision: dec-001 "Replace NAS" [in-progress]
  Context: synology-nas → comparing topo-002-sata, topo-003-nvme
  Requirements: budget ✓, noise ✓, capacity ✓
  Open: "SATA vs NVMe?"

Catalog: 67 products (prices 1 day old)

[Agent has full context, continues]

User: "Speed doesn't matter for backup workload. Go with SATA."

$ sp resolve q-001 "SATA" --rationale="Speed irrelevant for backup, save $50"
Resolved: SATA vs NVMe → SATA
Active topology: topo-002-sata
Pruned: topo-003-nvme (marked rejected)

$ sp question "Which SATA drives?"
Added question q-002 to dec-001

[More iteration...]

$ sp approve
Decision dec-001 approved.
Final topology: topo-002-sata ($649)

$ sp export purchase-list
OWC Mercury Elite Pro Dual Mini  $119  macsales.com
Samsung 870 EVO 4TB × 2          $530  eBay

Total: $649
```

### Key Observations from Simulation

1. **`sp prime` is the session recovery mechanism** — shows everything needed to continue
2. **Commands are simple** — propose, evaluate, compare, resolve, approve
3. **Branching is natural** — propose creates branches, resolve prunes alternatives
4. **Catalog queries are essential** — finding products with current prices
5. **Evaluation validates constraints** — catches violations automatically

---

## Analysis Capabilities

### From the Old Python Version

The Python archive had these analyses (worth reimplementing):

#### Failure Simulation
```python
simulate_node_failure(topology, "mac-mini-m4")
→ Which datasets lose copies?
→ Which are still recoverable?
→ Is there data loss risk?
```

#### Redundancy Analysis
```python
analyze_redundancy(topology)
→ For each dataset:
   - required_copies vs actual_copies
   - required_locations vs actual_locations
   - Pass/fail per dataset
```

#### RPO/RTO Analysis
```python
analyze_rpo_rto(topology)
→ For each dataset:
   - max_rpo (required) vs achieved_rpo (from sync regimes)
   - Estimated from cron schedule if not explicit
   - Pass/fail per dataset
```

#### Bandwidth Analysis
```python
analyze_bandwidth(topology)
→ Widest path algorithm (maximize minimum bandwidth)
→ Transfer time estimates for sync regimes
→ Bottleneck identification
```

#### Capacity Projection
```python
project_capacity(topology, months=12)
→ Growth rate per dataset
→ Months until full per volume
→ Warnings for volumes at risk
```

### Design Principle: Explicit Over Implicit

The Python version enforced: **all assumptions must be explicit**.

If constraints aren't set, validation fails with helpful error:
```
ERROR: Dataset 'photos-archive' is critical but
constraints.min_critical_data_copies is not set.
Add this to your topology's constraints section.
```

This forces completeness and prevents silent failures.

---

## Key Design Decisions

### 1. Topologies ARE Proposals

Not "current topology" vs "proposed topology" — topologies are versioned proposals. The current system is just v1. New proposals branch from it.

**Why**: Eliminates duplication. You don't maintain separate "current.yaml" and "proposed.yaml" with identical nodes.

### 2. Configuration = Delta + Products

A proposal is:
- Start from parent topology
- Remove these nodes/volumes
- Add these nodes/volumes (referencing catalog products)
- Move these datasets

The tool computes the full proposed topology by applying the delta.

**Why**: Avoids maintaining multiple full topology files. Changes are explicit.

### 3. Decisions and Topologies Are Coupled in Schema

Not just notes — actual foreign keys:
- `topology.created_by_decision_id`
- `decision.creates_topology_id`

**Why**: Session reconstruction depends on understanding "this topology exists because of this decision."

### 4. bd-like Hierarchy for Decisions

Decisions contain questions and requirements as children. This provides:
- Natural hierarchy (goal → questions → sub-questions)
- Session management (what's open, what's resolved)
- Branching (different answers lead to different paths)

**Why**: bd's patterns work well for AI agents. Reuse the mental model.

### 5. Simple Commands, Rich Output

Agent shouldn't need to memorize complex syntax. Commands map to operations on the model:
- `sp decide` — create decision
- `sp propose` — create topology branch
- `sp evaluate` — run analysis
- `sp resolve` — answer question
- `sp prime` — show context

**Why**: Agents forget complex syntax. Simple interface, let them reason.

### 6. Catalog Includes Software and Services

Not just hardware. Sync methods (resilio, borg) and services (Google Workspace with monthly cost) are catalog items.

**Why**: Edges in the topology graph (sync regimes) reference catalog items. Services have costs that factor into analysis.

---

## Open Questions

### 1. Schema Details for Topology Graph

How exactly do we store the graph? Options:
- **Normalized tables**: nodes, volumes, datasets, sync_regimes as separate tables
- **JSON blob**: topology.data is a JSON document
- **Hybrid**: core structure in tables, flexible config in JSON

Trade-off: queryability vs flexibility.

### 2. How Are Topologies Created/Edited?

The `sp propose` command — what's the UX?
- From YAML file? (import)
- Incrementally via CLI? (add-node, add-volume, etc.)
- Agent generates YAML, tool validates/imports?

Current YAML files in archive/ are detailed. Does the tool generate these, or expect them as input?

### 3. Sync Regime Modeling

Sync regimes are edges in the graph, but they connect volumes, not nodes. Schema:
- `source_volume_id` → `target_volume_id`
- For bidirectional (Resilio), is it one edge or two?
- How do we model "continuous" vs "scheduled"?

### 4. Price Freshness and Catalog Updates

Prices go stale. How do we:
- Track when prices were last checked?
- Alert when prices are old?
- Refresh from sources (API? manual? web scrape?)

Current code has Best Buy and eBay API integrations. How do these fit?

### 5. What's the Migration Path from Current Code?

Existing Rust code has:
- Items, Prices, Configurations, Decisions, Events tables
- CLI commands for these

How do we evolve to the new model?
- Add topology tables alongside existing?
- Migrate configurations → topologies?
- Keep backward compatibility?

### 6. Versioning and Git Sync

bd uses JSONL for git-friendly sync. Do we:
- Same pattern for topologies?
- Just commit the SQLite database?
- Hybrid (database + JSONL exports)?

### 7. How Much Analysis to Automate?

The agent can reason about "if this node fails, what happens." Do we need automated failure simulation, or is that over-engineering?

Arguments for automation:
- Agent might miss edge cases
- Consistent, repeatable validation
- Shows work (here's why this fails)

Arguments against:
- Added complexity
- Agent is smart enough
- Start simple, add later

### 8. The Proposal Workflow in Detail

When agent runs `sp propose sata --remove=nas --add-volume=...`:
- Does it create the topology immediately?
- Or is there a preview/confirm step?
- How are datasets moved to new volumes?

The delta application needs careful design.

---

## Technical Notes

### From Python Archive (git show 79b2ea5^:python-archive/...)

**Models** (pydantic-based):
- Topology contains: nodes, links, datasets, sync_regimes, constraints
- Node contains: volumes, location, power profile, noise
- Volume: capacity, type, raid level, hosts_datasets
- Dataset: criticality, required_copies, max_rpo, stored_on
- SyncRegime: source_volume, target_volumes, method, schedule

**Analysis modules**:
- `failure_sim.py` — node/volume failure impact
- `redundancy.py` — copy/location requirements
- `rpo_rto.py` — recovery point objectives
- `bandwidth.py` — widest path algorithm for transfer times
- `capacity.py` — growth projection
- `completeness.py` — validate all required fields are set

**Utilities**:
- Parse sizes: "8TB", "500GB" → bytes
- Parse durations: "1h", "30m", "7d" → seconds
- Parse bandwidth: "10Gbps", "500MB/s" → bytes/sec
- Cron parsing for RPO estimation

### Current Rust Code Structure

```
src/
├── main.rs               # CLI entry point
├── core/                 # Models, database, events
│   ├── models.rs         # Item, Price, Configuration, Decision
│   ├── db.rs             # SQLite operations
│   └── events.rs         # Event logging
├── cli/                  # Command implementations
│   ├── item.rs           # sp item add/list/show
│   ├── price.rs          # sp price add/list
│   ├── config.rs         # sp config create/add-item
│   └── ...
├── domains/storage/      # Storage-specific analysis
└── pricing/              # API integrations (Best Buy, eBay)
```

### Database Schema (Current)

```sql
items (id, name, specs, category, url, created_at)
prices (id, item_id, price, source, condition, observed_at)
configurations (id, name, domain, metadata, created_at)
configuration_items (id, configuration_id, item_id, quantity, role)
decisions (id, purpose, options, chosen_option, rationale, status, ...)
events (id, event_type, entity_id, actor, timestamp, data)
```

---

## Reference Material

### The User's Setup (from archive/topologies/)

**Current system (synology-nas.yaml)**:
- MacBook Pro M4 (laptop, 1TB internal, 2TB external Lexar)
- Synology DS224+ (2×8TB IronWolf RAID1) — **THE PROBLEM**: 32dB, too loud
- Mac mini M4 (always-on hub, runs services)
- EU server (Seedhost, 2TB NVMe + 40TB HDD array)
- Google Drive (2TB cloud backup)

**Datasets**:
- working-files (50GB, critical, 3 copies, 1h RPO)
- source-code (26GB, critical, 3 copies, 1h RPO)
- photos-archive (900GB, critical, 3 copies, 24h RPO)
- media-library (371GB, important, 2 copies, 7d RPO)
- hot-storage-sync (1.19TB, important, 3 copies, 1h RPO)
- project-archives (716GB, important, 2 copies, 30d RPO)
- time-machine-backups (827GB, important, 1 copy, 24h RPO)

**The problem**: NAS is 32dB (limit is 30dB), slow ARM CPU, HyperBackup to cloud is painfully slow.

**Proposed solution**: Replace NAS with external SSD on Mac mini. Options:
- SATA: OWC Mercury Dual + 2× Samsung 870 EVO 4TB = ~$649
- NVMe: OWC Express 4M2 + 2× Lexar NM790 4TB = ~$699

### Market Context (from archive/market-prices.yaml)

67 products with price observations:
- Enterprise SATA SSDs (datacenter pulls) are $/TB sweet spot
- 8TB single-drive portables are overpriced vs 2×4TB
- SSD prices volatile (NAND shortage, Jan 2026)
- Used drives save money but no warranty

### The bd Pattern

From `bd --help`, key patterns we borrowed:
- `bd prime` — AI-optimized context dump
- `bd ready` — what's actionable (no blockers)
- `bd show` — full details on one item
- `bd sync` — JSONL export for git
- Hierarchy via parent_id
- Dependencies via blocked_by
- Status workflow: open → in_progress → closed

---

## Next Steps

1. **Validate this model** — Does it cover all use cases?
2. **Design the schema** — Detailed table structures
3. **Design the commands** — Exact CLI interface
4. **Prototype** — Build minimal version to test
5. **Port analysis** — Bring over Python analysis functions
6. **Iterate** — Use it for real decision, refine

---

*Generated: 2026-02-01*
*Context: Design exploration session for storage-planner v2*
