"""Main CLI entry point for storage planner."""

from pathlib import Path
from typing import Optional

import typer

from storage_planner.cli import validate, analyze, cost, simulate, catalog, suggest

app = typer.Typer(
    name="storage-planner",
    help="Model and analyze storage/backup topologies.",
    no_args_is_help=True,
)

# Register subcommands
app.add_typer(analyze.app, name="analyze", help="Run analysis on topology")
app.add_typer(catalog.app, name="catalog", help="Browse hardware/software catalog")
app.add_typer(suggest.app, name="suggest", help="Get hardware/software suggestions")

# Register single commands
app.command("validate")(validate.validate_cmd)
app.command("cost")(cost.cost_cmd)
app.command("simulate")(simulate.simulate_cmd)


# Default config path resolution
def resolve_config_path(config: Optional[Path]) -> Path:
    """Resolve config path, defaulting to topology.yaml in current dir."""
    if config:
        return config
    default = Path("topology.yaml")
    if default.exists():
        return default
    raise typer.BadParameter("No config file specified and topology.yaml not found in current directory")


def resolve_catalog_path(catalog_dir: Optional[Path]) -> Path:
    """Resolve catalog directory path."""
    if catalog_dir:
        return catalog_dir
    # Check common locations
    candidates = [
        Path("catalog"),
        Path.home() / ".config" / "storage-planner" / "catalog",
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate
    # Return default even if doesn't exist
    return Path("catalog")


if __name__ == "__main__":
    app()
