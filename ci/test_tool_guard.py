"""Tests for ci/hooks/tool_guard.py and the bare-Rust-tool scan (obs-rust/obs-studio#691).

Run: uv run python -m unittest ci/test_tool_guard.py
"""

import importlib.util
import json
import os
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HOOK = ROOT / "ci" / "hooks" / "tool_guard.py"

spec = importlib.util.spec_from_file_location("tool_guard", HOOK)
tool_guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tool_guard)

# Scripts that run inside a container or CI image without soldr, where the
# image's own rustup toolchain is the only one. Each is reviewed by hand.
SCAN_ALLOWLIST = {
    "rust/tools/linux-validate/validate.sh",  # in the linux-validate image
    "rust/tools/sanitize/sanitize.sh",  # in the sanitizer image
}


def denied(command):
    return tool_guard.check_command(command)


class Denied(unittest.TestCase):
    def test_rust_tools(self):
        for command in [
            "cargo test",
            "cargo build --release",
            "rustup toolchain install",
            "rustc -V",
            "rustfmt src/lib.rs",
            "cargo-clippy",
            "/usr/bin/cargo build",
            "uv run cargo build",
        ]:
            with self.subTest(command=command):
                self.assertIsNotNone(denied(command))

    def test_hint_names_soldr(self):
        tool, reason = denied("cargo test")
        self.assertEqual(tool, "cargo")
        self.assertIn("soldr cargo", reason)

    def test_prefixes_do_not_bypass(self):
        for command in [
            "FOO=1 cargo build",
            "env cargo build",
            "env -u X FOO=1 cargo build",
            "exec cargo build",
            "time cargo build",
            "sudo rustup update",
            "command cargo build",
        ]:
            with self.subTest(command=command):
                self.assertIsNotNone(denied(command))

    def test_shell_c_strings(self):
        for command in [
            'bash -c "cargo build"',
            "sh -c 'cd rust && cargo test'",
            "bash -lc 'cargo build'",
            "bash -c \"bash -c 'cargo build'\"",
        ]:
            with self.subTest(command=command):
                self.assertIsNotNone(denied(command))

    def test_every_segment(self):
        for command in [
            "cd rust; cargo build",
            "true && cargo build",
            "false || cargo build",
            "echo hi | cargo build",
            "cd rust\ncargo build",
        ]:
            with self.subTest(command=command):
                self.assertIsNotNone(denied(command))

    def test_python(self):
        for command in ["python x.py", "python3 -m unittest", "pip install x", "FOO=1 python3 x.py"]:
            with self.subTest(command=command):
                self.assertIsNotNone(denied(command))


class Allowed(unittest.TestCase):
    def test_allowed(self):
        for command in [
            "soldr cargo test",
            "FOO=1 soldr cargo build",
            "uv run --no-project soldr cargo test",
            "uv run python -m unittest ci/test_tool_guard.py",
            "uv pip install x",
            'gh issue create --body "use soldr instead of bare rustup"',
            "gh pr comment 1 --body 'cargo build; rustup update'",
            "echo cargo:warning=foo",
            "git commit -m 'cargo test now passes'",
            "grep -rn cargo rust/",
            "docker run img cargo build",
            "ssh host cargo build",
            "ls",
            "",
        ]:
            with self.subTest(command=command):
                self.assertIsNone(denied(command))

    def test_heredoc_bodies(self):
        for command in [
            "cat <<EOF > notes.md\ncargo build\nrustup update\nEOF",
            "gh pr create --body-file - <<'EOF'\nRun cargo test.\npython x.py\nEOF\necho done",
            "cat <<-END\n\tcargo build\n\tEND",
        ]:
            with self.subTest(command=command):
                self.assertIsNone(denied(command))

    def test_command_after_heredoc_is_checked(self):
        self.assertIsNotNone(denied("cat <<EOF\ntext\nEOF\ncargo build"))


class HookProcess(unittest.TestCase):
    def run_hook(self, event, clud=False):
        env = {k: v for k, v in os.environ.items() if k != "CLUD_SESSION_ID"}
        if clud:
            env["CLUD_SESSION_ID"] = "test"
        return subprocess.run(
            [sys.executable, str(HOOK)], input=json.dumps(event), capture_output=True, text=True, env=env, check=True
        ).stdout

    def test_denies_bash_event(self):
        out = json.loads(self.run_hook({"tool_name": "Bash", "tool_input": {"command": "cargo test"}}))
        self.assertEqual(out["hookSpecificOutput"]["permissionDecision"], "deny")
        self.assertIn("soldr cargo", out["hookSpecificOutput"]["permissionDecisionReason"])

    def test_allows_soldr(self):
        self.assertEqual(self.run_hook({"tool_name": "Bash", "tool_input": {"command": "soldr cargo test"}}), "")

    def test_ignores_other_tools(self):
        self.assertEqual(self.run_hook({"tool_name": "Read", "tool_input": {"command": "cargo test"}}), "")

    def test_silent_under_clud(self):
        event = {"tool_name": "Bash", "tool_input": {"command": "cargo test"}}
        self.assertEqual(self.run_hook(event, clud=True), "")


class ProjectSettings(unittest.TestCase):
    def test_hook_is_wired(self):
        settings = json.loads((ROOT / ".claude" / "settings.json").read_text())
        commands = [
            hook["command"]
            for entry in settings["hooks"]["PreToolUse"]
            if "Bash" in entry.get("matcher", "")
            for hook in entry["hooks"]
        ]
        self.assertTrue(any("ci/hooks/tool_guard.py" in c and "[ -f ci/hooks/tool_guard.py ]" in c for c in commands))

    def test_clud_uses_soldr(self):
        settings = json.loads((ROOT / ".clud" / "settings.json").read_text())
        self.assertTrue(settings["rust"]["use_soldr"])


def shell_lines(path):
    """Command lines of a shell script, without comments or heredoc bodies."""
    text = tool_guard.strip_heredocs(path.read_text(errors="replace"))
    for number, line in enumerate(text.split("\n"), 1):
        stripped = line.strip()
        if stripped and not stripped.startswith("#"):
            yield number, stripped


def is_shell(path):
    if path.suffix == ".sh":
        return True
    try:
        first = path.open("rb").readline()
    except OSError:
        return False
    return first.startswith(b"#!") and any(s in first for s in (b"/sh", b"bash", b"zsh"))


class ScriptsUseSoldr(unittest.TestCase):
    def test_no_bare_rust_tools_in_scripts(self):
        files = subprocess.run(
            ["git", "ls-files", "ci", "rust/tools"], cwd=ROOT, capture_output=True, text=True, check=True
        ).stdout.split()
        found = []
        for name in files:
            path = ROOT / name
            if name in SCAN_ALLOWLIST or not path.is_file() or not is_shell(path):
                continue
            for number, line in shell_lines(path):
                result = denied(line)
                if result and result[0] in tool_guard.RUST_TOOLS:
                    found.append(f"{name}:{number}: {line}")
        self.assertEqual(found, [], "use `soldr <tool>` (or add a reviewed SCAN_ALLOWLIST entry)")

    def test_scan_catches_bare_cargo(self):
        self.assertIsNotNone(denied("  cargo build --workspace"))


if __name__ == "__main__":
    unittest.main()
