#!/usr/bin/env -S uv run --script
"""The AI rebase jobs skip their push steps when the ai-rebase App is not configured (#4461).

    uv run python -m unittest ci/test_ai_rebase_token_guard.py

`actions/checkout` failed with "Input required and not supplied: token" on
every master push while the repository had no AI_REBASE_TOKEN secret, so an
optional workflow showed a red run. GitHub does not allow the `secrets`
context in `jobs.<id>.if` (actionlint error; #4730 tried it and was closed),
so push/resolution steps check a job `env` flag. Read-only checkout, uv and
capacity reads/PR reruns also run without the push token.

The push token is minted per run from the obs-rust-ai-rebase GitHub App
(`vars.AI_REBASE_APP_ID`, `secrets.AI_REBASE_APP_PRIVATE_KEY`) by a step with
`id: app`, so no long-lived token is stored. The job flag is set from
`vars.AI_REBASE_APP_ID != ''`, and the minting step is itself guarded on it.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "ai-rebase.yaml"
SECRET = "steps.app.outputs.token"
FLAG_SOURCE = "vars.AI_REBASE_APP_ID"


def jobs(text: str) -> dict[str, str]:
    """Each job's name and body (the lines below `  <name>:`)."""
    body = text.split("\njobs:\n", 1)[1]
    parts = re.split(r"^  ([A-Za-z0-9_-]+):\n", body, flags=re.M)
    return dict(zip(parts[1::2], parts[2::2]))


def job_if(job: str) -> str:
    m = re.search(r"^    if: *(.*)\n((?:      .*\n)*)", job, re.M)
    return "" if not m else " ".join([m.group(1), *m.group(2).split()])


def token_flag(job: str) -> str | None:
    """The job-level env variable set from `vars.AI_REBASE_APP_ID != ''`."""
    env = re.search(r"^    env:\n((?:      .*\n)*)", job, re.M)
    if not env:
        return None
    for name, value in re.findall(r"^      ([A-Za-z0-9_]+): *(.*)$", env.group(1), re.M):
        if re.fullmatch(r"\$\{\{ *" + re.escape(FLAG_SOURCE) + r" *!= *'' *\}\}", value.strip()):
            return name
    return None


def steps(job: str) -> list[str]:
    """Each step's text, split on the `      - ` items under `    steps:`."""
    m = re.search(r"^    steps:\n((?:(?:      .*)?\n)*)", job, re.M)
    if not m:
        return []
    return [s for s in re.split(r"^      - ", m.group(1), flags=re.M)[1:] if s.strip()]


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
            found.append(f"{name}: no job env variable set from `{FLAG_SOURCE} != ''`")
            continue
        for step in steps(job):
            cond = step_if(step)
            if (("uses: actions/checkout@" in step and f"{SECRET} || github.token" in step)
                    or "uses: astral-sh/setup-uv@" in step
                    or ("ci/runner_loss_retry.py" in step
                        and f"GH_TOKEN: ${{{{ {SECRET} || github.token }}}}" in step)
                    or ("ci/mq_acquire_retry.py" in step
                        and f"GH_TOKEN: ${{{{ {SECRET} || github.token }}}}" in step)
                    or (name in ("capacity-recovery", "runner-loss-retry") and "uses: actions/checkout@" in step
                        and "persist-credentials: false" in step and "ref: master" in step)):
                continue  # these steps need no push credential
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
          token: ${{ steps.app.outputs.token }}
      - name: Rebase
        run: echo
"""

SECRETS_IN_IF = """\
on: push

jobs:
  rebase:
    if: secrets.AI_REBASE_APP_PRIVATE_KEY != ''
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
        with:
          token: ${{ steps.app.outputs.token }}
"""

GUARDED = """\
on: push

