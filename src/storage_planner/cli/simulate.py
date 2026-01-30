"""Simulate command for storage planner."""

from pathlib import Path
from typing import Optional

import typer
from rich.table import Table
from rich.panel import Panel

from storage_planner.loaders import load_topology, ValidationError
from storage_planner.output import console, print_error, print_warning, print_json
from storage_planner.analysis.failure_sim import simulate_node_failure, simulate_volume_failure
from storage_planner.models import Criticality
from storage_planner.cli.paths import resolve_config_path


def simulate_cmd(
    mode: str = typer.Argument(..., help="Node/volume ID, or 'diff' for comparison"),
    arg1: Optional[str] = typer.Argument(
        None, help="Topology path, or entity ID when using 'diff'"
    ),
    arg2: Optional[str] = typer.Argument(
        None, help="Baseline topology path when using 'diff'"
    ),
    arg3: Optional[str] = typer.Argument(
        None, help="Comparison topology path when using 'diff'"
    ),
    entity_type: str = typer.Option(
        "auto",
        "--type",
        "-t",
        help="Entity type: 'node', 'volume', or 'auto' (detect automatically)",
    ),
    json_output: bool = typer.Option(False, "--json", help="Output JSON"),
) -> None:
    """Simulate failure of a node or volume, or compare between two topologies."""
    if mode == "diff":
        if not arg1 or not arg2 or not arg3:
            raise typer.BadParameter(
                "simulate diff requires: diff <entity> <topology_a> <topology_b>"
            )
        _run_simulation_diff(arg1, Path(arg2), Path(arg3), entity_type, json_output)
        return

    entity = mode
    config = Path(arg1) if arg1 else None
    _run_simulation(entity, config, entity_type, json_output)


def _run_simulation(
    entity: str,
    config: Optional[Path],
    entity_type: str,
    json_output: bool,
) -> None:
    try:
        config_path = resolve_config_path(config)
        topology = load_topology(config_path)

        resolved_type = _resolve_entity_type(topology, entity, entity_type)
        result = _simulate_entity(topology, entity, resolved_type)

        if json_output:
            print_json(
                {
                    "topology": {"name": topology.name, "path": config_path},
                    "result": result,
                }
            )
            return

        _print_simulation(result)

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)
    except ValueError as e:
        print_error(str(e))
        raise typer.Exit(1)


def _run_simulation_diff(
    entity: str,
    config_a: Path,
    config_b: Path,
    entity_type: str,
    json_output: bool,
) -> None:
    try:
        topo_a = load_topology(config_a)
        topo_b = load_topology(config_b)

        resolved_type_a = _resolve_entity_type(topo_a, entity, entity_type)
        resolved_type_b = _resolve_entity_type(topo_b, entity, entity_type)
        if resolved_type_a != resolved_type_b:
            raise typer.BadParameter(
                f"Entity '{entity}' resolves to '{resolved_type_a}' in A and '{resolved_type_b}' in B"
            )

        result_a = _simulate_entity(topo_a, entity, resolved_type_a)
        result_b = _simulate_entity(topo_b, entity, resolved_type_b)

        summary_a = _summarize_failure(result_a)
        summary_b = _summarize_failure(result_b)
        diff = _diff_failure(result_a, result_b)

        if json_output:
            print_json(
                {
                    "entity": entity,
                    "entity_type": resolved_type_a,
                    "topology_a": {"name": topo_a.name, "path": config_a, "summary": summary_a, "result": result_a},
                    "topology_b": {"name": topo_b.name, "path": config_b, "summary": summary_b, "result": result_b},
                    "diff": diff,
                }
            )
            return

        console.print(f"[bold]Simulation Diff: {entity} ({resolved_type_a})[/bold]\n")
        console.print(f"A: {topo_a.name}")
        console.print(f"  Affected: {summary_a['affected_datasets']}")
        console.print(f"  Unrecoverable: {summary_a['unrecoverable']}")
        console.print(f"  Data loss risk: {summary_a['data_loss_risk']}")
        console.print(f"B: {topo_b.name}")
        console.print(f"  Affected: {summary_b['affected_datasets']}")
        console.print(f"  Unrecoverable: {summary_b['unrecoverable']}")
        console.print(f"  Data loss risk: {summary_b['data_loss_risk']}")

        if diff["added"] or diff["removed"] or diff["worsened"] or diff["improved"]:
            console.print("\n[bold]Dataset Changes[/bold]")
            if diff["added"]:
                console.print(f"  Added: {', '.join(diff['added'])}")
            if diff["removed"]:
                console.print(f"  Removed: {', '.join(diff['removed'])}")
            if diff["worsened"]:
                console.print(f"  Worsened: {', '.join(diff['worsened'])}")
            if diff["improved"]:
                console.print(f"  Improved: {', '.join(diff['improved'])}")

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)
    except ValueError as e:
        print_error(str(e))
        raise typer.Exit(1)


def _resolve_entity_type(topology, entity: str, entity_type: str) -> str:
    if entity_type == "auto":
        if topology.get_node(entity):
            return "node"
        if topology.get_volume(entity):
            return "volume"
        raise ValueError(f"Entity '{entity}' not found as node or volume")
    if entity_type not in ("node", "volume"):
        raise typer.BadParameter("Entity type must be 'node', 'volume', or 'auto'")
    return entity_type


def _simulate_entity(topology, entity: str, entity_type: str):
    if entity_type == "node":
        return simulate_node_failure(topology, entity)
    return simulate_volume_failure(topology, entity)


def _print_simulation(result) -> None:
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


def _summarize_failure(result) -> dict:
    unrecoverable = [d for d in result.affected_datasets if not d.is_recoverable]
    critical_unrecoverable = [
        d for d in unrecoverable if d.criticality == Criticality.CRITICAL
    ]
    return {
        "affected_datasets": len(result.affected_datasets),
        "unrecoverable": len(unrecoverable),
        "critical_unrecoverable": len(critical_unrecoverable),
        "data_loss_risk": result.data_loss_risk,
    }


def _diff_failure(result_a, result_b) -> dict:
    map_a = {d.dataset_id: d for d in result_a.affected_datasets}
    map_b = {d.dataset_id: d for d in result_b.affected_datasets}
    ids_a = set(map_a)
    ids_b = set(map_b)

    added = sorted(ids_b - ids_a)
    removed = sorted(ids_a - ids_b)
    worsened: list[str] = []
    improved: list[str] = []

    for dataset_id in sorted(ids_a & ids_b):
        a = map_a[dataset_id]
        b = map_b[dataset_id]
        if (a.is_recoverable and not b.is_recoverable) or (
            b.remaining_copies < a.remaining_copies
        ):
            worsened.append(dataset_id)
        elif (not a.is_recoverable and b.is_recoverable) or (
            b.remaining_copies > a.remaining_copies
        ):
            improved.append(dataset_id)

    return {
        "added": added,
        "removed": removed,
        "worsened": worsened,
        "improved": improved,
    }
