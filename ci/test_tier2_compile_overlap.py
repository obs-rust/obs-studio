"""Verify independent Tier 2 execution while Fast Gate remains required."""
import ast
from pathlib import Path
import re
import unittest

from tier2_matrix import matrix


ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/build-project.yaml"


def job(name):
    match = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)",
                      WORKFLOW.read_text(), re.M | re.S)
    return match.group(1)


def dependencies(name):
    match = re.search(r"^    needs: \[([^]]+)\]", job(name), re.M)
    return [value.strip() for value in match.group(1).split(",")] if match else []


def tests_allowed(results, cancelled=False):
    match = re.search(r"^    if: \$\{\{ (.*?) \}\}", job("rust-linux-tier2"), re.M)
    if not match:
        return all(results[name] == "success" for name in dependencies("rust-linux-tier2"))
    # This condition uses only boolean operators and literal result equality.
    # Fail closed on any future syntax outside that deliberately small subset.
    expression = re.sub(r"needs\.([\w-]+)\.result",
                        lambda m: repr(results[m[1]]), match[1])
    expression = expression.replace("always()", "True").replace("cancelled()", str(cancelled))
    expression = expression.replace("&&", " and ").replace("||", " or ")
    expression = re.sub(r"!(?!=)", "not ", expression).strip()
    tree = ast.parse(expression, mode="eval")
    allowed = (ast.Expression, ast.BoolOp, ast.And, ast.Or, ast.UnaryOp,
               ast.Not, ast.Compare, ast.Eq, ast.NotEq, ast.Constant)
    if any(not isinstance(node, allowed) for node in ast.walk(tree)):
        raise AssertionError("Unexpected syntax in Tier 2 test eligibility")
    return eval(compile(tree, "<tier2-if>", "eval"), {"__builtins__": {}})


class CompileOverlapTest(unittest.TestCase):
    def test_build_has_no_fast_gate_ancestor(self):
        def ancestors(name):
            direct = dependencies(name)
            return set(direct).union(*(ancestors(parent) for parent in direct))
        self.assertNotIn("rust-tests-linux", ancestors("rust-linux-tier2-build"))
        self.assertIn("check-event", dependencies("rust-linux-tier2-build"))
        self.assertIn("ci-mode", dependencies("rust-linux-tier2-build"))

    def test_tests_wait_for_build_without_fast_gate_ancestor(self):
        self.assertNotIn("rust-tests-linux", dependencies("rust-linux-tier2"))
        self.assertIn("rust-linux-tier2-build", dependencies("rust-linux-tier2"))

    def test_failure_cancellation_and_attestation_matrix(self):
        for gate in ("success", "skipped", "failure", "cancelled"):
            for build in ("success", "skipped", "failure", "cancelled"):
                for cancelled in (False, True):
                    with self.subTest(gate=gate, build=build, cancelled=cancelled):
                        results = {"rust-tests-linux": gate, "rust-linux-tier2-build": build,
                                   "check-event": "success", "ci-mode": "success"}
                        expected = not cancelled and build == "success"
                        self.assertEqual(tests_allowed(results, cancelled), expected)

    def test_final_gate_rejects_failed_or_cancelled_independent_workflow(self):
        gate = (ROOT / ".github/workflows/ci.yml").read_text().split("  ci-gate:", 1)[1]
        condition = re.search(r"if: (contains\(needs\.\*\.result[^\n]+)", gate)[1]
        for result in ("success", "skipped", "failure", "cancelled"):
            with self.subTest(result=result):
                values = ["success", result]  # Successful suites cannot mask Fast Gate.
                expression = re.sub(r"contains\(needs\.\*\.result, '([^']+)'\)",
                                    lambda m: repr(m[1] in values), condition).replace("||", " or ")
                self.assertEqual(eval(expression, {"__builtins__": {}}),
                                 result in {"failure", "cancelled"})
        self.assertNotIn("continue-on-error:", job("rust-tests-linux"))

    def test_both_implementations_and_final_gate_remain(self):
        tests = job("rust-linux-tier2")
        self.assertIn("fromJSON(needs.check-event.outputs.tier2_matrix)", tests)
        for event in ("merge_group", "pull_request", "push", "schedule"):
            self.assertEqual(matrix(event)["mode"], ["OFF", "ON"])
        self.assertIn("fail-fast: false", tests)
        gate = (ROOT / ".github/workflows/ci.yml").read_text().split("  ci-gate:", 1)[1]
        self.assertIn("build-project", gate.split("steps:", 1)[0])
        self.assertIn("contains(needs.*.result, 'failure')", gate)


if __name__ == "__main__":
    unittest.main()
