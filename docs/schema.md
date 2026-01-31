# Data Schema Reference

The storage planner uses SQLite as the source of truth. All data is stored in `.sp/decisions.db`.

## Core Tables

### items

Catalog of purchasable items.

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PRIMARY KEY | Unique identifier (e.g., `samsung-870-evo-4tb`) |
| name | TEXT NOT NULL | Human-readable name |
| category | TEXT NOT NULL | Category (ssd, enclosure, software, etc.) |
| brand | TEXT | Brand name |
| specs | TEXT (JSON) | Specifications object |
| tags | TEXT (JSON array) | Tags for filtering |
| metadata | TEXT (JSON) | Additional metadata |
| archived | INTEGER | Soft delete flag (0 or 1) |
| created_at | TEXT | ISO 8601 timestamp |
| updated_at | TEXT | ISO 8601 timestamp |

**Specs JSON example:**
```json
{
  "capacity": "4TB",
  "read_speed": "560MB/s",
  "write_speed": "530MB/s",
  "interface": "SATA",
  "form_factor": "2.5inch",
  "noise_db": 0
}
```

### prices

Price observations (append-only).

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PRIMARY KEY | UUID |
| item_id | TEXT NOT NULL | References items.id |
| source | TEXT NOT NULL | Price source (manual, ebay, bestbuy, keepa, amazon) |
| price | REAL NOT NULL | Price in currency |
| currency | TEXT | Currency code (default: USD) |
| condition | TEXT NOT NULL | Item condition (new, used, refurbished, open_box) |
| url | TEXT | Source URL |
| observed_at | TEXT | When price was observed |
| metadata | TEXT (JSON) | Additional data |

Prices are **never updated or deleted** - new observations are always appended.

### configurations

Named compositions of items.

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PRIMARY KEY | UUID |
| name | TEXT NOT NULL | Configuration name |
| domain | TEXT | Domain (storage, computing, etc.) |
| items | TEXT (JSON array) | List of ConfigItem objects |
| domain_data | TEXT (JSON) | Domain-specific data |
| metadata | TEXT (JSON) | Additional metadata |
| is_current | INTEGER | Is this the current deployment? (0 or 1) |
| archived | INTEGER | Soft delete flag |
| created_at | TEXT | ISO 8601 timestamp |
| updated_at | TEXT | ISO 8601 timestamp |

**Items JSON example:**
```json
[
  {"item_id": "samsung-870-evo-4tb", "quantity": 2, "unit_price": 289.0, "notes": null},
  {"item_id": "owc-dual-mini", "quantity": 1, "unit_price": 75.0, "notes": "USB-C enclosure"}
]
```

### decisions

Decision sessions with options and outcomes.

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PRIMARY KEY | UUID |
| purpose | TEXT NOT NULL | Decision purpose/goal |
| status | TEXT | active, decided, or abandoned |
| options | TEXT (JSON object) | Map of option name to config ID |
| chosen_option | TEXT | Name of chosen option |
| chosen_config_id | TEXT | References configurations.id |
| rationale | TEXT | Why this option was chosen |
| decided_at | TEXT | When decision was made |
| decided_by | TEXT | Who made the decision |
| created_at | TEXT | Session creation time |
| metadata | TEXT (JSON) | Additional metadata |

**Options JSON example:**
```json
{
  "sata": "39c74b62-d293-4a9f-881f-45723dc4257c",
  "nvme": "71c0f495-8a1e-4b2c-9d3f-6e7a8b9c0d1e"
}
```

### events

Immutable audit log (append-only).

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PRIMARY KEY | UUID |
| event_type | TEXT NOT NULL | Event type (created, updated, archived, price_observed, decision_made, config_deployed) |
| entity_type | TEXT NOT NULL | Entity type (item, price, configuration, decision) |
| entity_id | TEXT NOT NULL | ID of affected entity |
| payload | TEXT (JSON) | Event details |
| timestamp | TEXT | When event occurred |
| actor | TEXT | Who performed the action |

Events are **never updated or deleted**.

## Full-Text Search

The `items_fts` virtual table provides full-text search over items:

```sql
SELECT * FROM items_fts WHERE items_fts MATCH 'samsung ssd';
```

Searchable fields: id, name, category, brand, tags

## Spec Units

The system understands these units in spec values:

**Capacity:** B, KB, MB, GB, TB, PB, KiB, MiB, GiB, TiB
**Speed:** Same as capacity, with optional `/s` suffix
**Noise:** dB, dBA

Examples:
- `"capacity": "4TB"` → 4,000,000,000,000 bytes
- `"read_speed": "560MB/s"` → 560,000,000 bytes/sec
- `"noise_db": 32` → 32 dB

## Exported YAML

When running `sp sync`, the database is exported to YAML:

```
export/
├── current.yaml          # Current configuration
├── catalog/
│   ├── ssd.yaml          # Items by category
│   ├── enclosure.yaml
│   └── _prices.yaml      # Latest prices
└── history/
    └── 2026-01-30-a03420cf.yaml  # Decision snapshots
```

These files are **read-only snapshots**. The database is the source of truth.
