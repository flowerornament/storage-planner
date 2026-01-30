"""Analyze commands for storage planner."""

from pathlib import Path

import typer
from rich.table import Table

from storage_planner.analysis import bandwidth, capacity, redundancy, rpo_rto
from storage_planner.analysis.completeness import IssueSeverity, validate_completeness
from storage_planner.cli.paths import resolve_config_path
from storage_planner.loaders import ValidationError, load_topology
from storage_planner.output import (
    console,
    print_error,
    print_json,
    print_success,
    print_warning,
)

app = typer.Typer(no_args_is_help=True)


@app.command("all")
def analyze_all(
    config: Path | None = typer.Argument(
        None,
        help="Path to topology YAML file (defaults to ./topology.yaml)",
    ),
    catalog_dir: Path | None = typer.Option(
        None, "--catalog", "-c", help="Path to catalog directory"
    ),
    json_output: bool = typer.Option(False, "--json", help="Output JSON"),
) -> None:
    """Run full analysis on a topology (redundancy, RPO/RTO, bandwidth, capacity)."""
    try:
        config_path = resolve_config_path(config)
        topology = load_topology(config_path)
        completeness_report = validate_completeness(topology)

        if json_output:
            payload = {
                "topology": {"name": topology.name, "path": config_path},
                "completeness": _completeness_payload(completeness_report),
                "redundancy": _redundancy_payload(topology),
                "rpo_rto": _rpo_payload(topology),
                "bandwidth": _bandwidth_payload(topology),
                "capacity": _capacity_payload(topology, 12),
            }
            print_json(payload)
            return

        console.print(f"[bold]Analysis: {topology.name}[/bold]\n")
        _print_completeness_summary(completeness_report)

        # Run all analyses
        _run_redundancy_analysis(topology)
        console.print()
        _run_rpo_rto_analysis(topology)
        console.print()
        _run_bandwidth_analysis(topology)
        console.print()
        _run_capacity_analysis(topology)

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


@app.command("diff")
def analyze_diff(
    config_a: Path = typer.Argument(..., help="Path to baseline topology YAML file"),
    config_b: Path = typer.Argument(..., help="Path to comparison topology YAML file"),
    json_output: bool = typer.Option(False, "--json", help="Output JSON"),
) -> None:
    """Compare analysis results between two topologies."""
    try:
        topo_a = load_topology(config_a)
        topo_b = load_topology(config_b)

        payload_a = {
            "redundancy": _redundancy_payload(topo_a),
            "rpo_rto": _rpo_payload(topo_a),
            "bandwidth": _bandwidth_payload(topo_a),
            "capacity": _capacity_payload(topo_a, 12),
        }
        payload_b = {
            "redundancy": _redundancy_payload(topo_b),
            "rpo_rto": _rpo_payload(topo_b),
            "bandwidth": _bandwidth_payload(topo_b),
            "capacity": _capacity_payload(topo_b, 12),
        }

        diff = _diff_analysis(payload_a, payload_b)

        if json_output:
            print_json(
                {
                    "topology_a": {"name": topo_a.name, "path": config_a},
                    "topology_b": {"name": topo_b.name, "path": config_b},
                    "summaries": {
                        "a": {
                            k: v["summary"] for k, v in payload_a.items()
                        },
                        "b": {
                            k: v["summary"] for k, v in payload_b.items()
                        },
                    },
                    "diff": diff,
                }
            )
            return

        console.print("[bold]Analysis Diff[/bold]\n")
        console.print(f"A: {topo_a.name}")
        console.print(f"B: {topo_b.name}\n")

        _print_diff_section("Redundancy", diff["redundancy"])
        _print_diff_section("RPO/RTO", diff["rpo_rto"])
        _print_diff_section("Bandwidth", diff["bandwidth"])
        _print_diff_section("Capacity", diff["capacity"])

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


@app.command("redundancy")
def analyze_redundancy(
    config: Path | None = typer.Argument(
        None,
        help="Path to topology YAML file (defaults to ./topology.yaml)",
    ),
    json_output: bool = typer.Option(False, "--json", help="Output JSON"),
) -> None:
    """Check data redundancy against requirements."""
    try:
        config_path = resolve_config_path(config)
        topology = load_topology(config_path)
        completeness_report = validate_completeness(topology)

        payload = _redundancy_payload(topology)
        if json_output:
            payload["topology"] = {"name": topology.name, "path": config_path}
            payload["completeness"] = _completeness_payload(completeness_report)
            print_json(payload)
            return

        _print_completeness_summary(completeness_report)
        _run_redundancy_analysis(topology, payload)
    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


