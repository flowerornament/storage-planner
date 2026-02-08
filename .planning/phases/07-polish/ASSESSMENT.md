# Post-v1 Polish Assessment

Hands-on testing of the full `sp` CLI after Phase 6 completion. Exercised a real workflow: creating a home NAS topology with 3 nodes, 4 volumes, 4 datasets, 10 placements, 3 links, 2 sync regimes, 3 catalog items with 5 price observations, decisions, export/import round-trips, and all analysis commands.

## Priority 1: Prime Command Examples Are Wrong

`sp prime` is the agent bootstrap document -- its entire purpose is giving AI agents correct commands to run. **6 out of ~30 example commands have wrong syntax** (~20% failure rate on first contact).

| Prime says | Reality | Issue |
|---|---|---|
| `sp placement add --dataset=<ds> --volume=<vol>` | `sp placement add <ds> <vol>` | flags vs positional |
| `sp link add <src> <tgt> --type=lan` | `sp link add --from=<src> --to=<tgt> --connection-type=lan` | reversed (positional to flags), wrong flag name |
| `sp sync add <name> --source=<vol> --target=<vol> --type=rsync` | `sp sync add <name> --from=<vol> --to=<vol> --sync-type=rsync` | wrong flag names (--source/--target vs --from/--to, --type vs --sync-type) |
| `sp analyze compare --a=<t1> --b=<t2>` | `sp analyze compare <t1> <t2>` | flags vs positional |
| `sp decision add-topology <dec> <topo>` | `sp decision consider <dec> <topo>` | wrong subcommand name |
| `sp decision add-constraint <dec> --type=budget` | `sp decision constrain <dec> --type=budget` | wrong subcommand name |

**Fix:** Regenerate the static content in `src/cli/prime.rs` to match actual CLI syntax. Could also consider generating from clap metadata at build time to prevent drift, but that's overkill for v1 -- just fix the strings.

## Priority 2: No --item-id on Node/Volume Update

The schema has `item_id` on both `nodes` and `volumes` tables (added in migration v4). The entity resolver and catalog system work. But neither `sp node update` nor `sp volume update` expose `--item-id` as a flag.

This means:
- `sp analyze cost` always shows "$0.00" because no entities can be linked to catalog items
- The entire cost analysis workflow is unusable in practice
- The catalog exists in isolation from the topology

**Fix:** Add `--item-id <ITEM>` flag to both `sp node update` and `sp volume update`. Resolve the item via `resolve_catalog_item` to validate it exists. Also consider adding `--item-id` to `sp node add` and `sp volume add` for convenience.

## Priority 3: Raw SQLite Errors on Duplicate Names

Every entity surfaces raw constraint violations instead of user-friendly messages:

```
$ sp catalog add "Test Item" --category=test
Added item: Test Item (f3301f02)
$ sp catalog add "Test Item" --category=test
Error: UNIQUE constraint failed: catalog_items.name

Caused by:
    Error code 2067: A UNIQUE constraint failed
```

Same for topologies, nodes (scoped to topology), volumes (scoped to node), etc.

**Fix:** Catch `rusqlite::ErrorCode::ConstraintViolation` in insert paths and return friendly messages like "A catalog item named 'Test Item' already exists" or "Node 'server1' already exists in topology 'home-nas'".

## Priority 4: Catalog List Missing Column Headers

`sp catalog list` output:
```
  Samsung 870 EVO 4TB            ssd          -                                          $274.99
  Synology DS923+                nas          https://www.synology.com/ds923plus         $569.99
```

No column headers. Compare with `sp catalog price list` which does have headers (Date, Amount, Source, Condition, Type). Inconsistent.

**Fix:** Add a header row: `Name | Category | URL | Latest Price` with separator line, matching the price list pattern.

## Lower Priority Observations (Not Blocking)

### Negative Prices Accepted
`sp catalog price add "X" --amount=-500` records `$-5.00` without warning. Could be intentional (rebates/credits) but probably worth a confirmation or at least a warning message.

### Topology Show --tree vs Diagram Overlap
`sp topology show <name> --tree` shows nodes with volumes underneath but doesn't nest datasets. `sp diagram --tree` shows the full hierarchy (nodes > volumes > datasets). The diagram is strictly more informative. The show --tree feels like a leftover from before diagram existed.

### Redo Fails for Import and Decision Creation
`sp redo` after undoing a topology import or decision creation fails with `Error: No after_state for created event`. Undo works fine (deletes the entities), but redo can't recreate them. This affects multi-entity creation events.

### Template Export Retains Tag
`sp export --template` strips UUIDs but keeps `tag: current`. An imported template shouldn't inherit the "current" tag from the source topology.

## What Works Well

- **Tree diagram** -- Beautiful box-drawing output, proper nesting, informative metadata at each level
- **Network diagram** -- Clean `source (loc) --[type, bw]--> target (loc)` format
- **Status dashboard** -- Problems-first design, quiet when healthy, good section organization
- **Bandwidth analysis** -- `need 92.6 MB/s | have 1.0 GB/s` with ADEQUATE/TIGHT/INSUFFICIENT labels
- **Failure simulation** -- Per-dataset impact with severity classification
- **Export/import round-trip** -- Perfect fidelity, ID remapping works correctly
- **Undo** -- Works for complex operations including full topology imports
- **JSON output** -- Well-structured on all tested commands
- **Error messages** (for validation) -- Bad JSON specs shows example, bad price type lists valid options, bad TCO format shows expected pattern
- **Human-readable capacity input** -- Both `4TB` and `4000000000000` work
- **Entity resolution** -- Name and UUID prefix matching works consistently across all entity types
- **Price formatting** -- Cents stored, dollars displayed, consistent everywhere
