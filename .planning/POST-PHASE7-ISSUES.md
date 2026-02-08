# Post-Phase 7 Issues

Comprehensive list from hands-on testing after Phase 7 completion. Two full testing sessions covering topology CRUD, fork/diff, analysis, decisions, catalog, export/import, undo/redo, prime guide accuracy, and edge cases.

---

## Bugs (Data Correctness)

### BUG-01: Undo of `.updated` events destroys cascaded children

**Severity:** Critical — silent data loss

The undo handler for `.updated` events (line 270-276 in `events.rs`) does `delete_entity()` then `restore_entity_from_json()`. For entities with FK cascade children (topologies → nodes → volumes → datasets → placements → sync_regimes), the delete wipes all children via `ON DELETE CASCADE`, and the restore only puts back the parent row.

**Repro:**
```bash
sp topology create t1 && sp topology tag t1 current
sp node add n1 --role=server
sp topology untag t1          # this is a topology.updated event
sp undo                       # undoes the untag — deletes topology, cascades all children, re-inserts topology
sp node list                  # n1 is gone
```

Even worse: undoing a `sp current <name>` (which is `topology.updated`) nukes the entire topology contents.

**Root cause:** `delete_entity` uses `DELETE FROM {table} WHERE id = ?1` which triggers cascade. Should use `UPDATE` to restore `before_state` fields instead of delete+insert.

### BUG-02: Undo of `.deleted` events doesn't restore cascaded children

**Severity:** High — silent data loss

When `sp dataset remove` or `sp volume remove` cascades (deleting placements, sync regimes), the undo only restores the parent entity from `before_state`. Cascaded children are permanently lost.

**Repro:**
```bash
sp dataset add ds1 --size=1TB
sp placement add ds1 vol1 --role=primary
sp dataset remove ds1           # "cascaded: 1 placements"
sp undo                         # restores dataset ds1
sp placement list               # ds1→vol1 placement is gone
```

**Root cause:** Events only capture before/after state of the primary entity. Cascaded deletions aren't recorded as separate events, so there's nothing to restore them from.

### BUG-03: Undo permanently blocked by missing FK cascade on `topologies.parent_id`

**Severity:** High — undo becomes unusable

