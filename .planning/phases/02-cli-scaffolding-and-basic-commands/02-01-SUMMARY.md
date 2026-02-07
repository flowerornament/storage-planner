---
phase: 02-cli-scaffolding-and-basic-commands
plan: 01
subsystem: core-resolver-and-cli-wiring
tags: [resolver, cli, topology, placement, slug-validation, name-or-id]
requires:
  - phase-01 (schema, models, events, CLI scaffold)
provides:
  - Entity resolver module (name-or-ID lookup for all entities)
  - Slug validation for entity names
  - Active topology resolution with --topology override
  - CLI dispatch wiring (db + format to all entity commands)
  - Topology update subcommand (--rename, --description)
  - Topology show --tree (hierarchical node/volume view)
  - Placement command skeleton (Add/List/Remove)
affects:
  - 02-02 (node/volume CRUD will use resolver and --topology)
  - 02-03 (dataset/placement CRUD will use resolver and --topology)
  - 02-04 (link/sync CRUD will use resolver and --topology)
tech-stack:
  added: []
  patterns: [name-or-id-resolution, resolve-outside-transaction, slug-validation]
key-files:
  created:
    - src/core/resolve.rs
    - src/cli/placement.rs
  modified:
    - src/core/mod.rs
    - src/cli/mod.rs
    - src/cli/topology.rs
    - src/cli/node.rs
    - src/cli/volume.rs
    - src/cli/dataset.rs
    - src/cli/link.rs
    - src/cli/sync_regime.rs
key-decisions:
  - D009: "Resolve entities outside transactions, use resolved ID inside (avoids conn conflicts)"
  - D010: "Slug validation requires alphanumeric, hyphens, underscores (no spaces or special chars)"
  - D011: "UUID prefix minimum 4 chars for disambiguation"
  - D012: "Volume disambiguation via --node flag when same name on multiple nodes"
duration: 4m 57s
completed: 2026-02-07
---

# Phase 2 Plan 1: Entity Resolver, CLI Wiring, and Topology Enhancements Summary

Entity resolver with name-or-ID disambiguation, slug validation, --topology override on all commands, topology update/rename/--tree view, placement skeleton.

## Performance

- **Duration:** 4m 57s
- **Started:** 2026-02-07T09:32:37Z
- **Completed:** 2026-02-07T09:37:34Z
- **Tasks:** 2/2
- **Files changed:** 10 (2 created, 8 modified)

## Accomplishments

1. **Entity resolver module** (`src/core/resolve.rs`, 340 lines): Name-or-ID lookup for topologies, nodes, volumes, and datasets. UUID prefix matching (4+ chars) with ambiguity detection. Volume disambiguation via `--node` hint. Active topology resolution with `--topology` override fallback.

2. **Slug validation**: `validate_slug()` function enforces alphanumeric, hyphen, underscore naming. Applied to topology create and rename operations.

3. **CLI dispatch wiring**: All entity commands (`node`, `volume`, `dataset`, `link`, `sync`, `placement`) now receive `&mut Database` and `OutputFormat` parameters. All subcommand variants include `--topology` override arg.

4. **Topology update command**: New `sp topology update <name> --description "..." --rename new-name` subcommand with before/after event recording, slug validation on rename, and uniqueness checking.

5. **Topology show --tree**: Hierarchical view displaying nodes with their volumes (capacity + filesystem + raid). JSON mode includes nested node/volume/dataset data.

6. **Placement command skeleton**: `PlacementCommands` enum with Add/List/Remove subcommands registered in CLI dispatch. Placeholder bodies for Phase 2 Plan 03.

7. **Resolver-based lookups**: All topology commands (show, update, set-active, delete) now use `resolve_topology()` instead of direct SQL, enabling both name and UUID prefix matching.

8. **Create output ID prefix**: `sp topology create` now shows `(id: abc12345)` in text output for easy reference.

## Task Commits

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | Create entity resolver module and update CLI dispatch | 66c3adb | src/core/resolve.rs, src/cli/placement.rs, src/cli/mod.rs, + 6 CLI modules |
| 2 | Enhance topology commands with update, resolver, and tree view | 6bf79a2 | src/cli/topology.rs |

## Files Created

- `src/core/resolve.rs` -- Entity resolver with name-or-ID lookup, slug validation, active topology helper (340 lines)
- `src/cli/placement.rs` -- Placement command skeleton with Add/List/Remove subcommands (70 lines)

## Files Modified

- `src/core/mod.rs` -- Added `pub mod resolve;`
- `src/cli/mod.rs` -- Added placement module, updated all dispatches to pass db+format
- `src/cli/topology.rs` -- Added Update subcommand, --tree flag, resolver-based lookups, ID prefix in create
- `src/cli/node.rs` -- Updated signature to accept db+format, added --topology to all variants
- `src/cli/volume.rs` -- Updated signature to accept db+format, added --topology to all variants
- `src/cli/dataset.rs` -- Updated signature to accept db+format, added --topology to all variants
- `src/cli/link.rs` -- Updated signature to accept db+format, added --topology to all variants
- `src/cli/sync_regime.rs` -- Updated signature to accept db+format, added --topology to all variants

## Decisions Made

1. **D009: Resolve outside transactions** -- Entity lookups via `resolve_topology()` happen before `db.transaction()` calls. The resolved entity ID is then used inside the transaction. This avoids rusqlite connection conflicts (can't borrow conn while transaction is active).

2. **D010: Slug validation rules** -- Names must match `[a-zA-Z0-9_-]+`. No spaces, no special characters. Applied at creation and rename points. Existing direct SQL lookups remain case-sensitive.

3. **D011: UUID prefix minimum 4 chars** -- Prefix resolution requires at least 4 characters to avoid false matches. Errors include guidance about minimum length.

4. **D012: Volume --node disambiguation** -- When a volume name exists on multiple nodes, the resolver errors with "Use --node to disambiguate" and lists the node names. With `--node`, it resolves the node first, then filters volumes by node_id.

## Deviations from Plan

None -- plan executed exactly as written.

## Issues Encountered

None.

## Next Phase Readiness

Plans 02-02, 02-03, and 02-04 can now proceed in parallel. All shared infrastructure is in place:
- Resolver functions available in `crate::core::resolve`
- All CLI modules accept `db: &mut Database` and `format: OutputFormat`
- All subcommands have `--topology: Option<String>` for override
- Placement skeleton registered and ready for implementation

No blockers for subsequent plans.

## Self-Check: PASSED
