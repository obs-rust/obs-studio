#!/usr/bin/env -S uv run --script
"""Keep AI rebase recovery on its deliberate schedule/manual feedback boundary."""

from pathlib import Path
import re
import unittest


WORKFLOW = Path(__file__).resolve().parent.parent / ".github/workflows/ai-rebase.yaml"


def trigger_keys(text: str) -> set[str]:
    """Parse the block YAML event mapping, rejecting unsupported/ambiguous shapes.

    Like the existing workflow shape checks, use only the standard library so
    this runs in the dependency-free ci self-test lane. Nested event options
    are irrelevant; direct mapping keys define the workflow's triggers.
    """
    lines = [line for line in text.splitlines()
             if line.strip() and not line.lstrip().startswith("#")]
    starts = [i for i, line in enumerate(lines)
              if re.fullmatch(r'''(?:on|'on'|"on"):\s*(?:#.*)?''', line)]
    if len(starts) != 1:
        raise ValueError("expected one block YAML on mapping")
    keys = set()
    indent = None
    for line in lines[starts[0] + 1:]:
        depth = len(line) - len(line.lstrip(" "))
        if depth == 0:
            break
        if indent is None:
            indent = depth
        if depth < indent:
            raise ValueError("inconsistent event mapping indentation")
        if depth == indent:
            match = re.fullmatch(r'''\s+([\w-]+|'[^']+'|"[^"]+"):\s*(?:#.*)?''', line)
            if not match:
                raise ValueError("expected a block YAML event key")
            key = match[1].strip("'\"")
            if key in keys:
                raise ValueError("duplicate event key")
            keys.add(key)
    return keys


class WorkflowShapeTest(unittest.TestCase):
    def test_only_schedule_and_manual_triggers(self):
        self.assertEqual(trigger_keys(WORKFLOW.read_text()), {"schedule", "workflow_dispatch"})

    def test_no_completion_event_references_anywhere(self):
        self.assertNotIn("workflow_run", WORKFLOW.read_text())

    def test_header_documents_scheduled_recovery_latency(self):
        header = WORKFLOW.read_text().split("\non:", 1)[0]
        self.assertIn(
            "# Infrastructure-ejected PRs are re-armed on the schedule (up to 30 minutes later).",
            header,
        )

    def test_scheduled_requeue_backstop_stays(self):
        self.assertIn("ci/ai_rebase.py --requeue-ejected --limit 10", WORKFLOW.read_text())


if __name__ == "__main__":
    unittest.main()
