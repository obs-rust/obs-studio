#!/usr/bin/env -S uv run --script
r"""Run flatpak-builder-lint for a build artifact and annotate its findings (#2700).

Ports the two inline `run:` blocks of
`.github/actions/flatpak-builder-lint/action.yaml` into one Python module so the
composite action stops carrying shell (GEN-005). Behaviour is the shell's:

`guard` refuses an unknown artifact kind the way the shell's `=~` test did --
`::error::Given artifact type is incorrect` and exit 2 -- because
`flatpak-builder-lint` takes an artifact kind as its first argument and would
otherwise be asked to lint something the action never supported.

`lint` runs

    flatpak-builder-lint --exceptions --user-exceptions <exceptions.json> <artifact> <path>

and turns its JSON body into GitHub annotations:

- `warnings` and `errors` arrays each become one `::warning::`/`::error::` line
  suffixed with ` found in the Flatpak <artifact>`;
- a non-zero exit with an empty body is the tool failing to run at all -- exit 2
  with `::error::Error while running flatpak-builder-lint`;
- the `appstream` artifact is a validity report rather than a lint result, so its
  body is printed verbatim and the tool's exit code is returned, with
  `::error::Flatpak appstream info is not valid` when it is non-zero;
- every other artifact returns the tool's exit code after the annotations.

Three deliberate differences from the shell, all documented in the PR:

- the annotation suffix is now what the shell meant it to be. The old block set
  `template=" found in the Flatpak <artifact>"` but interpolated it inside a
  single-quoted `jq` program, so jq printed the literal text `${template}` and every
  annotation ended `…${template}` (`jq` interpolates with `\(…)`, not `${…}`). The
  port emits `… found in the Flatpak <artifact>`, which is what the block's own
  comment above the pipeline describes;
- a body that is not JSON produces no annotations instead of a `jq` parse error
  and exit 5 (the old pipeline ran under `-eo pipefail`). The real tool always
  prints JSON, so this only changes what a broken tool reports, and it does so in
  the direction of not masking the tool's own message;
- a `warnings`/`errors` value that is not a list is ignored rather than aborting
  the annotation pass, matching the `try` the original `jq` filter wrapped around
  `map`.

Standard library only: the flatpak job that calls the action sets uv up before
it, and the module needs nothing else.

    uv run --no-project ci/flatpak_builder_lint.py guard --artifact manifest
    uv run --no-project ci/flatpak_builder_lint.py lint --artifact builddir --path flatpak_app \\
        --exceptions "${{ github.action_path }}/exceptions.json"
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections.abc import Sequence

ARTIFACTS = ("builddir", "repo", "manifest", "appstream")


def guard(artifact: str) -> int:
    """Reject an artifact kind the linter cannot lint."""
    if artifact not in ARTIFACTS:
        print("::error::Given artifact type is incorrect")
        return 2
    return 0


def run_linter(argv: Sequence[str], cwd: str | None = None) -> tuple[int, str]:
    """Run the linter, returning its exit code and stdout."""
    # The tool name and arguments come from the action's own inputs.
    result = subprocess.run(
        [*argv],
        cwd=cwd,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.stderr:
        print(result.stderr, file=sys.stderr, end="" if result.stderr.endswith("\n") else "\n")
    return result.returncode, result.stdout


def annotation_lines(result: str, artifact: str) -> list[str]:
    """The `::warning::`/`::error::` lines for a lint body."""
    try:
        document = json.loads(result)
    except json.JSONDecodeError:
        return []
    if not isinstance(document, dict):
        return []
    lines: list[str] = []
    for key, level in (("warnings", "warning"), ("errors", "error")):
        entries = document.get(key)
        if not isinstance(entries, list):
            continue
        lines.extend(
            f"::{level}:: {entry} found in the Flatpak {artifact}"
            for entry in entries
            if isinstance(entry, str) and entry
        )
    return lines


def lint(artifact: str, path: str, exceptions: str, tool: str) -> int:
    """Lint one artifact and annotate the findings; returns the process exit code."""
    code, result = run_linter(
        [tool, "--exceptions", "--user-exceptions", exceptions, artifact, path]
    )
    if code != 0 and not result.strip():
        print("::error::Error while running flatpak-builder-lint")
        return 2
    if artifact == "appstream":
        print(result, end="" if result.endswith("\n") else "\n")
        if code != 0:
            print("::error::Flatpak appstream info is not valid")
        return code
    for line in annotation_lines(result, artifact):
        print(line)
    return code


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)

    check = commands.add_parser("guard", help="reject an unsupported artifact kind")
    check.add_argument("--artifact", required=True)

    run = commands.add_parser("lint", help="lint an artifact and annotate the findings")
    run.add_argument("--artifact", required=True)
    run.add_argument("--path", required=True)
    run.add_argument("--exceptions", required=True, help="path to the action's exceptions.json")
    run.add_argument("--tool", default="flatpak-builder-lint")

    args = parser.parse_args(argv)
    if args.command == "guard":
        return guard(args.artifact)
    if guard(args.artifact) != 0:
        return 2
    return lint(args.artifact, args.path, args.exceptions, args.tool)


if __name__ == "__main__":
    sys.exit(main())
