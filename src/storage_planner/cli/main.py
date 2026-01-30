"""Main CLI entry point for storage planner."""

import typer

from storage_planner.cli import validate, analyze, cost, simulate, catalog, suggest
from storage_planner.cli.paths import resolve_config_path, resolve_catalog_path

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


if __name__ == "__main__":
    app()
