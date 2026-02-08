---
phase: 07-cli-polish-and-correctness
verified: 2026-02-08T18:30:00Z
status: passed
score: 11/11 must-haves verified
re_verification: false
---

# Phase 7: CLI Polish and Correctness Verification Report

**Phase Goal:** Fix command correctness issues discovered in post-v1 testing. See `.planning/phases/07-cli-polish-and-correctness/ASSESSMENT.md` for full assessment with reproduction steps.

**Verified:** 2026-02-08T18:30:00Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #   | Truth                                                                                         | Status     | Evidence                                                                                  |
| --- | --------------------------------------------------------------------------------------------- | ---------- | ----------------------------------------------------------------------------------------- |
| 1   | sp prime shows correct syntax for placement add, link add, sync add, decision consider, constrain | ✓ VERIFIED | All 5 commands verified in prime output with correct syntax                              |
| 2   | sp node add and sp node update accept --item-id flag that links to a catalog item             | ✓ VERIFIED | Both commands show --item-id in --help, accept flag, resolve item, store UUID            |
| 3   | sp volume add and sp volume update accept --item-id flag that links to a catalog item         | ✓ VERIFIED | Both commands show --item-id in --help, accept flag, resolve item, store UUID            |
| 4   | Item ID is resolved via resolve_catalog_item before storage (stores UUID, not raw input)      | ✓ VERIFIED | resolve_catalog_item called before transaction in all 4 commands, UUID stored             |
| 5   | Event after_state captures item_id for undo/redo fidelity                                    | ✓ VERIFIED | to_json() called after setting item_id in add(), after-state builder includes it in update() |
| 6   | Duplicate topology names return friendly error instead of raw SQLite constraint violation      | ✓ VERIFIED | Returns "Topology 'X' already exists" not "UNIQUE constraint failed"                      |
| 7   | Duplicate node names (within topology) return friendly error                                  | ✓ VERIFIED | Returns "Node 'X' already exists in topology 'Y'" not raw SQLite error                    |
| 8   | Duplicate volume names (within node) return friendly error                                    | ✓ VERIFIED | Returns "Volume 'X' already exists on node 'Y'" not raw SQLite error                      |
| 9   | Duplicate dataset names (within topology) return friendly error                               | ✓ VERIFIED | Returns "Dataset 'X' already exists in topology 'Y'" not raw SQLite error                 |
| 10  | Duplicate catalog item names return friendly error                                            | ✓ VERIFIED | Returns "Catalog item 'X' already exists" not raw SQLite error                            |
| 11  | sp catalog list output includes column headers (Name, Category, URL, Latest Price)            | ✓ VERIFIED | Headers and separator line present, align with data columns                               |

**Score:** 11/11 truths verified

### Required Artifacts

| Artifact                         | Expected                                            | Status      | Details                                                                                          |
| -------------------------------- | --------------------------------------------------- | ----------- | ------------------------------------------------------------------------------------------------ |
| `src/cli/prime.rs`               | Corrected STATIC_GUIDE command examples            | ✓ VERIFIED  | Contains "sp placement add <ds> <vol>" and 4 other corrected commands (lines 50-52, 75-76)       |
| `src/cli/node.rs`                | Node add/update with --item-id support             | ✓ VERIFIED  | resolve_catalog_item imported, --item-id in Add/Update enums, resolution in add()/update()       |
| `src/cli/volume.rs`              | Volume add/update with --item-id support           | ✓ VERIFIED  | resolve_catalog_item imported, --item-id in Add/Update enums, resolution in add()/update()       |
| `src/cli/topology.rs`            | Pre-insert uniqueness check for topology create    | ✓ VERIFIED  | SELECT COUNT query at line ~185, "already exists" bail at line ~189                              |
| `src/cli/node.rs`                | Pre-insert uniqueness check for node add           | ✓ VERIFIED  | SELECT COUNT query for (topology_id, name) at line ~233, "already exists" bail at line ~239      |
| `src/cli/volume.rs`              | Pre-insert uniqueness check for volume add         | ✓ VERIFIED  | SELECT COUNT query for (topology_id, node_id, name) at line ~235, "already exists" bail at ~241  |
| `src/cli/dataset.rs`             | Pre-insert uniqueness check for dataset add        | ✓ VERIFIED  | SELECT COUNT query for (topology_id, name), "already exists" bail message                        |
| `src/cli/catalog.rs`             | Pre-insert uniqueness check for catalog add        | ✓ VERIFIED  | SELECT COUNT query for name, "already exists" bail message                                       |
| `src/cli/catalog.rs`             | List headers in list() function                    | ✓ VERIFIED  | Headers "Name, Category, URL, Latest Price" with separator line at ~327-329                      |

### Key Link Verification

| From                  | To                              | Via                            | Status     | Details                                                                                   |
| --------------------- | ------------------------------- | ------------------------------ | ---------- | ----------------------------------------------------------------------------------------- |
| src/cli/node.rs       | src/core/resolve.rs             | resolve_catalog_item call      | ✓ WIRED    | Imported at line ~10, called at line ~244 (add) and ~564 (update)                         |
| src/cli/volume.rs     | src/core/resolve.rs             | resolve_catalog_item call      | ✓ WIRED    | Imported at line ~10, called at line ~246 (add) and ~564 (update)                         |
| node.rs add()         | Node.item_id field              | UUID assignment                | ✓ WIRED    | resolved_item_id assigned to node.item_id at line ~262, before to_json()                  |
| node.rs update()      | after_state builder             | item_id capture                | ✓ WIRED    | after.item_id set at line ~600, before to_json() at line ~602                             |
| node.rs update()      | UPDATE nodes SET item_id        | SQL execution in transaction   | ✓ WIRED    | SQL UPDATE at line ~668, uses resolved_item_id                                            |
| volume.rs add()       | Volume.item_id field            | UUID assignment                | ✓ WIRED    | resolved_item_id assigned to vol.item_id at line ~257, before to_json()                   |
| volume.rs update()    | after_state builder             | item_id capture                | ✓ WIRED    | after.item_id set in update(), before to_json()                                           |
| volume.rs update()    | UPDATE volumes SET item_id      | SQL execution in transaction   | ✓ WIRED    | SQL UPDATE uses resolved_item_id                                                          |
| topology.rs create()  | topologies table UNIQUE(name)   | pre-check SELECT COUNT         | ✓ WIRED    | Query at line ~185, checks name before insert, bails if count > 0                         |
| catalog.rs add()      | catalog_items table UNIQUE(name)| pre-check SELECT COUNT         | ✓ WIRED    | Query checks name before insert, bails with friendly message if count > 0                 |

