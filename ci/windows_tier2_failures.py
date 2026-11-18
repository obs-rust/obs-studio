#!/usr/bin/env -S uv run --script
"""Map every unbuilt Windows CTest to failed Ninja edges, with log evidence."""

import argparse
from collections import defaultdict
import json
from pathlib import Path
import re
import subprocess


def failed_edges(log: str) -> dict:
    result = {}
    lines = log.splitlines()
    for index, line in enumerate(lines):
        if not line.startswith("FAILED:"):
            continue
        outputs = re.sub(r"^FAILED: (?:\[code=\d+\] )?", "", line).split()
        end = next(
            (
                n
                for n in range(index + 1, len(lines))
                if re.match(r"^\[\d+/\d+\]|^FAILED:", lines[n])
            ),
            len(lines),
        )
        details = [
            text.strip()
            for text in lines[index + 1 : end]
            if "error:" in text
            or "undefined symbol:" in text
            or "Error parsing" in text
        ]
        evidence = {"line": index + 1, "diagnostics": details[:40]}
        for output in outputs:
            result[output] = evidence
    return result


def dependency_failures(graph: str, failed: dict, output: str) -> list[dict]:
    labels, edges = {}, defaultdict(set)
    for line in graph.splitlines():
        label = re.match(r'^"(0x[0-9a-f]+)" \[label="((?:[^"\\]|\\.)*)"', line)
        if label:
            labels[label[1]] = json.loads('"' + label[2] + '"')
        edge = re.match(r'^"(0x[0-9a-f]+)" -> "(0x[0-9a-f]+)"', line)
        if edge:
            edges[edge[2]].add(edge[1])
    pending = [node for node, label in labels.items() if label == output]
    seen, hits = set(), {}
    while pending:
        node = pending.pop()
        if node in seen:
            continue
        seen.add(node)
        label = labels.get(node)
        if label in failed:
            hits[label] = {"output": label, **failed[label]}
        pending.extend(edges[node])
    return [hits[name] for name in sorted(hits)]


def report(build: Path, log_path: Path, inventory: dict) -> dict:
    graph = subprocess.check_output(
        ["ninja", "-C", str(build), "-t", "graph", "tier2-cross-full"], text=True
    )
    failed = failed_edges(log_path.read_text(errors="replace"))
    blockers = []
    for name in inventory["unbuilt_tests"]:
        causes = dependency_failures(graph, failed, f"test/cmocka/{name}.exe")
        blockers.append(
            {
                "target": name,
                "owner": "in-repo",
                "evidence_log": str(log_path),
                "failed_edges": causes,
                "status": "failed" if causes else "not-produced",
            }
        )
    for name in inventory["missing_native_tests"]:
        blockers.append(
            {
                "target": name,
                "owner": "in-repo",
                "status": "missing-real-owner-graph",
                "evidence": "absent from producer CTest JSON; native snapshot includes it",
            }
        )
    return {
        "complete": False,
        "failed_edges": failed,
        "blocked_tests": blockers,
        "toolchain_blocker": "soldr targeted GNU driver rejects clang-cl mode with injected GNU SDK arguments; Cargo native builder requires MSVC frontend options",
        "native_contract_status": "not produced; no Windows compilation fallback",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument("--log", type=Path, required=True)
    parser.add_argument("--inventory", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = report(args.build, args.log, json.loads(args.inventory.read_text()))
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(
        f"{len(result['failed_edges'])} failed build outputs; {len(result['blocked_tests'])} blocked tests. Full evidence: {args.output}"
    )


if __name__ == "__main__":
    main()
