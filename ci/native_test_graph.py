"""Read the canonical native test graph (including test fixtures)."""
import json
from pathlib import Path


def load_graph(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))
