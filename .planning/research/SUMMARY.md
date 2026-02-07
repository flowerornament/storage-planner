# Project Research Summary

**Project:** Storage Planner (sp) - CLI for AI-assisted purchase decisions
**Domain:** Rust CLI with graph-based topology modeling and SQLite persistence
**Researched:** 2026-02-06
**Confidence:** MEDIUM-HIGH

## Executive Summary

Storage Planner extends an existing Rust CLI tool to add topology modeling capabilities for storage infrastructure. The codebase already has solid foundations (SQLite database, CLI patterns, decision tracking), and research confirms these choices align with domain best practices. The key addition is graph-based topology modeling using a hybrid SQL + petgraph approach.

The recommended path is to extend the existing modular architecture with normalized topology tables (nodes, volumes, datasets, sync regimes) while leveraging petgraph for in-memory analysis. Use a hybrid storage pattern: normalized edge tables for relationships and queries, JSON columns for flexible metadata, and denormalized snapshots for complex graph traversals. This avoids the performance pitfalls of pure recursive CTEs while maintaining the "database is truth" principle.

Critical risks include recursive CTE performance degradation with deep graphs, flat CLI architecture limiting global options, and topology branching without structural sharing. Mitigate by using hybrid storage from day one, establishing proper CLI scaffolding upfront, and accepting full-copy forking for MVP with documented plans for optimization. The research provides high confidence in stack choices and architectural patterns, with moderate confidence in optimal branching approaches that can be validated during implementation.

## Key Findings

### Recommended Stack

The existing stack is solid and should be maintained. Three targeted additions enable topology modeling without disrupting proven patterns.

**Core technologies to add:**
- **petgraph 0.8.3** (with `serde-1` feature): Graph algorithms (BFS, DFS, pathfinding). Use `StableGraph` to maintain index stability across removals. Essential for redundancy analysis, failure simulation, and bandwidth calculations.
- **tabled 0.20.0** (with `derive` feature): Pretty-print tables for human-readable CLI output. Complements existing `console` crate for simple styling and `serde_json` for machine-readable output.
- **garde 0.22.1** (with `derive` feature): Constraint validation for topology rules (RAID configurations, RPO requirements, capacity constraints). Cleaner API than `validator` crate.

**Keep existing stack:**
- clap 4, rusqlite 0.31+, serde/serde_json, chrono, uuid, anyhow, console, ureq

**Version upgrades:**
- rusqlite 0.31 → 0.38.0 (medium priority, test for breaking changes)

**Reject:**
- diesel/sqlx (async overhead unnecessary for single-user CLI)
- tokio/async-std (synchronous CLI pattern works)
- graphlite/cqlite (embedded graph DBs add query language complexity)

### Expected Features

Research identified clear table stakes vs differentiators based on IaC patterns (Terraform, Pulumi), decision tools (ADR), and storage management practices.

**Must have (table stakes):**
- Graph modeling (nodes, volumes, datasets as graph) - Universal in IaC tools
- Dry-run/preview mode - `terraform plan` pattern expected
- CRUD for all entities - Basic usability
- Structured output (JSON/YAML) - Agent consumption requirement
- Validation before mutation - Catch errors early
- Audit trail/event log - Already implemented
- Context dump (`sp prime`) - Session recovery for agents

**Should have (competitive differentiators):**
- Decision tracking with rationale - bd-like workflow
- Topology versioning/branching - Fork to explore alternatives
- Redundancy analysis - Validates data protection requirements
- Failure simulation - "What if node X dies?"
- RPO/RTO compliance checking - Storage-specific validation
- Capacity projection - Growth rate forecasting
- Cost analysis (TCO) - One-time + recurring costs
- Sync regime modeling - How data moves between volumes

**Defer (v2+):**
- Bandwidth analysis - Requires full network link modeling
- Price freshness alerts - Nice-to-have, manual sufficient for MVP
- Complex topology versioning - Delta-based storage can wait

