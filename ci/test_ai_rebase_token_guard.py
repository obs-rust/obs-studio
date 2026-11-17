#!/usr/bin/env -S uv run --script
"""The AI rebase job skips its steps when AI_REBASE_TOKEN is not set (#4461).

    uv run python -m unittest ci/test_ai_rebase_token_guard.py

`actions/checkout` failed with "Input required and not supplied: token" on
every master push while the repository had no AI_REBASE_TOKEN secret, so an
optional workflow showed a red run. GitHub does not allow the `secrets`
context in `jobs.<id>.if` (actionlint error; #4730 tried it and was closed),
so the job exposes whether the token is set through its `env` and every step
is guarded on that variable: with no token the run only prints a notice.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "ai-rebase.yaml"
SECRET = "secrets.AI_REBASE_TOKEN"


def jobs(text: str) -> dict[str, str]:
    """Each job's name and body (the lines below `  <name>:`)."""
    body = text.split("\njobs:\n", 1)[1]
    parts = re.split(r"^  ([A-Za-z0-9_-]+):\n", body, flags=re.M)
    return dict(zip(parts[1::2], parts[2::2]))


def job_if(job: str) -> str:
    m = re.search(r"^    if: *(.*)\n((?:      .*\n)*)", job, re.M)
    return "" if not m else " ".join([m.group(1), *m.group(2).split()])


def token_flag(job: str) -> str | None:
    """The job-level env variable set from `secrets.AI_REBASE_TOKEN != ''`."""
    env = re.search(r"^    env:\n((?:      .*\n)*)", job, re.M)
    if not env:
        return None
    for name, value in re.findall(r"^      ([A-Za-z0-9_]+): *(.*)$", env.group(1), re.M):
        if re.fullmatch(r"\$\{\{ *" + re.escape(SECRET) + r" *!= *'' *\}\}", value.strip()):
            return name
    return None


def steps(job: str) -> list[str]:
    """Each step's text, split on the `      - ` items under `    steps:`."""
    m = re.search(r"^    steps:\n((?:(?:      .*)?\n)*)", job, re.M)
    if not m:
        return []
    return [s for s in re.split(r"^      - ", m.group(1), flags=re.M) if s.strip()]


def step_if(step: str) -> str:
    m = re.search(r"^(?:|        )if: *(.*)$", step, re.M)
    return m.group(1).strip() if m else ""


def problems(text: str) -> list[str]:
    found = []
    for name, job in jobs(text).items():
        if "secrets" in job_if(job):
            found.append(f"{name}: `secrets` in the job `if:` (GitHub rejects it)")
        if SECRET not in job:
            continue
        flag = token_flag(job)
        if flag is None:
            found.append(f"{name}: no job env variable set from `{SECRET} != ''`")
            continue
        for step in steps(job):
            cond = step_if(step)
            if not re.search(r"\benv\." + flag + r" *[!=]= *'true'", cond):
                first = step.splitlines()[0].strip()
                found.append(f"{name}: step `{first}` is not guarded on env.{flag}")
    return found


UNGUARDED = """\
on: push

jobs:
  rebase:
    if: github.event_name != 'schedule'
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
        with:
          token: ${{ secrets.AI_REBASE_TOKEN }}
      - name: Rebase
        run: echo
"""

SECRETS_IN_IF = """\
on: push

jobs:
  rebase:
    if: secrets.AI_REBASE_TOKEN != ''
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
        with:
          token: ${{ secrets.AI_REBASE_TOKEN }}
"""

GUARDED = """\
on: push

jobs:
  rebase:
    if: github.event_name != 'schedule'
    runs-on: ubuntu-24.04
    env:
      HAS_TOKEN: ${{ secrets.AI_REBASE_TOKEN != '' }}
    steps:
      - name: Skip
        if: env.HAS_TOKEN != 'true'
        run: echo skip

      - uses: actions/checkout@v4
        if: env.HAS_TOKEN == 'true'
        with:
          token: ${{ secrets.AI_REBASE_TOKEN }}
      - name: Rebase
        if: env.HAS_TOKEN == 'true'
        run: echo
"""


class TokenGuardTest(unittest.TestCase):
    def test_requeue_runs_without_push_token_with_minimal_permissions(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertRegex(text, r"permissions:\n  contents: read\n  pull-requests: write")
        shared = steps(jobs(text)["rebase"])
        requeue = next(s for s in shared if "--requeue-ejected" in s)
        self.assertEqual(step_if(requeue), "")
        self.assertIn("GH_TOKEN: ${{ github.token }}", requeue)
        checkout = next(s for s in shared if "uses: actions/checkout@" in s)
        self.assertEqual(step_if(checkout), "")
        self.assertIn("secrets.AI_REBASE_TOKEN || github.token", checkout)

    def test_checker_catches_unguarded_steps(self) -> None:
        self.assertEqual(
            problems(UNGUARDED),
            ["rebase: no job env variable set from `secrets.AI_REBASE_TOKEN != ''`"],
        )
        broken = GUARDED.replace("        if: env.HAS_TOKEN == 'true'\n        run: echo\n", "        run: echo\n")
        self.assertEqual(problems(broken), ["rebase: step `name: Rebase` is not guarded on env.HAS_TOKEN"])

    def test_checker_rejects_secrets_in_job_if(self) -> None:
        self.assertIn("rebase: `secrets` in the job `if:` (GitHub rejects it)", problems(SECRETS_IN_IF))

    def test_checker_accepts_guarded_job(self) -> None:
        self.assertEqual(problems(GUARDED), [])

    def test_real_workflow_is_guarded(self) -> None:
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn(SECRET, text)
        self.assertEqual(problems(text), [])

    def test_real_workflow_says_why_it_skipped(self) -> None:
        job = jobs(WORKFLOW.read_text(encoding="utf-8"))["rebase"]
        flag = token_flag(job)
        notices = [s for s in steps(job) if f"env.{flag} != 'true'" in step_if(s)]
        self.assertEqual(len(notices), 1)
        self.assertIn("::notice", notices[0])


if __name__ == "__main__":
    unittest.main()