### Requirements Coverage

Phase 7 addresses the 4 priority fixes from ASSESSMENT.md:

| Requirement                                       | Status      | Notes                                                               |
| ------------------------------------------------- | ----------- | ------------------------------------------------------------------- |
| Fix 6 wrong command examples in sp prime          | ✓ SATISFIED | Actually 5 commands (plan corrected count), all fixed               |
| Add --item-id flag to node/volume update          | ✓ SATISFIED | Added to both add and update for node and volume (4 commands total) |
| Friendly duplicate name errors (not SQLite raw)   | ✓ SATISFIED | All 5 entity creation commands have pre-checks with friendly errors |
| Add column headers to sp catalog list output      | ✓ SATISFIED | Headers with separator line added, align with data columns          |

### Anti-Patterns Found

No anti-patterns found.

**Scan Results:**
- TODO/FIXME/placeholder comments: 0 instances in modified files
- Empty implementations: 0 instances
- Console.log only implementations: 0 instances
- Stub patterns: 0 instances

**Build/Test Results:**
- `cargo build`: Compiles successfully (11 warnings about unused code, unrelated to changes)
- `cargo test`: 97 tests passed, 0 failed
- Runtime tests: All 11 manual verification tests passed

### Human Verification Required

None. All observable truths can be verified programmatically via CLI execution and code inspection.

---

## Verification Summary

**All 11 must-haves verified.** Phase 7 goal achieved.

### What Was Verified

**Plan 07-01 (Prime Corrections + Item-ID Linking):**
1. All 5 command examples in sp prime corrected to match actual CLI syntax
2. --item-id flag added to node add/update (2 commands)
3. --item-id flag added to volume add/update (2 commands)
4. Item resolution via resolve_catalog_item validates existence before storage
5. Resolved UUID stored (not raw user input)
6. Event after_state captures item_id for undo/redo fidelity

**Plan 07-02 (Uniqueness Pre-checks + Catalog Headers):**
7. Topology create returns friendly "already exists" error
8. Node add returns friendly "already exists in topology" error
9. Volume add returns friendly "already exists on node" error
10. Dataset add returns friendly "already exists in topology" error
11. Catalog add returns friendly "already exists" error
12. Catalog list includes column headers with separator line

### Testing Evidence

**Prime Command Verification:**
```bash
$ sp prime | grep -E "placement add|link add|sync add|decision consider|decision constrain"
sp placement add <ds> <vol> --role=primary
sp link add --from=<source-node> --to=<target-node> --type=lan --bandwidth=1GB/s
sp sync add <name> --dataset=<ds> --from=<vol> --to=<vol> --type=rsync
sp decision consider "NAS Upgrade 2026" <topology>
sp decision constrain "NAS Upgrade 2026" --type=budget --max=1500
```

**Item-ID Flag Verification:**
```bash
$ sp node add --help | grep item-id
      --item-id <ITEM_ID>  Link to a catalog item (name or ID prefix)

$ sp node add testnode --role=nas --item-id="Test Item"
Created node 'testnode' (id: 2e88fa4b)

$ sp node show testnode --format=json | grep item
  "item_id": "a3210e66-4be0-4f37-ada7-036962f5fc17",
```

**Uniqueness Pre-check Verification:**
```bash
$ sp catalog add "Test Item" --category=ssd
Added item: Test Item (a3210e66)

$ sp catalog add "Test Item" --category=ssd
Error: Catalog item 'Test Item' already exists
```

**Catalog Headers Verification:**
```bash
$ sp catalog list
  Name                           Category     URL                                        Latest Price
  ------------------------------------------------------------------------------------------
  Test Item                      ssd          -                                          -
```

**Item Resolution Validation:**
```bash
$ sp node add badnode --role=desktop --item-id="nonexistent"
Error: Catalog item 'nonexistent' not found
```

### Critical Implementation Details

**D009 Pattern Compliance:**
- resolve_catalog_item called BEFORE transaction in all 4 commands (node add/update, volume add/update)
- Avoids connection conflicts from nested queries

**Event System Fidelity:**
- item_id set on entity BEFORE calling to_json() in add functions
- after-state builder explicitly sets item_id in update functions
- Ensures undo/redo captures the catalog item linkage

**Pre-check Pattern Consistency:**
- All 5 entity creation commands use identical pattern: SELECT COUNT(*), bail if > 0
- Matches exact UNIQUE constraint columns for each table
- Only checks UNIQUE constraints (not FK constraints, per research pitfall 5)

**Header Format Consistency:**
- Catalog list headers match catalog price list pattern (headers + separator)
- Column widths align with existing data row format
- JSON output path unchanged (headers only in text mode)

---

_Verified: 2026-02-08T18:30:00Z_
_Verifier: Claude (gsd-verifier)_
