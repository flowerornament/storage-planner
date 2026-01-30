# Analysis Algorithms

All analysis functions are pure: `(Topology, Catalogs) → Results`

Located in `src/storage_planner/analysis/`.

## Redundancy Analysis

**File:** `redundancy.py`
**Command:** `sp analyze redundancy`

Checks if datasets meet copy and location requirements.

**Algorithm:**
1. For each dataset:
   - Find all volumes hosting it (from `stored_on` + `volume.hosts_datasets`)
   - Count unique copies
   - Map volumes to node locations, count unique locations
   - Compare against `required_copies` and `required_locations`
   - For critical data, also check global constraints

**Output:** `RedundancyResult` per dataset with pass/fail status.

## RPO/RTO Analysis

**File:** `rpo_rto.py`
**Command:** `sp analyze rpo-rto`

Checks if sync regimes meet recovery point objectives.

**Algorithm:**
1. For each dataset:
   - Find all sync regimes targeting this dataset
   - Get best `achieved_rpo` (smallest duration)
   - Continuous regimes assumed ~1 minute RPO
   - Compare against `max_rpo`

**Output:** `RpoRtoResult` with achieved vs required RPO.

## Bandwidth Analysis

**File:** `bandwidth.py`
**Command:** `sp analyze bandwidth`

Estimates transfer times and identifies bottlenecks.

**Algorithm:**
1. For each sync regime:
   - Find source and target nodes via volume mapping
   - Find link between nodes
   - Calculate: `transfer_time = dataset_size / bandwidth`
   - Flag as bottleneck if transfer > 1 hour

**Limitations:**
- Assumes direct links (no multi-hop routing)
- Uses raw bandwidth (no overhead calculation)
- Full sync time (not incremental)

**Output:** `BandwidthResult` with estimated sync time per regime.

## Capacity Analysis

**File:** `capacity.py`
**Command:** `sp analyze capacity [--months N]`

Projects future capacity usage based on growth rates.

**Algorithm:**
1. For each volume:
   - Sum current size of hosted datasets
   - Parse growth rates (absolute or percentage)
   - Project: `future = current + (monthly_growth × months)`
   - Calculate utilization percentage
   - Estimate months until full

**Output:** `CapacityResult` with projections and warnings.

## Cost Analysis

**File:** `cost.py`
**Command:** `sp cost`

Calculates operational and hardware costs.

**Algorithm:**
1. For each node:
   - Add `monthly_cost` (hosting)
   - Calculate power cost: `(watts × hours × $/kWh) / 1000`
   - Sum volume `purchase_cost` or lookup from catalog
2. Total monthly = sum of hosting + power
3. 5-year projection = hardware + (monthly × 60)

**Inputs:**
- `--power-cost`: $/kWh (default 0.12)
- `--catalog`: Directory for hardware price lookups

## Failure Simulation

**File:** `failure_sim.py`
**Command:** `sp simulate <node|volume>`

Analyzes impact of losing a node or volume.

**Algorithm:**
1. Identify affected volumes (all volumes on failed node, or single volume)
2. For each dataset with copies on affected volumes:
   - Count remaining copies
   - Identify recovery sources (remaining volumes)
   - Check if recoverable (any copies left)
   - Flag critical data at risk

**Output:** `FailureSimResult` with:
- Affected datasets
- Recovery paths
- Data loss risk assessment

## Adding New Analysis

1. Create `src/storage_planner/analysis/new_analysis.py`
2. Define result dataclass
3. Implement pure function: `analyze_X(topology, ...) → list[Result]`
4. Add CLI in `src/storage_planner/cli/analyze.py` or new command file
5. Document here

Keep analysis functions pure—no side effects, no state.