@app.command("bandwidth")
def analyze_bandwidth_cmd(
    config: Path | None = typer.Argument(
        None,
        help="Path to topology YAML file (defaults to ./topology.yaml)",
    ),
    json_output: bool = typer.Option(False, "--json", help="Output JSON"),
) -> None:
    """Analyze bandwidth and identify bottlenecks."""
    try:
        config_path = resolve_config_path(config)
        topology = load_topology(config_path)
        completeness_report = validate_completeness(topology)

        payload = _bandwidth_payload(topology)
        if json_output:
            payload["topology"] = {"name": topology.name, "path": config_path}
            payload["completeness"] = _completeness_payload(completeness_report)
            print_json(payload)
            return

        _print_completeness_summary(completeness_report)
        _run_bandwidth_analysis(topology, payload)
    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


@app.command("rpo-rto")
def analyze_rpo_rto_cmd(
    config: Path | None = typer.Argument(
        None,
        help="Path to topology YAML file (defaults to ./topology.yaml)",
    ),
    json_output: bool = typer.Option(False, "--json", help="Output JSON"),
) -> None:
    """Check RPO/RTO compliance for datasets."""
    try:
        config_path = resolve_config_path(config)
        topology = load_topology(config_path)
        completeness_report = validate_completeness(topology)

        payload = _rpo_payload(topology)
        if json_output:
            payload["topology"] = {"name": topology.name, "path": config_path}
            payload["completeness"] = _completeness_payload(completeness_report)
            print_json(payload)
            return

        _print_completeness_summary(completeness_report)
        _run_rpo_rto_analysis(topology, payload)
    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


@app.command("capacity")
def analyze_capacity_cmd(
    config: Path | None = typer.Argument(
        None,
        help="Path to topology YAML file (defaults to ./topology.yaml)",
    ),
    months: int = typer.Option(12, "--months", "-m", help="Projection period in months"),
    json_output: bool = typer.Option(False, "--json", help="Output JSON"),
) -> None:
    """Project capacity needs over time."""
    try:
        config_path = resolve_config_path(config)
        topology = load_topology(config_path)
        completeness_report = validate_completeness(topology)

        payload = _capacity_payload(topology, months)
        if json_output:
            payload["topology"] = {"name": topology.name, "path": config_path}
            payload["completeness"] = _completeness_payload(completeness_report)
            print_json(payload)
            return

        _print_completeness_summary(completeness_report)
        _run_capacity_analysis(topology, months, payload)
    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


def _run_redundancy_analysis(topology, payload: dict | None = None) -> None:
    """Run redundancy analysis and print results."""
    payload = payload or _redundancy_payload(topology)
    results = payload["results"]
    _print_insight_summary("Redundancy", payload["summary"])

    table = Table(title="Redundancy Analysis")
    table.add_column("Dataset")
    table.add_column("Criticality")
    table.add_column("Copies")
    table.add_column("Required")
    table.add_column("Locations")
    table.add_column("Req Loc")
    table.add_column("Status")

    for r in results:
        copies_str = str(r.actual_copies)
        if r.actual_copies < r.required_copies:
            copies_str = f"[red]{r.actual_copies}[/red]"

        locs_str = str(r.actual_locations)
        if r.actual_locations < r.required_locations:
            locs_str = f"[red]{r.actual_locations}[/red]"

        status = "[green]OK[/green]" if r.meets_requirements else "[red]FAIL[/red]"

        table.add_row(
            r.dataset_id,
            r.criticality.value,
            copies_str,
            str(r.required_copies),
            locs_str,
            str(r.required_locations),
            status,
        )

    console.print(table)

    # Summary
    if payload["summary"]["failing"] > 0:
        print_warning(
            f"{payload['summary']['failing']} dataset(s) do not meet redundancy requirements"
        )
    else:
        print_success("All datasets meet redundancy requirements")


