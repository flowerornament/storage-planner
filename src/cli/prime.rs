//! sp prime -- AI agent bootstrap document (CTX-02)
//!
//! Outputs a static instructional document with workflow guide and concrete
//! example commands, followed by a dynamically generated state summary.
//! Designed as the first command an AI agent runs to understand the system.

use anyhow::Result;

use crate::core::db::Database;
use crate::core::resolve::resolve_active_topology;

/// Static instructional content for the agent bootstrap document.
const STATIC_GUIDE: &str = r#"# Storage Planner -- Agent Bootstrap

Run `sp status` for a full health overview of the current system state.

## Core Workflow

### 1. Explore State

```
sp status                        # Health dashboard (problems, topology, decisions, catalog)
sp status --format=json          # Machine-readable health dashboard
sp current                       # Show current topology
sp topology list                 # List all topologies
sp topology show <name>          # Show topology details
sp topology show <name> --tree   # Hierarchical view of nodes/volumes
sp diagram --tree                # ASCII tree diagram
sp diagram --network             # ASCII network diagram
```

### 2. Work with Topologies

```
sp topology create <name> --description="..."   # Create new topology
sp topology fork <source> --name=<new-name>     # Fork for comparison
sp current <name>                               # Switch active topology
sp topology tag <name> exploring                # Tag lifecycle state
sp topology diff <target> [base]                # Compare two topologies
sp export <topology>                            # Export to YAML
sp import <file.yaml>                           # Import from YAML
```

### 3. Build Topology Content

```
sp node add <name> --role=<desktop|server|nas|cloud> --location=<loc>
sp volume add <name> --node=<node> --capacity=4TB
sp dataset add <name> --size=500GB --criticality=critical --min-copies=3
sp placement add <ds> <vol> --role=primary
sp link add --from=<source-node> --to=<target-node> --connection-type=lan --bandwidth=1GB/s
sp sync add <name> --dataset=<ds> --from=<vol> --to=<vol> --sync-type=rsync
```

### 4. Analyze Options

```
sp analyze                                      # Combined dashboard (all reports)
sp analyze redundancy                           # Check copy/location requirements
sp analyze capacity                             # Project time-until-full
sp analyze rpo                                  # Check sync schedule compliance
sp analyze failure <node1> [node2]              # Simulate node failure
sp analyze bandwidth                            # Check link capacity for syncs
sp analyze cost                                 # One-time + recurring costs
sp analyze cost --tco=3yr                       # Total cost of ownership
sp analyze compare <topo-a> <topo-b>            # Side-by-side comparison
sp analyze constraints --decision=<dec>         # Check budget/power/noise limits
```

### 5. Track Decisions

```
sp decision create "NAS Upgrade 2026"
sp decision update "NAS Upgrade 2026" --open
sp decision consider "NAS Upgrade 2026" <topology>
sp decision constrain "NAS Upgrade 2026" --type=budget --max=1500
sp analyze constraints --decision="NAS Upgrade 2026"
sp decision choose "NAS Upgrade 2026" <winner> --rationale="..."
sp decision show "NAS Upgrade 2026"
sp decision list
```

### 6. Manage Catalog

```
sp catalog add "Samsung 870 EVO 4TB" --category=ssd --specs='{"capacity_gb":4000}'
sp catalog price add "Samsung 870 EVO 4TB" --amount=289.99 --source=amazon
sp catalog show "Samsung 870 EVO 4TB"
sp catalog list
sp catalog search "870 EVO"
sp catalog price list "Samsung 870 EVO 4TB"
```

## Output Formats

Most commands support `--format=json` for machine-readable output:

```
sp status --format=json
sp topology list --format=json
sp analyze --format=json
```

## Key Concepts

- **Database is truth**: All data in `.sp/decisions.db`; always use `sp` commands
- **Topologies**: Named storage configurations (nodes -> volumes -> datasets -> placements)
- **Tags**: Lifecycle states -- `current` (active), `exploring`, `archived`
- **Decisions**: Track purchase choices with constraints, topology comparisons, rationale
- **Catalog**: Products under consideration with price history
- **Analysis**: Redundancy, capacity, RPO, failure simulation, bandwidth, cost
- **Events**: All mutations logged for undo/redo support
"#;

