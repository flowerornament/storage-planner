# Feature Landscape

**Domain:** CLI tools for system modeling and decision support
**Researched:** 2026-02-06
**Confidence:** MEDIUM (synthesized from IaC patterns, storage tools, decision tools, and existing codebase)

## Table Stakes

Features users expect. Missing = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **State persistence** | Core value prop; session continuity | Low | Already have SQLite; proven pattern |
| **Graph modeling (nodes, volumes)** | IaC tools universally use DAGs; topology is inherently a graph | Medium | Terraform, Pulumi all use resource graphs |
| **Dry-run / preview mode** | `terraform plan` pattern universal in IaC; must preview before mutation | Low | "What would this change?" before applying |
| **CRUD for all entities** | Basic usability; must add/edit/delete/list items | Low | Already have for items, prices, configs |
| **Structured output (JSON/YAML)** | Agent consumption; machine-readable output | Low | Already have `--format` flag |
| **Help text / discoverability** | `--help` on every command; agents need to learn commands | Low | Already have; maintain quality |
| **Validation before mutation** | Catch errors early; referential integrity | Medium | Verify config_id exists before add-option |
| **Audit trail / event log** | Know what changed and when; debugging | Low | Already have events table |
| **Import/export for portability** | Git sync, backup, sharing | Medium | `sp sync` exists; may need topology export |
| **Context dump (`sp prime`)** | Session recovery for agents; the bd pattern | Low | Already conceptualized; critical for agent UX |

**Source confidence:** HIGH for IaC patterns (Terraform/Pulumi official docs), MEDIUM for decision tools (synthesized from ADR patterns).

## Differentiators

Features that set `sp` apart. Not expected by users of generic tools, but valuable for this domain.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Decision tracking with rationale** | Why X over Y; bd-like workflow | Medium | Beyond simple CRUD; captures reasoning |
| **Topology versioning / branching** | Fork to explore alternatives; git-like | High | Terraform uses linear state; this goes beyond |
| **Redundancy analysis** | Validates data protection requirements | Medium | Unique to storage domain; already started |
| **Failure simulation** | "What if node X dies?" | High | Requires graph traversal, copy counting |
| **RPO/RTO compliance checking** | Validates sync schedules against requirements | Medium | Storage-specific; high value for data protection |
| **Capacity projection** | "When will I run out of space?" | Low | Simple growth rate math; high UX value |
| **Cost analysis (one-time + TCO)** | Total cost of ownership across options | Medium | Services have recurring costs; often missed |
| **Constraint checking** | Budget, noise, location requirements | Medium | Decision-level constraints vs data requirements |
| **Price freshness tracking** | Alert when prices are stale | Low | "Price last checked 30d ago" |
| **Catalog queries** | Find products by criteria | Medium | Essential for agent research workflow |
| **Comparison view (`sp decide compare`)** | Side-by-side option analysis | Low | Already implemented; table format |
| **Sync regime modeling** | How data moves between volumes | Medium | Edge in the topology graph |
| **Bandwidth analysis** | Can links support sync regimes? | High | Widest path algorithm; network modeling |

**Source confidence:** MEDIUM (derived from Python archive analysis, IaC patterns, storage management patterns).

## Anti-Features

Features to explicitly NOT build. Common mistakes in this domain.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Interactive mode / wizard** | Agents prefer scriptable commands over interactive prompts | Compose simple commands; use `--help` liberally |
| **Real-time sync daemon** | Tool should be stateless between commands; complexity | Keep tool invocation-based; external schedulers |
| **Multi-user / collaboration** | Adds auth, conflict resolution; overkill for single-user | Single-user local tool; share via git |
| **GUI / web interface** | Wrong interface for agent consumption | CLI-first; structured JSON output |
| **Automatic purchasing** | Liability, complexity, user trust | Provide recommendations; human executes |
| **Hidden state** | Agents can't reason about what they can't see | All state in database; explicit outputs |
| **Complex command syntax** | Agents forget; discoverability suffers | Simple subcommands: `sp node add`, not `sp --add-node --type=x` |
| **Undo stack** | Complexity; append-only is simpler | Versioned topologies; create new version instead of undo |
| **Implicit defaults** | Agent doesn't know what was assumed | Require explicit input or fail with helpful message |
| **Vendor lock-in for analysis** | Keep tool generic; storage is first domain, not only | Domain modules (storage/, networking/) not hard-coded |
| **Over-engineered decision formalism** | Agent is smart; doesn't need QOC, IBIS frameworks | Simple hierarchy: goal -> questions -> requirements |

**Source confidence:** HIGH (learned from design exploration, DESIGN-EXPLORATION.md "Meta-Observations").

## Feature Dependencies