def _run_rpo_rto_analysis(topology, payload: dict | None = None) -> None:
    """Run RPO/RTO analysis and print results."""
    payload = payload or _rpo_payload(topology)
    results = payload["results"]
    _print_insight_summary("RPO/RTO", payload["summary"])

    table = Table(title="RPO/RTO Analysis")
    table.add_column("Dataset")
    table.add_column("Max RPO")
    table.add_column("Achieved RPO")
    table.add_column("Max RTO")
    table.add_column("Status")

    for r in results:
        achieved = r.achieved_rpo or "[dim]unknown[/dim]"
        if r.achieved_rpo and r.achieved_rpo_source == "estimated":
            achieved = f"~{r.achieved_rpo}"
        if r.rpo_met is False:
            achieved = f"[red]{achieved}[/red]"
        elif r.rpo_met is True:
            achieved = f"[green]{achieved}[/green]"

        max_rpo = r.max_rpo or "[dim]none[/dim]"
        max_rto = r.max_rto or "[dim]none[/dim]"

        if r.rpo_met is None:
            status = "[yellow]?[/yellow]"
        elif r.rpo_met:
            status = "[green]OK[/green]"
        else:
            status = "[red]FAIL[/red]"

        table.add_row(r.dataset_id, max_rpo, achieved, max_rto, status)

    console.print(table)


def _run_bandwidth_analysis(topology, payload: dict | None = None) -> None:
    """Run bandwidth analysis and print results."""
    payload = payload or _bandwidth_payload(topology)
    results = payload["results"]
    _print_insight_summary("Bandwidth", payload["summary"])

    if not results:
        console.print("[dim]No sync regimes to analyze[/dim]")
        return

    table = Table(title="Bandwidth Analysis")
    table.add_column("Sync Regime")
    table.add_column("Target Volume")
    table.add_column("Dataset Size")
    table.add_column("Link")
    table.add_column("Bandwidth")
    table.add_column("Est. Full Sync")
    table.add_column("Notes")

    for r in results:
        notes = ""
        if r.is_bottleneck:
            notes = "[yellow]Potential bottleneck[/yellow]"

        table.add_row(
            r.sync_regime_id,
            r.target_volume,
            r.dataset_size,
            r.link_id or "[dim]unknown[/dim]",
            r.effective_bandwidth or "[dim]unknown[/dim]",
            r.estimated_sync_time or "[dim]unknown[/dim]",
            notes,
        )

    console.print(table)


def _run_capacity_analysis(topology, months: int = 12, payload: dict | None = None) -> None:
    """Run capacity analysis and print results."""
    payload = payload or _capacity_payload(topology, months)
    results = payload["results"]
    _print_insight_summary("Capacity", payload["summary"])

    table = Table(title=f"Capacity Projection ({months} months)")
    table.add_column("Volume")
    table.add_column("Current Used")
    table.add_column("Capacity")
    table.add_column("Projected Used")
    table.add_column("Utilization")
    table.add_column("Status")

    for r in results:
        util_str = f"{r.projected_utilization_pct:.0f}%"
        if r.projected_utilization_pct > 90:
            util_str = f"[red]{util_str}[/red]"
        elif r.projected_utilization_pct > 75:
            util_str = f"[yellow]{util_str}[/yellow]"

        status = "[green]OK[/green]"
        if r.will_exceed_capacity:
            status = "[red]OVER[/red]"
        elif r.projected_utilization_pct > 90:
            status = "[yellow]WARN[/yellow]"

        table.add_row(
            r.volume_id,
            r.current_used,
            r.capacity,
            r.projected_used,
            util_str,
            status,
        )

    console.print(table)


@app.command("quick")
def analyze_quick(
    config: Path | None = typer.Argument(
        None,
        help="Path to topology YAML file (defaults to ./topology.yaml)",
    ),
    json_output: bool = typer.Option(False, "--json", help="Output JSON"),
) -> None:
    """Run quick analysis (redundancy, RPO/RTO, capacity summaries only)."""
    try:
        config_path = resolve_config_path(config)
        topology = load_topology(config_path)
        completeness_report = validate_completeness(topology)

        payload = {
            "topology": {"name": topology.name, "path": config_path},
            "completeness": _completeness_payload(completeness_report),
            "redundancy": _redundancy_payload(topology),
            "rpo_rto": _rpo_payload(topology),
            "capacity": _capacity_payload(topology, 12),
        }

        if json_output:
            print_json(payload)
            return

        console.print(f"[bold]Quick Analysis: {topology.name}[/bold]\n")
        _print_completeness_summary(completeness_report)
        _print_insight_summary("Redundancy", payload["redundancy"]["summary"])
        _print_insight_summary("RPO/RTO", payload["rpo_rto"]["summary"])
        _print_insight_summary("Capacity", payload["capacity"]["summary"])

    except ValidationError as e:
        print_error(e.message)
        raise typer.Exit(1)