/// Run the prime command: print agent bootstrap document.
pub fn run(db: &mut Database) -> Result<()> {
    // Print static guide
    print!("{}", STATIC_GUIDE);

    // Print dynamic state summary
    println!("---");
    println!();
    println!("## Current State");
    println!();

    // Current topology
    let current_topo = resolve_active_topology(db, None).ok();
    match &current_topo {
        Some(topo) => {
            println!("**Current topology:** {}", topo.name);
        }
        None => {
            println!("**Current topology:** None set");
        }
    }

    // Topology counts by tag
    let total_topos: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM topologies", [], |row| row.get(0))?;

    if total_topos > 0 {
        let current_count: i64 = db.conn().query_row(
            "SELECT COUNT(*) FROM topologies WHERE tag = 'current'",
            [],
            |row| row.get(0),
        )?;
        let exploring_count: i64 = db.conn().query_row(
            "SELECT COUNT(*) FROM topologies WHERE tag = 'exploring'",
            [],
            |row| row.get(0),
        )?;
        let archived_count: i64 = db.conn().query_row(
            "SELECT COUNT(*) FROM topologies WHERE tag = 'archived'",
            [],
            |row| row.get(0),
        )?;
        let untagged_count: i64 = db.conn().query_row(
            "SELECT COUNT(*) FROM topologies WHERE tag IS NULL",
            [],
            |row| row.get(0),
        )?;

        print!("**Topologies:** {} total", total_topos);
        let mut parts = Vec::new();
        if current_count > 0 {
            parts.push(format!("{} current", current_count));
        }
        if exploring_count > 0 {
            parts.push(format!("{} exploring", exploring_count));
        }
        if archived_count > 0 {
            parts.push(format!("{} archived", archived_count));
        }
        if untagged_count > 0 {
            parts.push(format!("{} untagged", untagged_count));
        }
        if !parts.is_empty() {
            print!(" ({})", parts.join(", "));
        }
        println!();
    } else {
        println!("**Topologies:** None");
    }

    // Open decisions
    let open_decisions: Vec<String> = {
        let mut stmt = db.conn().prepare(
            "SELECT title FROM decisions WHERE status IN ('draft', 'open') ORDER BY created_at",
        )?;
        let result = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        result
    };

    if open_decisions.is_empty() {
        println!("**Open decisions:** None");
    } else {
        println!(
            "**Open decisions:** {} -- {}",
            open_decisions.len(),
            open_decisions.join(", ")
        );
    }

    // Catalog stats
    let item_count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM catalog_items", [], |row| row.get(0))?;
    let price_count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM prices", [], |row| row.get(0))?;

    println!(
        "**Catalog:** {} items, {} price observations",
        item_count, price_count
    );

    // Problems (brief)
    let problems = gather_brief_problems(db)?;
    if problems.is_empty() {
        println!("**Problems:** None");
    } else {
        println!("**Problems:**");
        for p in &problems {
            println!("  - {}", p);
        }
    }

    Ok(())
}

/// Gather brief problem descriptions for the state summary.
fn gather_brief_problems(db: &mut Database) -> Result<Vec<String>> {
    let mut problems = Vec::new();

    // Stale decisions
    let stale: i64 = db.conn().query_row(
        "SELECT COUNT(*) FROM decisions
         WHERE status IN ('draft', 'open')
         AND julianday('now') - julianday(created_at) >= 30",
        [],
        |row| row.get(0),
    )?;
    if stale > 0 {
        problems.push(format!("{} decision(s) open 30+ days", stale));
    }

    // Basic redundancy check on current topology
    if let Ok(topo) = resolve_active_topology(db, None) {
        use crate::core::models::{Dataset, Volume};
        use crate::domains::storage::analysis::{
            analyze_capacity, analyze_redundancy, load_placements_with_context,
        };

        let datasets = Dataset::load_for_topology(db, &topo.id)?;

        if !datasets.is_empty() {
            let placements = load_placements_with_context(db, &topo.id)?;
            let redundancy = analyze_redundancy(&datasets, &placements);
            if !redundancy.issues.is_empty() {
                problems.push(format!(
                    "{} dataset(s) with redundancy issues",
                    redundancy.issues.len()
                ));
            }

            let volumes = Volume::load_for_topology(db, &topo.id)?;

            let capacity = analyze_capacity(&datasets, &volumes, &placements, 6);
            if !capacity.issues.is_empty() {
                problems.push(format!(
                    "{} volume(s) projected full within 6 months",
                    capacity.issues.len()
                ));
            }
        }
    }

    Ok(problems)
}