```
Foundation (Phase 1)
  |
  v
sp prime <-- requires --> all CRUD commands
  |
  v
Topology CRUD (Phase 2)
  Node add/list/show
  Volume add/list/show  <-- requires --> Node exists
  Dataset add/list/show
  Sync regime add       <-- requires --> Volume exists (source + target)
  |
  v
Topology Versioning (Phase 2b)
  Fork topology         <-- requires --> Topology exists
  Tag topology (current/exploring/archived)
  |
  v
Analysis (Phase 3)
  Redundancy analysis   <-- requires --> Datasets + Volumes
  Capacity projection   <-- requires --> Volumes + Datasets
  RPO compliance        <-- requires --> Sync regimes + Datasets
  Failure simulation    <-- requires --> Full topology graph
  |
  v
Decision Tracking (Phase 4)
  Create decision
  Add topology to decision <-- requires --> Topology exists
  Run analysis          <-- requires --> Analysis functions
  Compare options       <-- requires --> Multiple topologies
  Choose + rationale
  |
  v
Integration (Phase 5)
  Link catalog items to volumes <-- requires --> Catalog + Topology
  Cost analysis         <-- requires --> Prices + Topology
  Export purchase list  <-- requires --> Decision + Catalog
```

## MVP Recommendation

For MVP, prioritize:

1. **`sp prime` context dump** - Core session recovery; enables agent workflow (table stakes)
2. **Topology CRUD** - Nodes, volumes, datasets as separate tables (table stakes)
3. **Basic redundancy analysis** - Count copies, identify unprotected datasets (differentiator)
4. **Decision tracking** - bd-like create/choose/close lifecycle (differentiator)

**Why this order:**
- `sp prime` is the agent's entry point every session
- Topology must exist before analysis can run
- Redundancy analysis is the simplest valuable analysis
- Decision tracking ties it together

Defer to post-MVP:
- **Topology versioning/branching**: Adds complexity; linear versions work initially
- **Failure simulation**: Requires complete graph model; add after basics work
- **Bandwidth analysis**: Requires network link modeling; lower priority than data protection
- **Price freshness alerts**: Nice-to-have; manual tracking sufficient initially

## Complexity Estimates

| Feature | Complexity | Reason |
|---------|------------|--------|
| sp prime | Low | Query and format existing data |
| Node/Volume/Dataset CRUD | Medium | New tables, CLI commands, referential integrity |
| Sync regime CRUD | Medium | Edge relationships between volumes |
| Topology versioning | High | Parent/child relationships, tag system, diff |
| Redundancy analysis | Medium | Count copies across volumes; mostly done |
| Capacity projection | Low | Simple growth rate calculation |
| RPO compliance | Medium | Parse cron schedules, compare to requirements |
| Failure simulation | High | Graph traversal, impact analysis |
| Bandwidth analysis | High | Widest path algorithm, network modeling |
| Decision-topology linking | Medium | Foreign keys, staleness detection |
| Export purchase list | Low | Query decided config, format output |

## Sources

### IaC Graph Modeling Patterns
- [Terraform Graph Command](https://developer.hashicorp.com/terraform/cli/commands/graph) - Official docs on DAG generation
- [Pulumi State and Backends](https://www.pulumi.com/docs/iac/concepts/state-and-backends/) - Resource graph and checkpoint concepts
- [Stategraph](https://stategraph.com/) - Database-backed dependency graphs for IaC
- [petgraph](https://docs.rs/petgraph/) - Rust graph library with DOT export

### Decision Record Patterns
- [adr-tools](https://github.com/npryce/adr-tools) - CLI for Architecture Decision Records
- [ADR GitHub org](https://adr.github.io/) - ADR tooling ecosystem

### Terraform Plan/Preview Patterns
- [Terraform Plan Command](https://developer.hashicorp.com/terraform/cli/commands/plan) - Dry-run pattern
- [Terraform Dry Run Explained](https://spacelift.io/blog/terraform-dry-run) - Pattern analysis

### Storage Management
- [ZFS vs Btrfs Architecture](https://klarasystems.com/articles/zfs-vs-btrfs-architects-features-and-stability-2/) - Storage topology patterns
- [RAID Calculator Tools](https://www.gigacalculator.com/calculators/raid-calculator.php) - Capacity/redundancy calculations

### Undo/Redo Patterns
- [Command Pattern for Undo](https://www.esveo.com/en/blog/undo-redo-and-the-command-pattern/) - Why we chose NOT to implement undo stack
- [Redux Undo History](https://redux.js.org/usage/implementing-undo-history) - State snapshot approach

### RPO/RTO Concepts
- [RTO vs RPO](https://www.veeam.com/blog/recovery-time-recovery-point-objectives.html) - Disaster recovery metrics
- [RPO/RTO on AWS](https://blog.thecloudengineers.com/p/rto-vs-rpo-designing-practical-dr) - Architectural constraints

### Existing Codebase (HIGH confidence)
- `/Users/morgan/code/storage-planner/.planning/DESIGN-EXPLORATION.md` - Detailed design decisions
- `/Users/morgan/code/storage-planner/.planning/PROJECT.md` - Requirements and constraints
- `/Users/morgan/code/storage-planner/src/domains/storage/analysis.rs` - Existing analysis code
- `/Users/morgan/code/storage-planner/src/cli/decide.rs` - Existing decision workflow