jobs:
  rebase:
    if: github.event_name != 'schedule'
    runs-on: ubuntu-24.04
    env:
      HAS_TOKEN: ${{ vars.AI_REBASE_APP_ID != '' }}
    steps:
      - name: Skip
        if: env.HAS_TOKEN != 'true'
        run: echo skip

      - uses: actions/checkout@v4
        if: env.HAS_TOKEN == 'true'
        with:
          token: ${{ steps.app.outputs.token }}
      - name: Rebase
        if: env.HAS_TOKEN == 'true'
        run: echo
"""


class TokenGuardTest(unittest.TestCase):
    def test_queue_recovery_requires_triggering_token_but_pr_retry_does_not(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertRegex(text, r"permissions:\n  contents: read\n  pull-requests: write")
        shared = steps(jobs(text)["capacity-recovery"])
        requeue = next(s for s in shared if "--requeue-ejected" in s)
        self.assertIn("env.HAS_TOKEN == 'true'", step_if(requeue))
        self.assertIn("GH_TOKEN: ${{ steps.app.outputs.token }}", requeue)
        checkout = next(s for s in shared if "uses: actions/checkout@" in s)
        self.assertEqual(step_if(checkout), "")
        self.assertIn("persist-credentials: false", checkout)
        retry_step = next(s for s in shared if "ci/mq_acquire_retry.py" in s)
        self.assertEqual(step_if(retry_step), "")
        self.assertIn("steps.app.outputs.token || github.token", retry_step)

    def test_checker_catches_unguarded_steps(self) -> None:
        self.assertEqual(
            problems(UNGUARDED),
            ["rebase: no job env variable set from `vars.AI_REBASE_APP_ID != ''`"],
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

    def test_real_workflow_mints_the_app_token_first(self) -> None:
        """Each job that pushes mints its token from the App before any step reads it,
        and no long-lived token secret is read anywhere."""
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertNotIn("secrets.AI_REBASE_TOKEN", text)
        for name in ("capacity-recovery", "rebase"):
            job = jobs(text)[name]
            flag = token_flag(job)
            self.assertIsNotNone(flag, name)
            job_steps = steps(job)
            mint = [i for i, s in enumerate(job_steps) if "uses: actions/create-github-app-token@" in s]
            self.assertEqual(len(mint), 1, name)
            step = job_steps[mint[0]]
            self.assertRegex(step, r"(?m)^        id: app$")
            self.assertIn(f"env.{flag} == 'true'", step_if(step))
            self.assertIn("app-id: ${{ vars.AI_REBASE_APP_ID }}", step)
            self.assertIn("private-key: ${{ secrets.AI_REBASE_APP_PRIVATE_KEY }}", step)
            readers = [i for i, s in enumerate(job_steps) if SECRET in s and i != mint[0]]
            self.assertTrue(readers, name)
            self.assertLess(mint[0], min(readers), name)

    def test_rebase_job_installs_the_agent_clud_drives(self) -> None:
        """`clud --openrouter` runs Claude Code; without it every conflicted
        rebase is refused with "claude not found on PATH" (run 45005866838)."""
        job_steps = steps(jobs(WORKFLOW.read_text(encoding="utf-8"))["rebase"])
        names = [re.search(r"name: (.+)", s).group(1) if re.search(r"name: (.+)", s) else "" for s in job_steps]
        install = [i for i, s in enumerate(job_steps)
                   if re.search(r"npm install --global @anthropic-ai/claude-code@\d+\.\d+\.\d+\s*$", s, re.M)]
        self.assertEqual(len(install), 1, "Claude Code is not installed at a pinned version")
        self.assertIn("env.HAS_TOKEN == 'true'", step_if(job_steps[install[0]]))
        self.assertLess(names.index("Install clud"), install[0])
        self.assertLess(install[0], names.index("Rebase"))

    def test_real_workflow_says_why_it_skipped(self) -> None:
        job = jobs(WORKFLOW.read_text(encoding="utf-8"))["rebase"]
        flag = token_flag(job)
        notices = [s for s in steps(job) if f"env.{flag} != 'true'" in step_if(s)]
        self.assertEqual(len(notices), 1)
        self.assertIn("::notice", notices[0])


if __name__ == "__main__":
    unittest.main()
