# Analysis Reference

The `sp analyze` command runs checks on configurations to identify issues.

## Running Analysis

```bash
sp analyze                            # Analyze current configuration
sp analyze "SATA Option"              # Analyze specific config
sp analyze --check=redundancy,cost    # Run specific checks
sp analyze --format=json              # JSON output
```

## Available Checks

### cost

Validates pricing data completeness.

**Pass:** All items have prices
**Warn:** Some items missing prices
**Details:**
- `total` - Total configuration cost
- `items_with_price` - Count of priced items
- `items_without_price` - Count of unpriced items

### capacity

Calculates total storage capacity from item specs.

**Pass:** Capacity data available
**Warn:** No capacity data found
**Details:**
- `total` - Total capacity (human-readable)
- `total_bytes` - Total capacity in bytes
- `items` - List of items with capacity

Requires `capacity` in item specs (e.g., `"capacity": "4TB"`).

### noise

Checks noise levels against threshold (default: 30 dB for quiet operation).

**Pass:** Max noise within threshold
**Warn:** Max noise exceeds threshold, or no noise data
**Details:**
- `max_noise_db` - Maximum noise level in configuration
- `threshold_db` - Target threshold
- `within_threshold` - Boolean
- `items` - List of items with noise data

Requires `noise_db` in item specs.

### redundancy

Checks for data protection.

**Pass:** 2+ storage devices (mirror possible) or 3+ (RAID5+ possible)
**Fail:** Single storage device (no redundancy)
**Warn:** No storage devices found
**Details:**
- `storage_device_count` - Number of storage items
- `storage_items` - List of storage item IDs
- `domain_data` - Domain-specific topology data

Storage devices are identified by having `capacity` in specs.

## JSON Output

```bash
sp analyze --format=json
```

Returns:
```json
{
  "config_id": "39c74b62-...",
  "config_name": "SATA Option",
  "checks": [
    {
      "name": "cost",
      "status": "pass",
      "details": {
        "total": 653.0,
        "items_with_price": 2,
        "items_without_price": 0,
        "message": "Total: $653.00"
      }
    },
    ...
  ],
  "summary": {
    "total_items": 2,
    "total_cost": 653.0,
    "total_capacity": "8.0TB",
    "passes": 3,
    "warnings": 1,
    "failures": 0
  }
}
```

## Storage Domain Analysis

The storage domain module (`src/domains/storage/analysis.rs`) provides additional analysis:

### Redundancy Report

Analyzes topology for single points of failure and unprotected datasets.

### Capacity Report

Projects capacity utilization and estimates time until full based on growth rates.

### RPO/RTO Report

Checks sync configurations against dataset requirements.

These are not yet exposed via CLI but are available in the Rust library.

## Adding Custom Checks

See [extending.md](extending.md) for adding new analysis checks.
