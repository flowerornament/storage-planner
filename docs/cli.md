# CLI Reference

## Defaults

- If no topology path is provided, commands default to `./topology.yaml`.

## JSON Output

Most commands accept `--json` for machine-readable output. This suppresses rich tables
and prints a structured JSON payload to stdout.

Example:

```bash
sp analyze redundancy --json
sp simulate laptop --json
sp catalog list --json
```

## Analyze

```bash
sp analyze all <topology.yaml>
sp analyze quick <topology.yaml>       # Summaries only (redundancy, RPO/RTO, capacity)
sp analyze diff <a.yaml> <b.yaml>       # Compare analysis summaries
```

## Simulate

```bash
sp simulate <node|volume> <topology.yaml>
sp simulate diff <node|volume> <a.yaml> <b.yaml>
```

## Suggest

```bash
sp suggest hardware <topology.yaml> -c catalog
sp suggest software <topology.yaml> -c catalog
```

Hardware suggestions include catalog-aware recommendations for redundancy gaps
(capacity, use case, and noise constraints when available).
