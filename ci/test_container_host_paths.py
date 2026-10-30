#!/usr/bin/env -S uv run --script
"""No runner-host path interpolation inside a container job's `run:` (#3035).

`${{ github.workspace }}` and `${{ runner.* }}` paths interpolate the runner
host path (`/home/runner/work/...`). The runner translates them in `env:`,
`with:` and `working-directory:`, but not inside a `run:` script, where the
container sees `/__w/...`. That broke the Flatpak lint action (#2705) and the
Tier 2 zccache bound (#3026). Use `$GITHUB_WORKSPACE` / `$RUNNER_TEMP` there.

    uv run python -m unittest discover -s ci -p test_container_host_paths.py
"""

import re
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HOST_PATH = re.compile(r"\$\{\{\s*(github\.workspace|runner\.(temp|tool_cache|workspace)|github\.action_path)\s*\}\}")


def run_scripts(text):
    """Yield (line number, script) for every `run:` in a YAML file."""
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        m = re.match(r"^(\s*)(?:- )?run:\s*(.*)$", lines[i])
        if not m:
            i += 1
            continue
        start, indent, rest = i + 1, len(m[1]), m[2]
        body = [] if rest[:1] in ("|", ">") else [rest]
        i += 1
        if not body:
            while i < len(lines) and (not lines[i].strip() or len(lines[i]) - len(lines[i].lstrip()) > indent):
                body.append(lines[i])
                i += 1
        yield start, "\n".join(body)


def container_jobs(text):
    """Yield (job name, job text) for every job with a `container:`."""
    if "\njobs:\n" not in text:
        return
    parts = re.split(r"^  ([A-Za-z0-9_-]+):\s*$", text.split("\njobs:\n", 1)[1], flags=re.M)
    for name, body in zip(parts[1::2], parts[2::2]):
        if re.search(r"^    container:", body, re.M):
            yield name, body


def violations(workflow_text, root=ROOT):
    found = []
    for job, body in container_jobs(workflow_text):
        for line, script in run_scripts(body):
            if HOST_PATH.search(script):
                found.append(f"{job}: run at job line {line}")
        for action in sorted(set(re.findall(r"uses:\s*\./(\.github/actions/[^\s@]+)", body))):
            for name in ("action.yml", "action.yaml"):
                path = root / action / name
                if path.is_file():
                    for line, script in run_scripts(path.read_text()):
                        if HOST_PATH.search(script):
                            found.append(f"{job}: {action}/{name}:{line}")
    return found


class ContainerHostPathsTest(unittest.TestCase):
    def test_detects_the_3026_bound(self):
        workflow = (
            "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n"
            "    container:\n      image: ubuntu\n    steps:\n"
            "      - name: Bound\n        env:\n          A: ${{ github.workspace }}\n"
            "        run: |\n          dir=\"${{ github.workspace }}/.cache\"\n"
            "          du -sm \"$dir\"\n"
            "  host:\n    runs-on: ubuntu-latest\n    steps:\n"
            "      - run: echo ${{ github.workspace }}\n"
        )
        self.assertEqual(len(violations(workflow)), 1)

    def test_workflows(self):
        found = []
        for path in sorted((ROOT / ".github/workflows").glob("*.y*ml")):
            found += [f"{path.name}: {v}" for v in violations(path.read_text())]
        self.assertEqual(found, [], "use $GITHUB_WORKSPACE / $RUNNER_TEMP in container run scripts")


if __name__ == "__main__":
    if len(sys.argv) > 1 and not sys.argv[1].startswith("-"):
        # RED evidence: check a given workflow file (e.g. a pre-#3028 copy).
        for arg in sys.argv[1:]:
            print("\n".join(violations(Path(arg).read_text())) or "clean")
    else:
        unittest.main()
