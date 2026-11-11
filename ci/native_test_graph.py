"""Deterministically assemble per-target native test graph registrations.

Single JSON documents remain supported for isolated test fixtures, not as a
second canonical inventory. Fragment names own one target and its test entries.
"""
import json
from pathlib import Path


def read_fragment(fragment: Path) -> dict:
    value = json.loads(fragment.read_text(encoding="utf-8"))
    if set(value) != {"schema", "targets", "tests"} or type(value["schema"]) is not int or value["schema"] != 1:
        raise ValueError(f"invalid native test graph fragment: {fragment}")
    if not isinstance(value["targets"], dict) or list(value["targets"]) != [fragment.stem]:
        raise ValueError(f"fragment must own target {fragment.stem}: {fragment}")
    if not isinstance(value["tests"], dict):
        raise ValueError(f"fragment tests must be an object: {fragment}")
    for name, test in value["tests"].items():
        if not isinstance(test, dict) or test.get("target") != fragment.stem:
            raise ValueError(f"test {name} must reference owning target {fragment.stem}")
    return value


def load_graph(path: Path) -> dict:
    if not path.is_dir():
        return json.loads(path.read_text(encoding="utf-8"))
    graph = {"schema": 1, "targets": {}, "tests": {}}
    fragments = sorted(path.glob("*.json"))
    if not fragments:
        raise ValueError(f"empty native test graph directory: {path}")
    for fragment in fragments:
        value = read_fragment(fragment)
        for section in ("targets", "tests"):
            duplicates = graph[section].keys() & value[section].keys()
            if duplicates:
                raise ValueError(f"duplicate native test {section}: {sorted(duplicates)}")
            graph[section].update(value[section])
    for section in ("targets", "tests"):
        graph[section] = dict(sorted(graph[section].items()))
    return graph
