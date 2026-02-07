# Phase 3: Topology Versioning - Context

**Gathered:** 2026-02-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Users can fork topologies to explore alternatives, tag them with lifecycle stages, diff two topologies to see what changed, and view fork lineage. This phase adds the exploration workflow on top of the existing CRUD from Phase 2.

</domain>

<decisions>
## Implementation Decisions

### Fork behavior
- User can fork from any topology, not just the active one
- Fork name is optional — user can provide via flag, otherwise auto-generate
- Claude's Discretion: deep copy vs shallow strategy, auto-generated name format

### Tagging semantics
- Topologies have lifecycle tags: current, exploring, archived
- Only one topology can be "current" at a time (enforced)
- Claude's Discretion: whether tags replace or coexist with the existing set-active concept
- Claude's Discretion: one tag per topology vs multiple tags
- Claude's Discretion: what happens to the previous "current" when a new one is set
- Claude's Discretion: whether archived topologies are hidden by default in list

### Diff output
- Full detail: entity-level changes PLUS field-level diffs (e.g., "node capacity: 4TB → 8TB")
- Diff supports filtering by entity type via flags (e.g., `--nodes --volumes`)
- If only one topology specified, diff uses the current/active topology as the implicit base
- Claude's Discretion: terminal presentation style (git-style, side-by-side, summary+detail)

### Version history
- Two lineage commands: tree view (all topologies as fork tree) and log view (single topology's ancestry)
- Tree view shows tags alongside each topology name (e.g., "my-topo [current]")
- Claude's Discretion: fork depth limits, detail level in topology show for parent/child info

</decisions>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 03-topology-versioning*
*Context gathered: 2026-02-07*
