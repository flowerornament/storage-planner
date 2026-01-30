"""Simulate command for storage planner."""

from pathlib import Path

import typer
from rich.table import Table
from rich.panel import Panel

from storage_planner.loaders import load_topology, ValidationError
from storage_planner.output import console, print_error, print_warning
from storage_planner.analysis.failure_sim import simulate_node_failure, simulate_volume_failure
from storage_planner.models import Criticality


def simulate_cmd(
    entity: str = typer.Argument(..., help="Node or volume ID to simulate failure of"),
    config: Path = typer.Argument(
        ...,
        help="Path to topology YAML file",
        exists=True,
        readable=True,
    ),
    entity_type: str = typer.Option(
        "auto",
        "--type",
        "-t",
        help="Entity type: 'node', 'volume', or 'auto' (detect automatically)",
    ),
) -> None:
    """Simulate failure of a node or volume.

    Shows which datasets would be affected and recovery paths.
    """
    try:
        topology = load_topology(config)

        # Auto-detect entity type
        if entity_type == "auto":
            if topology.get_node(entity):
                entity_type = "node"
            elif topology.get_volume(entity):
                entity_type = "volume"
            else:
                print_error(f"Entity '{entity}' not found as node or volume")
                raise typer.Exit(1)

        # Run simulation
        if entity_type == "node":
            result = simulate_node_failure(topology, entity)
        else:
            result = simulate_volume_failure(topology, entity)

        # Display results
        style = "red" if result.data_loss_risk else "yellow" if result.affected_datasets else "green"
        console.print(
            Panel(
                f"[bold]{result.summary}[/bold]",
                title=f"Failure Simulation: {result.failed_entity} ({result.failed_type})",
                border_style=style,
            )
        )

        if result.affected_volumes:
            console.print(f"\n[bold]Affected Volumes:[/bold] {', '.join(result.affected_volumes)}")

        if result.affected_datasets:
            console.print()
            table = Table(title="Dataset Impact")
            table.add_column("Dataset")
            table.add_column("Criticality")
            table.add_column("Lost")
            table.add_column("Remaining")
            table.add_column("Recovery Sources")
            table.add_column("Status")

            for impact in result.affected_datasets:
                crit_style = {
                    Criticality.CRITICAL: "red",
                    Criticality.IMPORTANT: "yellow",
                    Criticality.REPLACEABLE: "dim",
                }.get(impact.criticality, "")

                if not impact.is_recoverable:
                    status = "[red]UNRECOVERABLE[/red]"
                elif impact.remaining_copies < 2:
                    status = "[yellow]AT RISK[/yellow]"
                else:
                    status = "[green]OK[/green]"

                recovery = ", ".join(impact.recovery_sources[:3])
                if len(impact.recovery_sources) > 3:
                    recovery += f" (+{len(impact.recovery_sources) - 3})"

                table.add_row(
                    impact.dataset_name,
                    f"[{crit_style}]{impact.criticality.value}[/{crit_style}]",
                    str(impact.lost_copies),
                    str(impact.remaining_copies),
                    recovery or "[red]none[/red]",
                    status,
                )

            console.print(table)

            # Show notes for critical issues
            for impact in result.affected_datasets:
                if impact.notes and impact.criticality == Criticality.CRITICAL:
                    for note in impact.notes:
                        print_warning(f"{impact.dataset_name}: {note}")

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)