`topologies.parent_id REFERENCES topologies(id)` has **no ON DELETE action** (no CASCADE, no SET NULL). If topology B was forked from A, any undo that tries to delete A (even transiently via BUG-01's delete+insert pattern) fails with:

```
Error: FOREIGN KEY constraint failed
```

The undo pointer doesn't advance, so undo is stuck permanently on that event.

**Repro:**
```bash
sp topology create base && sp topology tag base current
sp node add n1 --role=server
sp topology fork base --name=child
sp current base                 # topology.updated event
sp undo                         # FK error — child.parent_id references base
# All subsequent undo calls fail with same error
```

**Fix:** Add `ON DELETE SET NULL` to `topologies.parent_id` FK.

---

## Prime Guide Errors (Still Wrong After Phase 7)

Phase 7 fixed 5 examples but missed these 5:

### PRIME-01: `sp link add --type=lan`
- **Prime says:** `sp link add --from=<source-node> --to=<target-node> --type=lan --bandwidth=1GB/s`
- **Actual:** `--connection-type=lan` (not `--type`)

### PRIME-02: `sp sync add --type=rsync`
- **Prime says:** `sp sync add <name> --dataset=<ds> --from=<vol> --to=<vol> --type=rsync`
- **Actual:** `--sync-type=rsync` (not `--type`)

### PRIME-03: `sp decision update --status=open`
- **Prime says:** `sp decision update "NAS Upgrade 2026" --status=open`
- **Actual:** `--open` flag (not `--status=open`)

### PRIME-04: `sp decision compare` doesn't exist
- **Prime says:** `sp decision compare "NAS Upgrade 2026"`
- **Actual:** This subcommand doesn't exist. Comparison is done via `sp analyze compare <topo-a> <topo-b>` and `sp analyze constraints --decision=<dec>`

### PRIME-05: `sp decision choose --topology=<winner>`
- **Prime says:** `sp decision choose "NAS Upgrade 2026" --topology=<winner> --rationale="..."`
- **Actual:** Topology is a positional arg: `sp decision choose <DECISION> <TOPOLOGY> --rationale="..."`

---

## Analysis Correctness

### ANLZ-01: Capacity analysis treats dataset size as per-volume footprint

**Severity:** Medium — false warnings

Capacity projection sums all placed dataset sizes as "used bytes" per volume. If a 2TB dataset is placed on a 2TB volume alongside a 100GB dataset, the analysis reports 2.1TB used / 2.0TB capacity = "full in 0 months" — even though the volume may have plenty of actual space.

The `size_bytes` on a dataset represents the *total* dataset size, not how much space it occupies on each individual volume. A replica may be smaller (deduplicated) or larger (with snapshots).

**Repro:**
```bash
sp volume add small-vol --node=n1 --capacity=2TB
sp dataset add big-ds --size=1.5TB
sp dataset add small-ds --size=800GB
sp placement add big-ds small-vol --role=primary
sp placement add small-ds small-vol --role=primary
sp analyze capacity              # "full in 0 months (2.3TB/2.0TB)"
```

### ANLZ-02: `$-0.00` in compare output

**Severity:** Low — cosmetic

Topologies with no catalog item links show `$-0.00` instead of `$0.00` in `sp analyze compare` cost column. Negative zero formatting.

---

## UX Issues

### UX-01: Price amount requires cents, not dollars

`sp catalog price add --amount=549.99` fails with "invalid digit found in string." The help text says "in cents" but every user will try dollars first. The flag should either accept decimal dollars or be renamed `--cents`.

### UX-02: Negative numeric values accepted without validation

`--power-draw=-100` stores -100W and is summed into totals, effectively reducing total power consumption. `--cost=-500` creates negative costs. `--noise=-10` creates negative noise. No fields validate for non-negative values.

**Impact:** `sp analyze constraints --decision=X` produces wrong power/noise totals. `sp analyze compare` shows wrong comparative metrics.

### UX-03: Zero and negative capacity accepted silently

- `--capacity=0GB` creates a volume with `0B` capacity
- `--capacity=-1TB` silently parses to `0B` (no error)
- Zero-capacity volumes break capacity analysis (division edge cases)

### UX-04: Dataset update "nothing to update" message doesn't list available flags

Node and volume update helpfully list all available flags when called with no changes:
```
Error: Nothing to update. Provide --rename, --role, --location, --bays, --interface-types, --power-draw, --cost, --noise, --rack-units, or --item-id.
```

Dataset update just says:
```
Error: Nothing to update. Provide at least one field to change.
```

### UX-05: 11 dead-code compiler warnings

`cargo build` produces 11 warnings for unused functions, structs, and fields. Not user-facing but noisy during development. Includes: `conn_mut`, `open_memory`, `is_initialized`, `path`, `CURRENT_VERSION`, `Event::to_json`, `parse_spec`, `get_capacity`, `get_read_speed`, `get_write_speed`, `get_noise`, `NoiseLevel`.

### UX-06: Clippy warnings for format string literals

5 clippy warnings about `print_literal` and `format_in_format_args` in `analyze.rs`, `catalog.rs`, and `status.rs`. The literal strings in format macros could be inlined.

---

## Undo/Redo Edge Cases

### UNDO-01: Redo fails for import and multi-entity creation events

(From prior ASSESSMENT.md) `sp redo` after undoing a topology import fails with `Error: No after_state for created event`. The import creates entities but the event's `after_state` may not capture everything needed for redo.

### UNDO-02: Failed undo leaves system in stuck state

When undo fails (e.g., BUG-03 FK error), the undo pointer doesn't move — but the user has no way to skip past the problematic event. Every subsequent `sp undo` hits the same error. There's no `sp undo --skip` or `sp undo --force`.

---

## Schema Issues

### SCHEMA-01: `topologies.parent_id` FK missing ON DELETE action

```sql
parent_id TEXT REFERENCES topologies(id)
```

Should be:
```sql
parent_id TEXT REFERENCES topologies(id) ON DELETE SET NULL
```

This blocks undo (BUG-03) and would also block `sp topology delete` on a parent topology if any forks exist.

### SCHEMA-02: Template export retains tag

(From prior ASSESSMENT.md) `sp export --template` strips UUIDs but keeps `tag: current`. An imported template shouldn't inherit the "current" tag from the source.

---

## Summary by Severity

| Severity | Count | IDs |
|----------|-------|-----|
| Critical | 1 | BUG-01 |
| High | 2 | BUG-02, BUG-03 |
| Medium | 1 | ANLZ-01 |
| Low | 16 | PRIME-01–05, ANLZ-02, UX-01–06, UNDO-01–02, SCHEMA-01–02 |

### Recommended Fix Order

1. **BUG-01 + BUG-02 + BUG-03** — Undo system rewrite. Change `.updated` undo from delete+insert to field-level UPDATE. Record cascaded deletions as sub-events for `.deleted` undo. Add `ON DELETE SET NULL` to `parent_id`.
2. **PRIME-01–05** — Fix remaining 5 wrong examples in `STATIC_GUIDE` string.
3. **UX-02 + UX-03** — Add non-negative validation for power, cost, noise, capacity.
4. **Everything else** — Cosmetic/convenience.
