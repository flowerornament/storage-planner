# Domain Pitfalls: Rust CLI Graph Modeling in SQLite

**Domain:** Rust CLI tool with graph-based topology modeling, SQLite backend, AI-agent usability
**Project:** storage-planner (sp)
**Researched:** 2026-02-06
**Context:** Brownfield addition to existing Rust CLI with items/prices/configurations/decisions tables

---

## Critical Pitfalls

Mistakes that cause rewrites or major issues.

### Pitfall 1: Normalized Tables for Deep Graph Traversal

**What goes wrong:** Using fully normalized tables (separate nodes, edges, node_properties tables) for graph data, then discovering recursive CTE performance degrades badly with graph depth and size.

**Why it happens:** Developers apply traditional relational normalization habits. Works fine for shallow queries (depth 1-2), but SQLite recursive CTEs visit the same nodes multiple times when there are multiple paths, and performance degrades linearly or worse with data size.

**Consequences:**
- `sp analyze` commands become seconds-slow as topology grows
- Complex traversals (failure simulation, path finding) timeout
- Forced to add caching layer or rewrite schema

**Warning signs:**
- Queries with `WITH RECURSIVE` taking >100ms on moderate data
- Analysis commands slower after each topology fork
- N+1 query patterns in trace logs

**Prevention:**
- Hybrid schema: Normalized edges table + denormalized JSON snapshots
- Store computed traversal results (closure table) for frequently-queried relationships
- Index expressions on JSON fields for common access patterns
- Consider storing small topologies (<100 nodes) as full JSON documents with edge table for cross-topology queries only

**Phase to address:** Schema design phase. Cannot be retrofitted easily without migration.

**Confidence:** HIGH (verified with SQLite documentation and forum discussions)

