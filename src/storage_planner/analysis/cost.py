"""Cost analysis for storage planner."""

from dataclasses import dataclass, field

from storage_planner.models import HardwareCatalog, MarketPrices, Topology


class CostConfigError(Exception):
    """Raised when cost analysis lacks required configuration."""

    pass


@dataclass
class NodeCost:
    """Cost breakdown for a node."""

    node_id: str
    node_name: str
    monthly_hosting: float = 0.0
    monthly_power: float = 0.0
    hardware_cost: float = 0.0
    notes: list[str] = field(default_factory=list)


@dataclass
class CostSummary:
    """Overall cost summary."""

    nodes: list[NodeCost]
    total_monthly: float
    total_hardware: float
    five_year_projection: float
    breakdown_notes: list[str]


def analyze_cost(
    topology: Topology,
    hardware_catalog: HardwareCatalog | None = None,
    market_prices: MarketPrices | None = None,
    power_cost_per_kwh: float | None = None,  # Override from CLI
) -> CostSummary:
    """Analyze costs for the topology.

    Calculates:
    - Monthly operational costs (hosting, power)
    - Hardware costs (from catalog if available)
    - 5-year total cost projection

    Power cost is taken from:
    1. power_cost_per_kwh parameter (CLI override)
    2. topology.constraints.power_cost_per_kwh

    Raises:
        CostConfigError: If power cost calculation is needed but not configured.
    """
    # Resolve power cost: CLI override > topology config
    effective_power_cost = power_cost_per_kwh or topology.constraints.power_cost_per_kwh

    node_costs: list[NodeCost] = []
    notes: list[str] = []

    for node in topology.nodes:
        nc = NodeCost(node_id=node.id, node_name=node.name)

        # Monthly hosting cost
        if node.monthly_cost:
            nc.monthly_hosting = node.monthly_cost

        # Estimate power cost for always-on devices
        if node.power_profile and node.power_profile.value == "always_on":
            if node.power_watts_idle:
                if effective_power_cost is None:
                    raise CostConfigError(
                        f"Node '{node.id}' has power_watts_idle set but no power cost configured. "
                        "Add 'power_cost_per_kwh' to constraints or use --power-cost CLI flag."
                    )
                # Monthly power = watts * hours * cost / 1000
                # 720 hours = 24 * 30 (average month)
                hours_per_month = 720
                kwh_per_month = (node.power_watts_idle * hours_per_month) / 1000
                nc.monthly_power = kwh_per_month * effective_power_cost
                nc.notes.append(f"Power estimate: {node.power_watts_idle}W idle @ ${effective_power_cost}/kWh")

        # Hardware costs from volumes
        for volume in node.volumes:
            if volume.purchase_cost:
                nc.hardware_cost += volume.purchase_cost
            elif volume.product_id and hardware_catalog:
                product = hardware_catalog.get_product(volume.product_id)
                if product and product.retail_price:
                    nc.hardware_cost += product.retail_price
                    nc.notes.append(f"{volume.id}: ${product.retail_price} (retail)")

                    # Check for used market price
                    if market_prices:
                        mp = market_prices.get_best_price(volume.product_id)
                        if mp:
                            nc.notes.append(
                                f"  Used market: ${mp.price_low}-${mp.price_high}"
                            )

        node_costs.append(nc)

    # Calculate totals
    total_monthly = sum(nc.monthly_hosting + nc.monthly_power for nc in node_costs)
    total_hardware = sum(nc.hardware_cost for nc in node_costs)

    # 5-year projection: hardware + 60 months of operational
    five_year = total_hardware + (total_monthly * 60)

    # Check against constraints
    if topology.constraints.max_monthly_cost:
        if total_monthly > topology.constraints.max_monthly_cost:
            notes.append(
                f"Warning: Monthly cost ${total_monthly:.2f} exceeds "
                f"constraint ${topology.constraints.max_monthly_cost:.2f}"
            )

    return CostSummary(
        nodes=node_costs,
        total_monthly=total_monthly,
        total_hardware=total_hardware,
        five_year_projection=five_year,
        breakdown_notes=notes,
    )
