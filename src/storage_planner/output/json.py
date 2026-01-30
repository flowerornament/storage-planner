"""JSON output helpers."""

from __future__ import annotations

import json
from dataclasses import asdict, is_dataclass
from enum import Enum
from pathlib import Path
from typing import Any

from pydantic import BaseModel


def to_jsonable(value: Any) -> Any:
    """Convert common objects (dataclasses, enums, Paths) to JSON-serializable form."""
    if is_dataclass(value):
        return to_jsonable(asdict(value))
    if isinstance(value, BaseModel):
        return to_jsonable(value.model_dump())
    if isinstance(value, Enum):
        return value.value
    if isinstance(value, Path):
        return str(value)
    if isinstance(value, set):
        return [to_jsonable(v) for v in sorted(value, key=lambda x: str(x))]
    if isinstance(value, dict):
        return {k: to_jsonable(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [to_jsonable(v) for v in value]
    return value


def print_json(data: Any) -> None:
    """Print JSON data to stdout."""
    print(json.dumps(to_jsonable(data), indent=2, sort_keys=True))