**Sources:**
- [SQLite Recursive CTE Performance](https://sqlite.org/forum/info/016a25083a9f8eb5a961eb7c2362f667cbca305f65dccb2e82170df7)
- [SQLite JSON and Denormalization](https://maximeblanc.fr/blog/sqlite-json-and-denormalization)

---

### Pitfall 2: Flat Subcommand Enum Architecture

**What goes wrong:** Implementing CLI as a single flat enum of all subcommands (`enum Command { ItemAdd, ItemList, TopologyCreate, TopologyFork, ... }`), then needing to add global options or cross-cutting concerns.

**Why it happens:** Flat enums are simplest to start with. clap derives work cleanly. But the design doesn't accommodate global options (`--format=json`), shared flags (`--topology=X` across multiple commands), or middleware (logging, timing).

**Consequences:**
- Massive refactor when adding `--json` output flag
- Duplicated argument definitions across subcommands
- Difficulty adding `sp --verbose` or `sp --format=json prime`
- AI agents struggle with inconsistent option placement

**Warning signs:**
- Repeating `#[clap(long)]` on same option across multiple subcommands
- Unable to add global `--format` without touching every command
- Help output shows inconsistent option availability

**Prevention:**
- Start with nested structure: `App` struct with global options, `Command` enum flattened in
- Use `#[clap(flatten)]` for shared option groups (OutputOptions, TopologyContext)
- Mark shared options with `#[clap(global = true)]`
- Only expose top-level `App` struct publicly

**Phase to address:** CLI scaffolding phase (first phase). Much harder to fix later.

**Confidence:** HIGH (documented Rust CLI best practice)

**Sources:**
- [Rain's Rust CLI Recommendations](https://rust-cli-recommendations.sunshowers.io/handling-arguments.html)

---

### Pitfall 3: Branches Without Structural Sharing

**What goes wrong:** Implementing topology forking by copying entire topology data for each branch. Storage grows linearly with branches. Large topologies make forking slow (seconds to minutes).

**Why it happens:** Full copy is simplest to implement. "Fork" becomes `INSERT INTO topologies SELECT * FROM topologies WHERE id = ?` with new IDs. Works fine until topologies grow or users create many exploration branches.

**Consequences:**
- Database bloat (10 forks = 10x storage)
- Slow fork operations (copying hundreds of nodes/edges)
- Diff operations require comparing full documents
- Cannot scale to "what-if" exploration workflows

**Warning signs:**
- Database file growing faster than expected
- `sp topology fork` taking >1 second
- Users avoiding forking due to slowness

**Prevention:**
- Content-addressed storage for topology components (hash-based deduplication)
- Store deltas from parent topology, not full copies
- Compute full topology on-demand by walking parent chain
- Use closure table for efficient ancestor queries

**Alternative (simpler):** Accept the tradeoff for v1. If topologies stay small (<50 nodes, <10 forks), full copy is fine. Document the limitation. Optimize in v2 if needed.

**Phase to address:** Topology versioning phase. Design choice, not critical for MVP if topologies stay small.

**Confidence:** MEDIUM (pattern documented, but tradeoff may be acceptable for this use case)

**Sources:**
- [DoltHub: Database Branches](https://www.dolthub.com/blog/2024-09-18-database-branches/)
- [Specfy: Git-like Versioning in Postgres](https://www.specfy.io/blog/7-git-like-versioning-in-postgres)

---

## Moderate Pitfalls

Mistakes that cause delays or technical debt.

### Pitfall 4: Index Instability with petgraph

**What goes wrong:** Using petgraph's `Graph` type for in-memory analysis, storing node/edge indices in the database or across operations, then discovering indices become invalid after removal operations.

**Why it happens:** `NodeIndex` and `EdgeIndex` in petgraph's default `Graph` type are unstable. Removing a node forces the last node to take its place. Code that caches indices across operations gets wrong results or panics.

**Consequences:**
- Mysterious wrong results in analysis
- Panics when accessing removed nodes
- Subtle bugs that appear only in specific removal sequences

**Warning signs:**
- Tests passing individually but failing when run together
- Analysis results that vary based on operation order
- Panics on `graph[node_idx]` access

**Prevention:**
- Use `StableGraph` instead of `Graph` if any removal operations occur
- Never persist `NodeIndex`/`EdgeIndex` to database; use domain IDs
- Rebuild graph from database for each analysis operation (simpler, safer)
- If caching graph in memory, invalidate on any topology mutation

**Phase to address:** Analysis implementation phase.

**Confidence:** HIGH (documented petgraph behavior)

**Sources:**
- [Petgraph Documentation](https://docs.rs/petgraph/)
- [Petgraph Review](https://timothy.hobbs.cz/rust-play/petgraph_review.html)

---

### Pitfall 5: JSON Blob Without Indexes

**What goes wrong:** Storing topology data as JSON in a single column, then needing to query by properties inside the JSON. Every query becomes a full table scan.

**Why it happens:** JSON storage is flexible and fast to implement. "We'll add indexes later." But later never comes, and queries like "find all volumes with capacity > 4TB" scan every topology.

**Consequences:**
- Slow queries as data grows
- Unable to use SQL for filtering; must load and filter in Rust
- `sp catalog search` becomes unusably slow

**Warning signs:**
- EXPLAIN showing "SCAN" not "SEARCH" for JSON queries
- Query time proportional to table size, not result size
- Adding more data makes everything slower

**Prevention:**
- Create expression indexes on frequently-queried JSON fields: `CREATE INDEX idx_capacity ON volumes (json_extract(data, '$.capacity'))`
- Use generated columns for commonly-accessed JSON properties
- Hybrid approach: Extract key queryable fields to columns, keep rest as JSON metadata
- Add indexes during schema design, not as afterthought

**Phase to address:** Schema design phase.

**Confidence:** HIGH (well-documented SQLite pattern)

**Sources:**
- [SQLite JSON Functions](https://sqlite.org/json1.html)
- [High Performance SQLite: JSON vs JSONB](https://highperformancesqlite.com/watch/json-vs-jsonb)

---

### Pitfall 6: Inconsistent Output Formats Across Commands

**What goes wrong:** Some commands output human-readable tables, others output JSON, others output prose. AI agents cannot reliably parse output. Users cannot script the tool.

**Why it happens:** Different commands implemented at different times. No output format standard established. "We'll make it consistent later."

**Consequences:**
- AI agents fail to parse output, require custom prompting per command
- Shell scripting requires fragile regex parsing
- `sp prime` works but individual commands don't compose
- Users frustrated by inconsistency

**Warning signs:**
- Different output formats in adjacent commands
- AI agent errors parsing specific command output
- Users asking "how do I get JSON from X command?"

**Prevention:**
- Establish output format trait/enum from day one: `Human | Json | Yaml`
- Global `--format` flag on `App` struct
- All commands implement same output interface
- Test JSON output parseability in CI
- Default to human-readable, but make machine-readable trivial

**Phase to address:** CLI scaffolding phase. Enforce from first command.

**Confidence:** HIGH (essential for AI-agent usability per project requirements)

---

### Pitfall 7: Schema Migration Without Version Tracking

**What goes wrong:** Adding new columns or tables without proper migration tracking. Existing databases break silently or require manual intervention.

**Why it happens:** SQLite makes it easy to just `ALTER TABLE`. No migration framework feels necessary for a "small" project. Then users hit "table already exists" or "no such column" errors.

**Consequences:**
- Existing `.sp/decisions.db` files become unusable after update
- Users lose data or must manually migrate
- No rollback path
- Different users on different schema versions

**Warning signs:**
- "no such column" errors after upgrade
- Tests passing but real databases failing
- Users reporting database errors after update

**Prevention:**
- Track schema version in database (e.g., `PRAGMA user_version`)
- Run migrations on database open, not just on init
- Write forward-only migrations (no down migrations for simplicity)
- Test migrations against databases from previous versions
- Never use `CREATE TABLE IF NOT EXISTS` for evolving schemas (hides version drift)

**Phase to address:** First phase (schema design). The existing codebase uses `CREATE TABLE IF NOT EXISTS` which will cause issues as schema evolves.

**Confidence:** HIGH (existing code already has this anti-pattern)

---

## Minor Pitfalls

Mistakes that cause annoyance but are fixable.

### Pitfall 8: Overly Deep Command Hierarchy

**What goes wrong:** Creating deeply nested subcommands like `sp topology node volume dataset add`. Users (and agents) forget the path. Help output becomes overwhelming.

**Why it happens:** Mirroring the data model in the CLI. Nodes contain volumes, so `sp node` contains `sp node volume`. Logical, but unusable.

**Consequences:**
- Users can't remember command paths
- AI agents hallucinate wrong paths
- Tab completion becomes essential but may not be configured
- Help output overwhelms

**Warning signs:**
- Commands with 4+ words
- Users frequently typing wrong command paths
- `--help` output scrolling multiple screens

**Prevention:**
- Maximum 2-3 levels: `sp <entity> <action>` or `sp <action> <entity>`
- Flatten where possible: `sp volume add` not `sp node volume add`
- Use arguments for context: `sp volume add --node=X` not `sp node X volume add`
- Test: Can a user type the command from memory?

**Phase to address:** CLI design phase.

**Confidence:** MEDIUM (subjective, but deep hierarchies are common complaint)

---

### Pitfall 9: Missing Discoverability for AI Agents

**What goes wrong:** Relying on `--help` alone for discoverability. AI agents must call `sp --help`, parse it, then call `sp subcommand --help`, parse that, etc. Context window fills with help text.

**Why it happens:** Standard CLI practice. Works for humans who read once and remember. Doesn't work for agents starting fresh each session.

**Consequences:**
- Agent wastes context window on help text
- Multiple tool calls just to understand available commands
- Agent may miss commands or misunderstand options
- Slower agent interaction

**Warning signs:**
- Agent transcripts showing multiple `--help` calls before real work
- Agent confusing command syntax despite help
- Agent not using commands that exist

**Prevention:**
- `sp prime` includes command reference (already planned)
- Consider `sp help --machine` for structured command metadata
- Embed command summaries in `sp prime` output
- Add "next steps" suggestions to command output
- Document commands in a format agents can request once

**Phase to address:** Prime command enhancement (after core features).

**Confidence:** MEDIUM (emerging pattern for AI-tool interaction)

**Sources:**
- [Rethinking CLI Interfaces for AI](https://www.notcheckmark.com/2025/07/rethinking-cli-interfaces-for-ai/)

---

### Pitfall 10: Storing Dates as Text

**What goes wrong:** Using `datetime('now')` (TEXT) for all timestamps. Date arithmetic, comparisons, and ordering become string operations that can have unexpected behavior.

**Why it happens:** SQLite's type affinity makes it easy. `datetime()` returns text. Works fine until you need "prices from last 7 days" queries.

**Consequences:**
- Date comparisons require careful formatting
- Time zones become ambiguous
- Queries like `WHERE observed_at > date('now', '-7 days')` work but are fragile
- Epoch math requires conversion functions

**Warning signs:**
- Date comparison bugs (lexicographic vs chronological)
- Timezone confusion in queries
- Slow date range queries (no index benefit)

**Prevention:**
- Use INTEGER (Unix epoch) for internal storage
- Store as seconds or milliseconds from epoch
- Convert to human-readable on display only
- Index integer timestamps for fast range queries
- Document the convention (UTC epoch seconds)

**Note:** The existing schema uses TEXT dates. This is a cleanup item, not blocking.

**Phase to address:** Can be migrated later, but note during schema review.

**Confidence:** MEDIUM (existing code uses TEXT, works but suboptimal)

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Schema design | Normalized tables for graph data | Use hybrid: edge table + JSON snapshots with indexes |
| Schema design | JSON without indexes | Add expression indexes upfront for key fields |
| Schema design | IF NOT EXISTS migrations | Add proper version tracking from start |
| CLI scaffolding | Flat subcommand enum | Start with nested App/Command structure |
| CLI scaffolding | Inconsistent output | Global --format flag from first command |
| CLI scaffolding | Deep command hierarchy | Maximum 2-3 levels, use arguments for context |
| Topology versioning | Full-copy forking | Accept for MVP, document limitation, plan v2 optimization |
| Analysis implementation | petgraph index instability | Use StableGraph, never persist indices |
| Analysis implementation | Recursive CTE performance | Denormalize or add closure table for deep traversals |
| AI usability | Help-only discoverability | Embed command reference in `sp prime` |

---

## Domain-Specific Insights

### Topology Modeling in SQLite is Viable

SQLite can handle graph data well with the right patterns:
- Edge tables work for relationship queries
- JSON columns work for flexible node/edge properties
- Expression indexes make JSON queryable
- Recursive CTEs work for moderate graph sizes (<1000 nodes)

The key is hybrid design: use relational structure for relationships and queries, JSON for flexible properties, and denormalized snapshots for complex traversals.

### AI Agent Usability is a First-Class Concern

This project explicitly targets AI agent consumption. Key patterns:
- Structured output (JSON) must be trivially accessible
- `sp prime` is the session recovery mechanism
- Commands should be memorable, not discoverable
- Output should suggest next actions

Standard CLI UX (progressive disclosure, help commands) is insufficient for agents.

### Versioning Complexity Can Be Deferred

Full git-like branching with structural sharing is complex. For a tool modeling <50 nodes with <10 exploration branches:
- Full copy on fork is acceptable
- Simple parent_id lineage is sufficient
- Delta-based storage can be added later

Don't over-engineer versioning until usage patterns prove it necessary.

---

## Sources

### High Confidence (Official/Authoritative)
- [SQLite Documentation: Recursive CTEs](https://sqlite.org/lang_with.html)
- [SQLite Documentation: JSON Functions](https://sqlite.org/json1.html)
- [petgraph Documentation](https://docs.rs/petgraph/)
- [Rain's Rust CLI Recommendations](https://rust-cli-recommendations.sunshowers.io/handling-arguments.html)

### Medium Confidence (Multiple Sources Agree)
- [SQLite JSON and Denormalization](https://maximeblanc.fr/blog/sqlite-json-and-denormalization)
- [DoltHub: Database Branches](https://www.dolthub.com/blog/2024-09-18-database-branches/)
- [petgraph Review](https://timothy.hobbs.cz/rust-play/petgraph_review.html)
- [SQLite Forum: Recursive CTE Performance](https://sqlite.org/forum/info/016a25083a9f8eb5a961eb7c2362f667cbca305f65dccb2e82170df7)

### Low Confidence (Single Source, Needs Validation)
- [Rethinking CLI Interfaces for AI](https://www.notcheckmark.com/2025/07/rethinking-cli-interfaces-for-ai/)
