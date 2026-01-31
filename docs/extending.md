# Extending Storage Planner

## Adding Hardware Products

Edit `catalog/hardware.yaml`:

```yaml
products:
  - id: new-product-id        # Lowercase, hyphenated
    name: "Product Name"
    brand: BrandName
    category: ssd             # ssd|hdd|enclosure|nas|cable
    specs:
      capacity: "4TB"
      interface: "SATA"
      # ... category-specific fields
    # NO prices - those go in session files
```

No code changes needed. See [schema.md](schema.md) for full spec options.

## Adding Software Definitions

Edit `catalog/software.yaml`:

```yaml
software:
  - id: new_software
    name: "New Software"
    type: sync                # sync|backup|replication
    strengths:
      - feature1
      - feature2
    weaknesses:
      - limitation1
    best_for:
      change_rate: [high]
      direction: bidirectional
    platforms: [macos, linux]
```

The `best_for` fields drive `sp suggest software` matching.

## Adding New Analysis

### 1. Create Analysis Module

`src/storage_planner/analysis/new_analysis.py`:

```python
from dataclasses import dataclass
from storage_planner.models import Topology

@dataclass
class NewResult:
    item_id: str
    finding: str
    severity: str

def analyze_new(topology: Topology) -> list[NewResult]:
    """Pure function: topology → results."""
    results = []
    for dataset in topology.datasets:
        # Your analysis logic
        results.append(NewResult(...))
    return results
```

### 2. Add CLI Command

Option A: Add to existing analyze group (`cli/analyze.py`):

```python
@app.command("new-analysis")
def analyze_new_cmd(config: Path = typer.Argument(...)):
    topology = load_topology(config)
    results = analyze_new(topology)
    # Format and print results
```

Option B: Create new command file (`cli/new_cmd.py`):

```python
def new_cmd(config: Path = typer.Argument(...)):
    ...
```

Register in `cli/main.py`:

```python
from storage_planner.cli import new_cmd
app.command("new")(new_cmd.new_cmd)
```

### 3. Update Exports

Add to `analysis/__init__.py`:

```python
from storage_planner.analysis import new_analysis
```

## Adding New Model Fields

### 1. Update Pydantic Model

`models/topology.py` or `models/catalog.py`:

```python
class Dataset(BaseModel):
    # Existing fields...
    new_field: Optional[str] = None  # Optional with default
```

### 2. Update Schema Docs

Add to `docs/schema.md`.

### 3. Use in Analysis

Access in analysis functions:

```python
if dataset.new_field:
    # Use it
```

## Adding New Enum Values

`models/enums.py`:

```python
class SyncMethod(str, Enum):
    # Existing...
    NEW_METHOD = "new_method"
```

YAML files can immediately use `method: new_method`.

## Adding CLI Subcommand Group

`cli/new_group.py`:

```python
import typer

app = typer.Typer(no_args_is_help=True)

@app.command("sub1")
def sub1_cmd():
    ...

@app.command("sub2")
def sub2_cmd():
    ...
```

Register in `cli/main.py`:

```python
from storage_planner.cli import new_group
app.add_typer(new_group.app, name="new", help="New feature group")
```

## Testing Changes

```bash
source .venv/bin/activate
pytest                              # Run all tests
pytest tests/test_new.py -v         # Run specific test
sp validate current.yaml            # Quick validation
```

## Key Principles

1. **Pure functions**: Analysis takes data, returns results. No side effects.
2. **YAML is the interface**: Users edit YAML, not code.
3. **No internal state**: Every run reads fresh from files.
4. **Fail fast**: Validate early, provide clear errors.
5. **Prices in sessions**: Catalog has specs; session files capture point-in-time prices.
