#!/usr/bin/env -S uv run --no-project --script
"""Client Format/Data evidence using the unchanged Actions validators.

Data and Qt checks validate the full tracked applicable set, even for an empty
BASE..HEAD diff. There are no head-owned scoped no-op proofs. Swift is separate
and optional: a missing formatter, unsupported filename or failed check cannot
stamp it. The receipt records the immutable diff and tool versions alongside
the tree-bound prooft gates; it is signed by ./push after prooft finishes.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

import run_swift_format
import swift_changes

ROOT = Path(__file__).resolve().parents[1]
CLIENT_CHECK_GATES = {
    "python/all/self-test", "data/all/flatpak-manifest", "data/all/qt-xml",
    "data/all/compatibility", "data/all/services-schema",
}


def git(root: Path, *args: str) -> str:
    return subprocess.run(["git", *args], cwd=root, text=True, capture_output=True, check=True).stdout


@dataclass(frozen=True)
class Inputs:
    base: str
    head: str
    tree: str
    changed: tuple[str, ...]
    tracked: tuple[str, ...]
    digest: str

    @classmethod
    def read(cls, root: Path, base: str) -> Inputs:
        if not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", base):
            raise ValueError("--base must be an immutable full commit SHA")
        git(root, "cat-file", "-e", f"{base}^{{commit}}")
        if git(root, "status", "--porcelain", "--untracked-files=normal").strip():
            raise ValueError("client checks require a clean committed tree")
        head = git(root, "rev-parse", "HEAD").strip()
        tracked = tuple(filter(None, git(root, "ls-files", "-z").split("\0")))
        changed = tuple(filter(None, git(root, "diff", "--no-ext-diff", "--no-renames", "--name-only", "-z", base, head).split("\0")))
        digest = hashlib.sha256()
        for name in tracked:
            path = root / name
            digest.update(os.fsencode(name) + b"\0")
            # Submodule commits are bound by the tree; do not traverse their files.
            if path.is_file():
                digest.update(path.read_bytes())
            digest.update(b"\0")
        return cls(base, head, git(root, "rev-parse", "HEAD^{tree}").strip(), changed, tracked, digest.hexdigest())


def command(args: list[str], *, env: dict[str, str] | None = None) -> None:
    print("+ " + " ".join(args), flush=True)
    subprocess.run(args, cwd=ROOT, env=env, check=True)


def tool_versions(swift: bool) -> dict[str, str]:
    probes = {"uv": ["uv", "--version"]}
    if swift:
        probes["swift-format"] = [run_swift_format.formatter_binary(), "--version"]
    else:
        probes["xmllint"] = ["xmllint", "--version"]
        probes["schema-dependencies"] = [
            "uv", "run", "--no-project", "--with", "jsonschema", "--with", "json_source_map", "python", "-c",
            "from importlib.metadata import version; import sys; print(sys.version); "
            "print({n: version(n) for n in ('jsonschema', 'json_source_map')})",
        ]
    versions = {"orchestrator-python": sys.version}
    for name, argv in probes.items():
        result = subprocess.run(argv, cwd=ROOT, text=True, capture_output=True, check=True)
        # Some tools (xmllint) report their version on stderr. Prefer stdout
        # when present: uv may add transient lock/install diagnostics to stderr.
        versions[name] = (result.stdout or result.stderr).strip()
    return versions


def receipt_path(root: Path, swift: bool) -> Path:
    directory = Path(git(root, "rev-parse", "--absolute-git-dir").strip()) / "client-checks"
    return directory / ("swift.json" if swift else "checks.json")


def full_checks(inputs: Inputs) -> None:
    print("client checks: full-file validation (no scoped no-op proofs)", flush=True)
    env = dict(os.environ)
    env.pop("OBS_GATE_SKIP_SELF_TEST", None)
    command(["uv", "run", "--no-project", "ci/local_gate.py", "--only", "self-test"], env=env)
    command(["uv", "run", "--no-project", "ci/flatpak_manifest_validate.py", "validate", "--manifest",
             "build-aux/io.github.obs_rust.OBSStudioRust.json", "--fail-condition", "error"])
    forms = [name for name in inputs.tracked if name.startswith("frontend/forms/") and name.endswith(".ui")]
    command(["uv", "run", "--no-project", "ci/qt_xml_validate.py", "validate", "--changed-files-json",
             json.dumps(forms), "--schema", "frontend/forms/XML-Schema-Qt5.15.xsd", "--fail-condition", "error"])
    command(["uv", "run", "--no-project", "ci/compatibility_validate.py", "validate",
             "plugins/win-capture/data/compatibility.json", "plugins/win-capture/data/package.json"])
    command(["uv", "run", "--no-project", "ci/services_validate.py", "schema",
             "plugins/rtmp-services/data/services.json", "plugins/rtmp-services/data/package.json"])


def swift_checks(inputs: Inputs) -> None:
    # Reuse the remote selector as a diagnostic, but prove all tracked Swift
    # files so this gate is never a claim about a head-owned scoped no-op.
    if not swift_changes.comparable(inputs.base):
        raise ValueError("Swift base is unavailable")
    print(f"Swift diff selected: {swift_changes.swift_changed(inputs.base)}", flush=True)
    if run_swift_format.formatter_version() is None:
        raise FileNotFoundError("swift-format is unavailable; keep the remote fallback")
    files = [name for name in inputs.tracked if name.endswith(".swift")]
    if not files or any(any(c in name for c in ",[]'\"\n\r") for name in files):
        # The shared wrapper's legacy CHANGED_FILES parser cannot represent
        # these paths losslessly. Do not invent a pass for them.
        raise ValueError("no representable tracked Swift inputs; keep the remote fallback")
    selected = [name for name in inputs.changed if name in files]
    env = dict(os.environ, CHANGED_FILES=",".join(selected))
    command(["uv", "run", "--no-project", "ci/run_swift_format.py", "format", "--fail-condition", "error"], env=env)
    # Empty CHANGED_FILES means the wrapper's default full set. Explicitly
    # include every tracked file too (including files outside its default globs).
    env["CHANGED_FILES"] = ",".join(files)
    command(["uv", "run", "--no-project", "ci/run_swift_format.py", "format", "--fail-condition", "error"], env=env)


def execute(base: str, *, swift: bool = False) -> int:
    try:
        before = Inputs.read(ROOT, base)
        versions = tool_versions(swift)
        (swift_checks if swift else full_checks)(before)
        if Inputs.read(ROOT, base) != before or tool_versions(swift) != versions:
            raise ValueError("client check inputs or tools changed during validation")
        record = {"base": base, "head": before.head, "tree": before.tree,
                  "diff": list(before.changed), "tracked-digest": before.digest,
                  "tools": versions, "coverage": "all-tracked-applicable-files"}
        path = receipt_path(ROOT, swift)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(record, sort_keys=True) + "\n")
    except (OSError, ValueError, subprocess.CalledProcessError) as exc:
        print(f"client checks: no proof: {exc}", file=sys.stderr)
        return 75 if swift else 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True, help="immutable comparison commit (not a ref)")
    parser.add_argument("--swift", action="store_true", help="optional Swift proof only")
    args = parser.parse_args()
    return execute(args.base, swift=args.swift)


if __name__ == "__main__":
    sys.exit(main())