**Explicitly avoid (anti-features):**
- Interactive mode/wizard - Agents prefer scriptable commands
- Real-time sync daemon - Keep tool invocation-based
- Multi-user collaboration - Single-user local tool
- GUI/web interface - CLI-first for agent consumption
- Automatic purchasing - Liability and trust issues
- Complex undo stack - Versioned topologies replace undo

### Architecture Approach

Extend the existing modular architecture (core/, cli/, domains/, pricing/) rather than restructuring. The current codebase demonstrates clean separation of concerns.

**Major components to add:**
1. **core/topology.rs** — Topology versioning, forking, tags (current/exploring/archived)
2. **core/graph.rs** — In-memory graph construction from SQL, traversal operations, diff
3. **domains/storage/topology_analysis.rs** — Pure analysis functions (redundancy, failure sim, RPO, capacity projection)
4. **cli/topology.rs** — Topology CRUD commands (create, fork, diff, show)
5. **cli/node.rs, cli/volume.rs, cli/dataset.rs** — Entity-specific commands
6. **output/** — Structured formatting (text tables, JSON, YAML, prime context)

**Data flow:**
```
User input → CLI parse → Open DB → Load models → Domain analysis → Format output → Print
```

**Graph operations:**
```
Load topology → Build petgraph from SQL → Run analysis → Return results
```

**Storage pattern:**
Normalized tables (topologies, nodes, volumes, datasets, placements, sync_regimes, links) with JSON metadata columns. Use petgraph for in-memory analysis, not for persistence. Store topology snapshots as denormalized JSON for complex traversals if recursive CTEs prove slow.

**Key architectural principles:**
- Layered dependencies (cli → output → domains → core)
- Transaction-scoped mutations with event logging
- Pure analysis functions (no I/O inside analysis)
- Typed spec parsing (avoid string comparison bugs)
- AI-friendly output via `--format` flag

### Critical Pitfalls

Research identified 10 pitfalls across three severity levels. Top 5 critical/moderate issues:

1. **Normalized tables for deep graph traversal** — SQLite recursive CTEs degrade badly with graph depth. Mitigation: Hybrid schema with denormalized JSON snapshots for complex queries, or closure table for frequently-queried relationships. Address in schema design phase.

2. **Flat subcommand enum architecture** — Makes global options (`--format=json`) impossible to add later without massive refactor. Mitigation: Start with nested App/Command structure using `#[clap(flatten)]` and `#[clap(global = true)]`. Address in CLI scaffolding phase.

3. **Branches without structural sharing** — Full-copy forking causes database bloat and slow operations. Mitigation: Accept for MVP if topologies stay small (<50 nodes, <10 forks), document limitation, plan delta-based storage for v2. Address in topology versioning phase.

4. **Index instability with petgraph** — Using `Graph` instead of `StableGraph` causes indices to become invalid after removals. Mitigation: Use `StableGraph`, never persist `NodeIndex`/`EdgeIndex` to database, rebuild graph from SQL for each analysis. Address in analysis implementation phase.

5. **Inconsistent output formats** — Different commands using different formats breaks AI agent parsing. Mitigation: Global `--format` flag from day one, all commands implement same output interface, test JSON parseability in CI. Address in CLI scaffolding phase.

## Implications for Roadmap

Based on research, suggested phase structure follows dependency order and architecture component boundaries.

### Phase 1: Schema and Core Types
**Rationale:** Foundation for all topology work. Cannot add CLI commands without database schema and core models in place.
**Delivers:** Migration for topology tables, Topology/Node/Volume/Dataset structs with CRUD operations.
**Addresses:** Table stakes requirement for state persistence and graph modeling.
**Avoids:** Pitfall #7 (schema migration without version tracking) by implementing proper version tracking from start.

**Research flag:** Standard pattern (database schema design), skip `/gsd:research-phase`.

### Phase 2: CLI Scaffolding and Basic Commands
**Rationale:** Establish output format patterns and command structure before implementing all entity types. Prevents Pitfall #2 (flat subcommand enum) and Pitfall #6 (inconsistent output).
**Delivers:** Global `--format` flag, nested CLI structure, `sp topology create/show/list`, `sp node add/list`, `sp volume add/list`.
**Addresses:** Table stakes CRUD and structured output requirements.
**Avoids:** Pitfalls #2, #6, #8 (deep command hierarchy) by designing CLI structure upfront.

**Research flag:** Standard pattern (Rust CLI with clap), skip `/gsd:research-phase`.

### Phase 3: Topology Versioning
**Rationale:** Enables exploration workflow (fork, tag, diff). Must come before decision integration so decisions can reference specific topology versions.
**Delivers:** Fork operation, tag management (current/exploring/archived), diff between topologies.
**Addresses:** Differentiator feature for git-like branching.
**Avoids:** Pitfall #3 by accepting full-copy for MVP, documenting limitation.

**Research flag:** Standard pattern (version control concepts), skip `/gsd:research-phase`.

### Phase 4: Analysis Functions
**Rationale:** Core value proposition. Can be developed in parallel with CLI after schema exists. Pure functions (no I/O) make this highly testable.
**Delivers:** Redundancy analysis, capacity projection, RPO compliance checking, failure simulation (basic).
**Addresses:** Differentiator features (redundancy, RPO, failure sim, capacity).
**Avoids:** Pitfall #1 (recursive CTE performance) by using hybrid approach, Pitfall #4 (index instability) by using StableGraph.

**Research flag:** Needs `/gsd:research-phase` for failure simulation and bandwidth analysis algorithms (complex graph traversal).

### Phase 5: Decision Integration
**Rationale:** Ties topologies to decision workflow. Requires topology commands and analysis functions to be complete.
**Delivers:** Link decisions to topologies, topology staleness detection, enhanced `sp prime` with topology context.
**Addresses:** Table stakes `sp prime` requirement and differentiator decision tracking.
**Avoids:** Pitfall #9 (missing discoverability) by including command reference in prime output.

**Research flag:** Standard pattern (foreign keys, joins), skip `/gsd:research-phase`.

### Phase 6: Cost Analysis Integration
**Rationale:** Final integration connecting catalog items to topology nodes/volumes, enabling full purchase decision workflow.
**Delivers:** Link catalog items to volumes, cost analysis (one-time + TCO), export purchase list.
**Addresses:** Differentiator cost analysis feature.
**Avoids:** No specific pitfalls, but depends on all prior phases.

**Research flag:** Standard pattern (aggregation, joins), skip `/gsd:research-phase`.

### Phase Ordering Rationale

- **Schema first (Phase 1)** because nothing works without database tables
- **CLI scaffolding early (Phase 2)** to establish patterns before implementing many commands
- **Versioning before analysis (Phase 3)** so analysis can reference specific versions
- **Analysis can develop in parallel (Phase 4)** with earlier phases since functions are pure
- **Decision integration late (Phase 5)** because it depends on topology + analysis
- **Cost analysis last (Phase 6)** as final integration of all systems

This ordering minimizes blocking dependencies, allows parallel work on analysis functions, and addresses critical pitfalls early (CLI structure, schema design) before they become expensive to fix.

### Research Flags

**Phases likely needing `/gsd:research-phase` during planning:**
- **Phase 4 (Analysis):** Failure simulation and bandwidth analysis require complex graph algorithms (widest path, transitive closure, cut vertex detection). Research needed for optimal petgraph usage patterns.

**Phases with standard patterns (skip research-phase):**
- **Phase 1 (Schema):** Database schema design is straightforward, well-documented patterns
- **Phase 2 (CLI):** Rust CLI with clap is well-established, existing codebase provides template
- **Phase 3 (Versioning):** Parent pointers and tagging are standard version control patterns
- **Phase 5 (Decision Integration):** Foreign keys and joins are standard SQL
- **Phase 6 (Cost Analysis):** Aggregation queries are standard SQL

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Verified via official docs (petgraph, rusqlite, tabled, garde). All versions confirmed. |
| Features | MEDIUM | Synthesized from IaC patterns (Terraform, Pulumi), ADR tools, storage management practices. No single authoritative source for this domain. |
| Architecture | MEDIUM-HIGH | Verified against existing codebase patterns. Hybrid storage approach based on multiple SQLite performance articles and forum discussions. |
| Pitfalls | MEDIUM-HIGH | SQLite and petgraph pitfalls verified via official docs and community consensus. CLI pitfalls based on Rust CLI recommendations. Versioning pitfalls from database branching articles. |

**Overall confidence:** MEDIUM-HIGH

Research provides strong guidance on technology choices and architectural patterns. Feature prioritization is based on pattern matching across adjacent domains (IaC, decision tools) which is less authoritative but well-reasoned. Confidence is sufficient to proceed with roadmap creation.

### Gaps to Address

Areas where research was inconclusive or needs validation during implementation:

- **Recursive CTE performance threshold:** Research indicates problems with deep/wide graphs, but doesn't specify exact thresholds for SQLite. May need performance testing during Phase 1 to determine when to switch to denormalized snapshots or closure tables. Mitigation: Implement hybrid approach from start, monitor query times.

- **Optimal branching strategy:** Research identified full-copy vs delta-based approaches but couldn't determine exact tipping point. Mitigation: Accept full-copy for MVP, document storage growth, add telemetry to inform v2 optimization.

- **Failure simulation algorithm specifics:** Research confirmed need for cut vertex detection and reachability analysis but didn't detail implementation. Mitigation: Flag Phase 4 for `/gsd:research-phase` to research graph algorithms when that phase is planned.

- **AI agent CLI usability patterns:** Emerging area with limited established practices. Research found one article on rethinking CLI for AI. Mitigation: Iterate based on actual agent usage, treat `sp prime` output format as experimental.

## Sources

### Primary (HIGH confidence)
- [petgraph 0.8.3 documentation](https://docs.rs/petgraph/latest/petgraph/) — Graph library, serialization, StableGraph
- [rusqlite 0.38.0 documentation](https://docs.rs/rusqlite/latest/rusqlite/) — SQLite bindings
- [tabled 0.20.0 documentation](https://docs.rs/tabled/latest/tabled/) — Table formatting
- [garde 0.22.1 documentation](https://docs.rs/garde/latest/garde/) — Validation
- [SQLite Recursive CTEs](https://sqlite.org/lang_with.html) — Graph traversal in SQL
- [SQLite JSON Functions](https://sqlite.org/json1.html) — JSON column indexing
- [Rain's Rust CLI Recommendations](https://rust-cli-recommendations.sunshowers.io/handling-arguments.html) — CLI architecture
- Existing codebase: `/Users/morgan/code/storage-planner/src/` — Verified patterns

### Secondary (MEDIUM confidence)
- [Terraform Graph Command](https://developer.hashicorp.com/terraform/cli/commands/graph) — IaC DAG patterns
- [Pulumi State and Backends](https://www.pulumi.com/docs/iac/concepts/state-and-backends/) — Resource graph concepts
- [SQLite JSON and Denormalization](https://maximeblanc.fr/blog/sqlite-json-and-denormalization) — Hybrid storage patterns
- [SQLite Recursive CTE Performance](https://sqlite.org/forum/info/016a25083a9f8eb5a961eb7c2362f667cbca305f65dccb2e82170df7) — Performance issues
- [DoltHub: Database Branches](https://www.dolthub.com/blog/2024-09-18-database-branches/) — Branching strategies
- [petgraph Review](https://timothy.hobbs.cz/rust-play/petgraph_review.html) — Index stability issues
- [adr-tools](https://github.com/npryce/adr-tools) — Decision record patterns
- [ZFS vs Btrfs Architecture](https://klarasystems.com/articles/zfs-vs-btrfs-architects-features-and-stability-2/) — Storage topology patterns

### Tertiary (LOW confidence)
- [Rethinking CLI Interfaces for AI](https://www.notcheckmark.com/2025/07/rethinking-cli-interfaces-for-ai/) — Emerging pattern, single source

---
*Research completed: 2026-02-06*
*Ready for roadmap: yes*
