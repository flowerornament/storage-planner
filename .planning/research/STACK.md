# Technology Stack

**Project:** Storage Planner (sp) - CLI for AI-assisted purchase decisions
**Researched:** 2026-02-06
**Mode:** Brownfield - extending existing Rust CLI

## Existing Stack (Keep)

The project already has a solid foundation. These dependencies should remain:

| Technology | Current Version | Purpose | Notes |
|------------|-----------------|---------|-------|
| clap | 4 | CLI parsing | Already using derive features; current version is 4.5.57 |
| rusqlite | 0.31 | SQLite database | Current version is 0.38.0 - consider upgrading |
| serde | 1 | Serialization | Standard, battle-tested |
| serde_json | 1 | JSON handling | Used for specs, metadata, structured output |
| serde_yaml | 0.9 | YAML support | Useful for config files |
| chrono | 0.4 | Time handling | Already in use for timestamps |
| uuid | 1 | ID generation | v4 UUIDs for entities |
| anyhow | 1 | Error handling | Ergonomic error propagation |
| console | 0.15 | Terminal output | Basic terminal styling |
| ureq | 2 | HTTP client | For pricing APIs |
| camino | 1 | UTF-8 paths | Clean path handling |
| fs-err | 3 | Better fs errors | Improved error messages |

**Confidence:** HIGH - verified from existing Cargo.toml

## Recommended Additions

### Graph Modeling: petgraph

