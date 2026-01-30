"""RPO/RTO analysis for storage planner."""

from dataclasses import dataclass
from typing import Optional

from storage_planner.models import Topology
from storage_planner.analysis.utils import parse_duration, format_duration
from croniter import croniter
from datetime import datetime


@dataclass
class RpoRtoResult:
    """Result of RPO/RTO analysis for a dataset."""

    dataset_id: str
    dataset_name: str
    max_rpo: Optional[str]  # Required RPO
    max_rto: Optional[str]  # Required RTO
    achieved_rpo: Optional[str]  # Best achieved RPO from sync regimes
    achieved_rpo_source: str  # "explicit", "estimated", "unknown"
    rpo_met: Optional[bool]  # None if can't determine
    sync_regimes: list[str]  # Sync regime IDs covering this dataset


def _estimate_rpo_from_schedule(schedule: str) -> Optional[int]:
    """Estimate RPO in seconds from a cron schedule."""
    if not schedule:
        return None
    try:
        base = datetime(2025, 1, 1, 0, 0, 0)
        itr = croniter(schedule, base)
        t1 = itr.get_next(datetime)
        t2 = itr.get_next(datetime)
    except Exception:
        return None
    return int((t2 - t1).total_seconds())


def analyze_rpo_rto(topology: Topology) -> list[RpoRtoResult]:
    """Analyze RPO/RTO compliance for all datasets.

    For each dataset, finds sync regimes that sync it and determines
    the best achieved RPO. Compares against max_rpo requirement.
    """
    results = []

    for dataset in topology.datasets:
        # Find all sync regimes for this dataset
        regimes = [r for r in topology.sync_regimes if r.dataset == dataset.id]
        regime_ids = [r.id for r in regimes]

        # Find the best (smallest) achieved RPO
        achieved_rpo: Optional[str] = None
        achieved_rpo_seconds: Optional[int] = None
        achieved_rpo_source = "unknown"

        for regime in regimes:
            if regime.achieved_rpo:
                rpo_sec = parse_duration(regime.achieved_rpo)
                if rpo_sec is not None:
                    if achieved_rpo_seconds is None or rpo_sec < achieved_rpo_seconds:
                        achieved_rpo_seconds = rpo_sec
                        achieved_rpo = regime.achieved_rpo
                        achieved_rpo_source = "explicit"
            elif regime.schedule:
                estimated = _estimate_rpo_from_schedule(regime.schedule)
                if estimated is not None:
                    if achieved_rpo_seconds is None or estimated < achieved_rpo_seconds:
                        achieved_rpo_seconds = estimated
                        achieved_rpo = format_duration(estimated)
                        achieved_rpo_source = "estimated"
            # Note: We do NOT assume RPO for continuous sync.
            # Users must specify achieved_rpo explicitly.

        # Check if RPO requirement is met
        rpo_met: Optional[bool] = None
        if dataset.max_rpo and achieved_rpo_seconds is not None:
            required_rpo_seconds = parse_duration(dataset.max_rpo)
            if required_rpo_seconds is not None:
                rpo_met = achieved_rpo_seconds <= required_rpo_seconds

        results.append(
            RpoRtoResult(
                dataset_id=dataset.id,
                dataset_name=dataset.name,
                max_rpo=dataset.max_rpo,
                max_rto=dataset.max_rto,
                achieved_rpo=achieved_rpo,
                achieved_rpo_source=achieved_rpo_source,
                rpo_met=rpo_met,
                sync_regimes=regime_ids,
            )
        )

    return results
