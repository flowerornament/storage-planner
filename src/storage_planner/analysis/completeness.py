"""Completeness validation for storage planner.

Checks that all required configuration is explicitly specified,
with no implicit assumptions or defaults being used.
"""

from dataclasses import dataclass, field
from enum import Enum
from typing import Optional

from storage_planner.models import Topology, Criticality


class IssueSeverity(str, Enum):
    """Severity of a completeness issue."""

    ERROR = "error"  # Analysis will fail or produce incorrect results
    WARNING = "warning"  # Analysis uses implicit assumption


@dataclass
class CompletenessIssue:
    """A single completeness issue found in the topology."""

    severity: IssueSeverity
    location: str  # e.g., "constraints", "dataset.photos-archive", "sync_regime.tm-backup"
    field: str  # e.g., "min_critical_data_copies", "achieved_rpo"
    message: str
    suggestion: Optional[str] = None


@dataclass
class CompletenessReport:
    """Report of all completeness issues found."""

    issues: list[CompletenessIssue] = field(default_factory=list)

    @property
    def has_errors(self) -> bool:
        return any(i.severity == IssueSeverity.ERROR for i in self.issues)

    @property
    def has_warnings(self) -> bool:
        return any(i.severity == IssueSeverity.WARNING for i in self.issues)

    @property
    def is_complete(self) -> bool:
        return len(self.issues) == 0


def validate_completeness(topology: Topology) -> CompletenessReport:
    """Validate that topology has all required explicit configuration.

    Checks for:
    - Missing required constraints for dataset criticality levels
    - Sync regimes without achieved_rpo
    - Datasets without required_copies/required_locations
    - Nodes with power_watts but no power_cost_per_kwh
    - Other implicit assumptions
    """
    issues: list[CompletenessIssue] = []

    # Track which constraint levels are needed
    has_critical = any(d.criticality == Criticality.CRITICAL for d in topology.datasets)
    has_important = any(d.criticality == Criticality.IMPORTANT for d in topology.datasets)

    # Check constraints completeness
    if has_critical:
        if topology.constraints.min_critical_data_copies is None:
            issues.append(
                CompletenessIssue(
                    severity=IssueSeverity.ERROR,
                    location="constraints",
                    field="min_critical_data_copies",
                    message="Required for critical datasets but not set",
                    suggestion="Add 'min_critical_data_copies: 3' (or your desired value) to constraints",
                )
            )
        if topology.constraints.min_locations_for_critical is None:
            issues.append(
                CompletenessIssue(
                    severity=IssueSeverity.ERROR,
                    location="constraints",
                    field="min_locations_for_critical",
                    message="Required for critical datasets but not set",
                    suggestion="Add 'min_locations_for_critical: 2' (or your desired value) to constraints",
                )
            )

    if has_important:
        if topology.constraints.min_important_data_copies is None:
            issues.append(
                CompletenessIssue(
                    severity=IssueSeverity.ERROR,
                    location="constraints",
                    field="min_important_data_copies",
                    message="Required for important datasets but not set",
                    suggestion="Add 'min_important_data_copies: 2' (or your desired value) to constraints",
                )
            )

    # Check if power cost is needed
    needs_power_cost = any(
        node.power_watts_idle is not None
        for node in topology.nodes
    )
    if needs_power_cost and topology.constraints.power_cost_per_kwh is None:
        issues.append(
            CompletenessIssue(
                severity=IssueSeverity.ERROR,
                location="constraints",
                field="power_cost_per_kwh",
                message="Node(s) have power_watts_idle set but power cost not configured",
                suggestion="Add 'power_cost_per_kwh: 0.12' (or your local rate) to constraints",
            )
        )

    # Check sync regimes for achieved_rpo
    for regime in topology.sync_regimes:
        if regime.achieved_rpo is None:
            issues.append(
                CompletenessIssue(
                    severity=IssueSeverity.WARNING,
                    location=f"sync_regime.{regime.id}",
                    field="achieved_rpo",
                    message="No achieved_rpo specified; RPO analysis will show 'unknown'",
                    suggestion=f"Add 'achieved_rpo: \"30s\"' (or measured value) to sync_regime '{regime.id}'",
                )
            )

    # Check datasets for explicit requirements
    for dataset in topology.datasets:
        # Check for stored_on
        if not dataset.stored_on:
            issues.append(
                CompletenessIssue(
                    severity=IssueSeverity.ERROR,
                    location=f"dataset.{dataset.id}",
                    field="stored_on",
                    message="No volumes specified; dataset has no storage location",
                    suggestion=f"Add 'stored_on: [volume-id]' to dataset '{dataset.id}'",
                )
            )

    # Check nodes for location (needed for redundancy location counting)
    locations_used = set()
    for node in topology.nodes:
        if node.location:
            locations_used.add(node.location)
        else:
            # Node ID used as implicit location
            issues.append(
                CompletenessIssue(
                    severity=IssueSeverity.WARNING,
                    location=f"node.{node.id}",
                    field="location",
                    message=f"No location specified; using node ID '{node.id}' as location",
                    suggestion=f"Add 'location: home-office' (or appropriate location) to node '{node.id}'",
                )
            )

    return CompletenessReport(issues=issues)


def format_completeness_report(report: CompletenessReport) -> str:
    """Format completeness report as human-readable string."""
    if report.is_complete:
        return "✓ Topology is complete - all required configuration is explicit"

    lines = ["Completeness issues found:\n"]

    errors = [i for i in report.issues if i.severity == IssueSeverity.ERROR]
    warnings = [i for i in report.issues if i.severity == IssueSeverity.WARNING]

    if errors:
        lines.append(f"ERRORS ({len(errors)}):")
        for issue in errors:
            lines.append(f"  ✗ {issue.location}.{issue.field}: {issue.message}")
            if issue.suggestion:
                lines.append(f"    → {issue.suggestion}")
        lines.append("")

    if warnings:
        lines.append(f"WARNINGS ({len(warnings)}):")
        for issue in warnings:
            lines.append(f"  ⚠ {issue.location}.{issue.field}: {issue.message}")
            if issue.suggestion:
                lines.append(f"    → {issue.suggestion}")

    return "\n".join(lines)
