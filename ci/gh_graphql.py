#!/usr/bin/env -S uv run --no-project --script
"""Every GitHub GraphQL call in this repo goes through here, and from here
through ghd (forge-ci/ghd), not `gh api graphql`.

    uv run --no-project --script ci/gh_graphql.py -f query='{viewer{login}}'
    uv run --no-project --script ci/gh_graphql.py -f query='query($n:Int!){...}' -F n=5 --jq '.data...'

The arguments are `gh api graphql`'s: `-f`/`--raw-field name=value` (string),
`-F`/`--field name=value` (typed: true/false/null/integers, `@file`), and
`--jq EXPR`. Agents share one 5,000-point hourly GraphQL budget per account,
and re-check loops burn it. ghd's per-user daemon makes identical concurrent
queries one upstream call and reuses an answer for its fresh window
(`GHD_FRESH_MS`, 2 s). Mutations always go upstream. The PreToolUse hook
(ci/hooks/tool_guard.py) denies a bare `gh api graphql`, and
ci/test_tool_guard.py fails one written into a script, skill or doc.

ghd comes from `$GHD_EXE`, else `ghd` on PATH, else
`uvx --no-build --from ghd==<GHD_VERSION> ghd`. The published binary wheel
avoids compiling the Rust CLI on cold admission runners. Missing wheels fail
closed instead of starting a source build. `--jq` runs `jq -r`, which prints the same output as gh's
`--jq` (strings unquoted). The exit status is ghd's: 1 when GitHub answered
with `errors`.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys

# forge-ci/ghd v0.2.1 publishes wheels for all six supported targets.
# Keep cold identity verification inside its deadline: never build ghd here.
GHD_VERSION = "0.2.1"
GHD_SOURCE = f"ghd=={GHD_VERSION}"


def ghd_command() -> list[str]:
    exe = os.environ.get("GHD_EXE")
    if exe:
        return [exe]
    found = shutil.which("ghd")
    if found:
        return [found]
    uvx = shutil.which("uvx")
    if not uvx:
        raise SystemExit("gh_graphql: ghd not found; install uv or set GHD_EXE")
    return [uvx, "--no-build", "--from", GHD_SOURCE, "ghd"]


def split_jq(args: list[str]) -> tuple[list[str], str | None]:
    """gh's `--jq EXPR` / `-q EXPR` / `--jq=EXPR`, removed from `args`."""
    rest: list[str] = []
    jq = None
    it = iter(args)
    for arg in it:
        if arg in ("--jq", "-q"):
            jq = next(it, None)
            if jq is None:
                raise SystemExit(f"gh_graphql: {arg} needs an expression")
        elif arg.startswith("--jq="):
            jq = arg[len("--jq=") :]
        else:
            rest.append(arg)
    return rest, jq


def main(argv: list[str]) -> int:
    args, jq = split_jq(argv)
    if args[:2] == ["api", "graphql"]:  # tolerate a pasted `gh api graphql ...` tail
        args = args[2:]
    command = [*ghd_command(), "api", "graphql", *args]
    if jq is None:
        return subprocess.call(command)
    result = subprocess.run(command, stdout=subprocess.PIPE, check=False)
    if result.returncode != 0:
        sys.stdout.buffer.write(result.stdout)
        return result.returncode
    jq_exe = shutil.which("jq")
    if not jq_exe:
        raise SystemExit("gh_graphql: --jq needs jq on PATH")
    return subprocess.run([jq_exe, "-r", jq], input=result.stdout, check=False).returncode


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
