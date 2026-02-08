# Phase 6: Cost and Context - Context

**Gathered:** 2026-02-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Cost analysis for topology purchase decisions, AI agent bootstrap via `sp prime`, status dashboard, and topology import/export with ASCII diagrams. Catalog CRUD may already exist (verify or implement). This phase completes the tool by connecting pricing data to topologies and enabling session continuity for AI agents.

</domain>

<decisions>
## Implementation Decisions

### Cost model design
- Both per-entity breakdown AND category summary views available — user picks with a flag
- Default shows separate one-time and recurring sections; `--tco=3yr` flag adds total cost of ownership projection
- Price selection uses latest observation (most recent price recorded)
- Claude's discretion on how catalog items link to topology entities (direct association vs bill of materials — pick what fits the data model)

### Prime output format
- `sp prime` is an **agent bootstrap** document (like `bd prime`), NOT a data dump
- Static instructional content (how to use sp, workflow guide, example commands) with dynamically appended state summary
- Instructions only — no inline topology data. Agent runs `sp status` or specific commands for state
- Stdout only, no file output flag
- Complements CLAUDE.md — CLAUDE.md has project-level info, `sp prime` has runtime command guide and usage patterns
- Include concrete example commands showing typical usage patterns

### Status dashboard
- `sp status` is a full health report: current topology + analysis summary, open decisions with status, catalog stats, recent activity
- Problems highlighted prominently at the top — "2 datasets at risk, 1 decision open 30+ days" — action-oriented alerts
- Supports `--format=json` consistent with all other commands
- Claude's discretion on whether to run inline mini-analysis or reference last analysis results

### Import/export & diagram
- YAML export: default preserves identity for backup (round-trip fidelity), `--template` flag strips IDs for reuse
- Export scope: default full graph, `--only=nodes,volumes` for partial export of large topologies
- ASCII diagram: standalone `sp diagram` command (not a flag on show)
- Two diagram perspectives: `sp diagram --tree` for node-volume-dataset hierarchy, `sp diagram --network` for link topology between nodes
- Claude's discretion on diagram rendering approach

### Claude's Discretion
- Catalog item linking model (direct vs bill of materials)
- Whether `sp status` runs inline mini-analysis or references last analysis
- Diagram rendering implementation
- `sp prime` workflow guide structure (full workflow vs action-oriented — pick what works best for agent bootstrap)

</decisions>

<specifics>
## Specific Ideas

- `sp prime` should be modeled on `bd prime` — agent instructions for how to use the tool, not a data dump
- Include concrete command examples in prime output so agents can copy/adapt
- Status dashboard should feel like a health check — highlight what needs attention, quiet when everything's fine

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 06-cost-and-context*
*Context gathered: 2026-02-07*
