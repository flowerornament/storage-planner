"""Cost command for storage planner."""

from pathlib import Path
from typing import Optional

import typer
from rich.table import Table

from storage_planner.loaders import load_topology, load_all_catalogs, ValidationError
from storage_planner.output import console, print_error, print_json
from storage_planner.analysis.cost import analyze_cost
from storage_planner.cli.paths import resolve_config_path, resolve_catalog_path


def cost_cmd(
    config: Optional[Path] = typer.Argument(
        None,
        help="Path to topology YAML file (defaults to ./topology.yaml)",
    ),
    catalog_dir: Optional[Path] = typer.Option(
        None, "--catalog", "-c", help="Path to catalog directory"
    ),
    power_cost: float = typer.Option(
        0.12, "--power-cost", help="Cost per kWh for power calculations"
    ),
    json_output: bool = typer.Option(False, "--json", help="Output JSON"),
) -> None:
    """Calculate cost breakdown and projections.

    Shows monthly operational costs, hardware costs, and 5-year projection.
    """
    try:
        config_path = resolve_config_path(config)
        topology = load_topology(config_path)

        # Load catalog if available
        hardware = None
        prices = None
        if catalog_dir:
            hardware, _, prices = load_all_catalogs(resolve_catalog_path(catalog_dir))

        summary = analyze_cost(topology, hardware, prices, power_cost)

        if json_output:
            print_json(
                {
                    "topology": {"name": topology.name, "path": config_path},
                    "summary": summary,
                }
            )
            return

        console.print(f"[bold]Cost Analysis: {topology.name}[/bold]\n")

        # Node breakdown table
        table = Table(title="Cost by Node")
        table.add_column("Node")
        table.add_column("Monthly Hosting", justify="right")
        table.add_column("Monthly Power", justify="right")
        table.add_column("Hardware", justify="right")
        table.add_column("Notes")

        for nc in summary.nodes:
            notes = "; ".join(nc.notes[:2])  # Show first 2 notes
            if len(nc.notes) > 2:
                notes += f" (+{len(nc.notes) - 2} more)"

            table.add_row(
                nc.node_name,
                f"${nc.monthly_hosting:.2f}" if nc.monthly_hosting else "-",
                f"${nc.monthly_power:.2f}" if nc.monthly_power else "-",
                f"${nc.hardware_cost:.2f}" if nc.hardware_cost else "-",
                notes or "-",
            )

        console.print(table)
        console.print()

        # Summary
        console.print("[bold]Summary[/bold]")
        console.print(f"  Monthly operational: [cyan]${summary.total_monthly:.2f}[/cyan]")
        console.print(f"  Hardware (one-time): [cyan]${summary.total_hardware:.2f}[/cyan]")
        console.print(f"  5-year projection:   [cyan]${summary.five_year_projection:.2f}[/cyan]")

        # Notes/warnings
        for note in summary.breakdown_notes:
            console.print(f"\n[yellow]{note}[/yellow]")

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)