**Version:** 0.8.3 (verified via [docs.rs](https://docs.rs/petgraph/latest/petgraph/))

| Aspect | Details |
|--------|---------|
| Purpose | Topology graph modeling (nodes, volumes, datasets, connections) |
| Why | De facto Rust graph library, 10+ years mature, comprehensive algorithms |
| Features to enable | `serde-1` for persistence |

**Key capabilities:**
- `StableGraph` - indices remain stable across node/edge removals (critical for persistence)
- Built-in BFS, DFS, Dijkstra, A*, topological sort
- Serialization preserves node/edge indices
- Arbitrary data on nodes and edges

**Usage pattern:**
```rust
use petgraph::stable_graph::StableGraph;

// Topology is a graph with typed nodes and edges
type Topology = StableGraph<TopologyNode, Connection>;

#[derive(Serialize, Deserialize)]
enum TopologyNode {
    Device { id: String, specs: DeviceSpecs },
    Volume { id: String, raid_level: RaidLevel },
    Dataset { id: String, size_gb: u64 },
}

#[derive(Serialize, Deserialize)]
struct Connection {
    port: Option<String>,
    speed_gbps: Option<f64>,
}
```

**Persistence approach:**
1. Serialize graph to JSON with `serde_json::to_string(&topology)`
2. Store as TEXT in SQLite `configurations.domain_data` column (already exists)
3. Deserialize on load - indices remain valid

**Confidence:** HIGH - verified via [petgraph docs](https://docs.rs/petgraph/latest/petgraph/) and [GitHub](https://github.com/petgraph/petgraph)

### Table Output: tabled

**Version:** 0.20.0 (verified via [docs.rs](https://docs.rs/tabled/latest/tabled/))

| Aspect | Details |
|--------|---------|
| Purpose | Pretty-print tables for human-readable CLI output |
| Why | Derive macro integration, extensive customization, maintained |
| Alternative | console (already have it) for simpler cases |

**Usage pattern:**
```rust
use tabled::{Table, Tabled};

#[derive(Tabled)]
struct ConfigSummary {
    name: String,
    #[tabled(rename = "Total GB")]
    capacity_gb: u64,
    #[tabled(rename = "Cost")]
    total_cost: String,
}

let table = Table::new(configs).to_string();
```

**When to use:**
- `tabled` for structured data (lists of items, comparisons)
- `console` for simple styling (colors, bold)
- `serde_json` for machine-readable output (AI context dumps)

**Confidence:** HIGH - verified via [tabled docs](https://docs.rs/tabled/latest/tabled/)

### Validation: garde

**Version:** 0.22.1 (verified via [docs.rs](https://docs.rs/garde/latest/garde/))

| Aspect | Details |
|--------|---------|
| Purpose | Constraint validation for topology rules |
| Why | Rewrite of validator with cleaner API, better error messages |
| Alternative | validator 0.20.0 - older, more widely used but less ergonomic |

**Usage pattern:**
```rust
use garde::Validate;

#[derive(Validate)]
struct RaidConfig {
    #[garde(range(min = 2))]
    min_drives: u32,

    #[garde(range(min = 0.0, max = 1.0))]
    parity_ratio: f64,

    #[garde(custom(validate_raid_level))]
    level: RaidLevel,
}

fn validate_raid_level(level: &RaidLevel, _: &()) -> garde::Result {
    match level {
        RaidLevel::Raid0 { drives } if *drives < 2 => {
            Err(garde::Error::new("RAID 0 requires at least 2 drives"))
        }
        _ => Ok(())
    }
}
```

**Why garde over validator:**
- Cleaner derive syntax
- Better compile-time error messages
- More flexible custom validation
- Active development (validator is slower to update)

**Confidence:** MEDIUM - garde is newer; validator is more battle-tested. Either works.

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Graph library | petgraph | graphina | graphina requires Rust 1.86+, less mature |
| Graph library | petgraph | custom impl | Reinventing the wheel; petgraph is comprehensive |
| Graph persistence | JSON in SQLite | GraphLite | Overkill for embedded use; adds query language complexity |
| Graph persistence | JSON in SQLite | Separate graph DB | Violates "database is truth" principle; adds operational complexity |
| Validation | garde | validator | validator 0.20.0 works but garde has cleaner API |
| Validation | garde | custom | Validation logic is tricky; use proven library |
| Table output | tabled | prettytable-rs | prettytable-rs is older (2020); tabled is actively maintained |
| Table output | tabled | comfy-table | Both good; tabled has better derive support |

## What NOT to Add

| Library | Why Not |
|---------|---------|
| diesel/sqlx | rusqlite is simpler for single-user CLI; async not needed |
| tokio/async-std | CLI is synchronous; async adds complexity without benefit |
| event sourcing crates | Existing append-only events table is sufficient; full ES is overkill |
| graphlite/cqlite | Embedded graph DBs add query language; petgraph + SQLite is simpler |
| sled | SQLite is more portable and debuggable; sled has durability concerns |

## Cargo.toml Additions

```toml
# Graph modeling
petgraph = { version = "0.8", features = ["serde-1"] }

# Table output
tabled = { version = "0.20", features = ["derive"] }

# Validation (choose one)
garde = { version = "0.22", features = ["derive"] }
# OR: validator = { version = "0.20", features = ["derive"] }
```

## Version Upgrade Recommendations

| Dependency | Current | Latest | Priority |
|------------|---------|--------|----------|
| rusqlite | 0.31 | 0.38.0 | MEDIUM - breaking changes possible |
| clap | 4 | 4.5.57 | LOW - minor version, compatible |

**Note:** Test thoroughly after rusqlite upgrade - API may have changed.

## Architectural Implications

### Graph + SQLite Pattern

The codebase already stores complex data as JSON in SQLite columns (`specs`, `domain_data`, `metadata`). Extend this pattern for topologies:

```
configurations.domain_data = {
    "topology": <petgraph serialized>,
    "constraints": [...],
    "analysis_cache": {...}
}
```

**Benefits:**
- Single source of truth (existing principle)
- Atomic transactions via rusqlite
- No new persistence technology to learn
- Graph can be loaded into memory for analysis

**Tradeoffs:**
- No graph-native queries (must load entire graph)
- JSON parsing overhead on load
- Suitable for graphs < 10K nodes (sufficient for storage configs)

### Versioning Strategy

For versioned topologies (track changes over time):

1. **Option A: Append-only snapshots** (RECOMMENDED)
   - Each topology change creates new version
   - Store version_id, parent_version_id, timestamp
   - Existing events table can track changes

2. **Option B: Event sourcing**
   - Store topology mutations as events
   - Rebuild topology by replaying events
   - More complex, rarely needed for this use case

Recommendation: Start with Option A. The existing `events` table already provides audit history.

## AI-Friendly Output

The codebase already uses `serde_json` for structured data. For AI context dumps (`sp prime`):

```rust
#[derive(Serialize)]
struct PrimeContext {
    items: Vec<Item>,
    configurations: Vec<Configuration>,
    decisions: Vec<Decision>,
    analysis: AnalysisSummary,
}

// Output as pretty JSON for AI consumption
println!("{}", serde_json::to_string_pretty(&context)?);
```

**No additional libraries needed** - serde_json handles this well.

## Sources

- [petgraph 0.8.3 documentation](https://docs.rs/petgraph/latest/petgraph/) - HIGH confidence
- [petgraph serde serialization](https://github.com/petgraph/petgraph/blob/master/src/graph_impl/serialization.rs) - HIGH confidence
- [rusqlite 0.38.0 documentation](https://docs.rs/rusqlite/latest/rusqlite/) - HIGH confidence
- [tabled 0.20.0 documentation](https://docs.rs/tabled/latest/tabled/) - HIGH confidence
- [garde 0.22.1 documentation](https://docs.rs/garde/latest/garde/) - HIGH confidence
- [validator 0.20.0 documentation](https://docs.rs/validator/latest/validator/) - HIGH confidence
- [clap 4.5.57 documentation](https://docs.rs/clap/latest/clap/) - HIGH confidence
- [Pathfinding algorithms in Rust](https://blog.logrocket.com/pathfinding-rust-tutorial-examples/) - MEDIUM confidence