def _completeness_payload(report) -> dict:
    issues = [
        {
            "severity": issue.severity.value,
            "location": issue.location,
            "field": issue.field,
            "message": issue.message,
            "suggestion": issue.suggestion,
        }
        for issue in report.issues
    ]
    errors = [i for i in issues if i["severity"] == IssueSeverity.ERROR.value]
    warnings = [i for i in issues if i["severity"] == IssueSeverity.WARNING.value]
    return {
        "is_complete": report.is_complete,
        "errors": errors,
        "warnings": warnings,
        "missing_context": issues,
    }


def _print_completeness_summary(report) -> None:
    if report.is_complete:
        return
    errors = [i for i in report.issues if i.severity == IssueSeverity.ERROR]
    warnings = [i for i in report.issues if i.severity == IssueSeverity.WARNING]
    print_warning(
        f"Completeness: {len(errors)} error(s), {len(warnings)} warning(s). "
        "Results may be incomplete."
    )
    for issue in report.issues[:3]:
        print_warning(f"{issue.location}.{issue.field}: {issue.message}")


def _print_insight_summary(title: str, summary: dict) -> None:
    console.print(f"[bold]Insight Summary ({title})[/bold]")
    for line in summary.get("highlights", [])[:3]:
        console.print(f"  • {line}")
    console.print()


def _redundancy_payload(topology) -> dict:
    results = redundancy.analyze_redundancy(topology)
    summary = _summarize_redundancy(results)
    return {"summary": summary, "results": results}


def _rpo_payload(topology) -> dict:
    results = rpo_rto.analyze_rpo_rto(topology)
    summary = _summarize_rpo(results)
    return {"summary": summary, "results": results}


def _bandwidth_payload(topology) -> dict:
    results = bandwidth.analyze_bandwidth(topology)
    summary = _summarize_bandwidth(results)
    return {"summary": summary, "results": results}


def _capacity_payload(topology, months: int) -> dict:
    results = capacity.analyze_capacity(topology, months)
    summary = _summarize_capacity(results, months)
    return {"summary": summary, "results": results, "months": months}


def _summarize_redundancy(results) -> dict:
    failing = [r for r in results if not r.meets_requirements]
    missing = sorted(
        failing,
        key=lambda r: (
            (r.required_copies - r.actual_copies),
            (r.required_locations - r.actual_locations),
        ),
        reverse=True,
    )
    top = [
        {
            "dataset_id": r.dataset_id,
            "missing_copies": max(r.required_copies - r.actual_copies, 0),
            "missing_locations": max(r.required_locations - r.actual_locations, 0),
        }
        for r in missing[:3]
    ]
    highlights = [f"{len(results)} datasets, {len(failing)} failing"]
    for item in top:
        highlights.append(
            f"{item['dataset_id']}: -{item['missing_copies']} copies, -{item['missing_locations']} locations"
        )
    return {
        "total": len(results),
        "failing": len(failing),
        "passing": len(results) - len(failing),
        "top_issues": top,
        "highlights": highlights,
        "status": "fail" if failing else "ok",
    }


def _summarize_rpo(results) -> dict:
    failing = [r for r in results if r.rpo_met is False]
    unknown = [r for r in results if r.rpo_met is None]
    highlights = [
        f"{len(results)} datasets, {len(failing)} failing, {len(unknown)} unknown"
    ]
    top_items = []
    for r in failing[:2]:
        top_items.append(
            {
                "dataset_id": r.dataset_id,
                "max_rpo": r.max_rpo,
                "achieved_rpo": r.achieved_rpo,
                "status": "fail",
            }
        )
        highlights.append(f"{r.dataset_id}: {r.achieved_rpo} > {r.max_rpo}")
    for r in unknown[:1]:
        top_items.append(
            {
                "dataset_id": r.dataset_id,
                "max_rpo": r.max_rpo,
                "achieved_rpo": r.achieved_rpo,
                "status": "unknown",
            }
        )
        highlights.append(f"{r.dataset_id}: achieved RPO unknown")
    return {
        "total": len(results),
        "failing": len(failing),
        "unknown": len(unknown),
        "passing": len(results) - len(failing) - len(unknown),
        "top_issues": top_items,
        "highlights": highlights,
        "status": "fail" if failing else "warn" if unknown else "ok",
    }


def _summarize_bandwidth(results) -> dict:
    if not results:
        return {
            "total": 0,
            "bottlenecks": 0,
            "top_issues": [],
            "highlights": ["0 transfers"],
            "status": "ok",
        }
    bottlenecks = [r for r in results if r.is_bottleneck]
    sortable = [r for r in results if r.estimated_sync_time_seconds is not None]
    sortable.sort(key=lambda r: r.estimated_sync_time_seconds or 0, reverse=True)
    top = [
        {
            "sync_regime_id": r.sync_regime_id,
            "target_volume": r.target_volume,
            "estimated_sync_time": r.estimated_sync_time,
        }
        for r in sortable[:3]
    ]
    highlights = [f"{len(results)} transfers, {len(bottlenecks)} bottlenecks"]
    for item in top[:3]:
        highlights.append(
            f"{item['sync_regime_id']} -> {item['target_volume']}: {item['estimated_sync_time']}"
        )
    return {
        "total": len(results),
        "bottlenecks": len(bottlenecks),
        "top_issues": top,
        "highlights": highlights,
        "status": "warn" if bottlenecks else "ok",
    }


def _summarize_capacity(results, months: int) -> dict:
    if not results:
        return {
            "total": 0,
            "over": 0,
            "warnings": 0,
            "top_issues": [],
            "highlights": [f"0 volumes over {months} months"],
            "status": "ok",
        }
    over = [r for r in results if r.will_exceed_capacity]
    warn = [r for r in results if r.projected_utilization_pct > 90]
    sorted_by_util = sorted(results, key=lambda r: r.projected_utilization_pct, reverse=True)
    top = [
        {
            "volume_id": r.volume_id,
            "projected_utilization_pct": r.projected_utilization_pct,
            "months_until_full": r.months_until_full,
        }
        for r in sorted_by_util[:3]
    ]
    highlights = [
        f"{len(results)} volumes, {len(over)} over capacity, {len(warn)} >90% in {months} months"
    ]
    for item in top:
        highlights.append(
            f"{item['volume_id']}: {item['projected_utilization_pct']:.0f}% projected"
        )
    return {
        "total": len(results),
        "over": len(over),
        "warnings": len(warn),
        "top_issues": top,
        "highlights": highlights,
        "status": "fail" if over else "warn" if warn else "ok",
        "months": months,
    }


def _diff_analysis(payload_a: dict, payload_b: dict) -> dict:
    return {
        "redundancy": _diff_summary(
            payload_a["redundancy"]["summary"],
            payload_b["redundancy"]["summary"],
            keys=["failing", "passing"],
        ),
        "rpo_rto": _diff_summary(
            payload_a["rpo_rto"]["summary"],
            payload_b["rpo_rto"]["summary"],
            keys=["failing", "unknown", "passing"],
        ),
        "bandwidth": _diff_summary(
            payload_a["bandwidth"]["summary"],
            payload_b["bandwidth"]["summary"],
            keys=["bottlenecks", "total"],
        ),
        "capacity": _diff_summary(
            payload_a["capacity"]["summary"],
            payload_b["capacity"]["summary"],
            keys=["over", "warnings", "total"],
        ),
    }


def _diff_summary(summary_a: dict, summary_b: dict, keys: list[str]) -> dict:
    diff = {}
    for key in keys:
        a_val = summary_a.get(key, 0)
        b_val = summary_b.get(key, 0)
        diff[key] = {"a": a_val, "b": b_val, "delta": b_val - a_val}
    return diff


def _print_diff_section(title: str, diff: dict) -> None:
    console.print(f"[bold]{title}[/bold]")
    for key, values in diff.items():
        delta = values["delta"]
        sign = "+" if delta > 0 else ""
        console.print(f"  {key}: {values['a']} -> {values['b']} ({sign}{delta})")
    console.print()
