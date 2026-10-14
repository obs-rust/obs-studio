#!/usr/bin/env -S uv run --no-project --script
"""PreToolUse hook: deny bare Rust tools and bare python/pip (obs-rust/obs-studio#691).

Rust tools (cargo, rustup, rustc, ...) go through `soldr`, which pins the
toolchain and routes compiles through its zccache; Python goes through uv
(AGENTS.md "Quick rules"). Ported from soldr's .claude/hooks/tool_guard.py.

The command is parsed, not substring-matched: a tool name inside a quoted
argument, a heredoc body or an `echo` is allowed. These still count as running
the tool: leading `FOO=bar` assignments, `env`/`exec`/`time`/`nice`/`sudo`/
`command` prefixes, `bash -c`/`sh -c` strings, and every line, `;`, `&&`,
`||` and `|` segment. `docker`/`ssh` arguments run elsewhere and are allowed.

clud sessions (CLUD_SESSION_ID set) are skipped: clud enforces the same rule
and also puts soldr's shims on PATH, so the hook would only repeat its denial.

Reads the hook event JSON on stdin; prints a deny decision or nothing. Exit 0.
Usage: uv run --no-project --script ci/hooks/tool_guard.py < event.json
"""

import json
import os
import re
import shlex
import sys

RUST_TOOLS = {
    "cargo",
    "cargo-clippy",
    "cargo-fmt",
    "clippy-driver",
    "rustc",
    "rustdoc",
    "rustfmt",
    "rustup",
    "rust-gdb",
    "rust-lldb",
    "rust-analyzer",
}
PYTHON_TOOLS = set("python python3 pip pip3".split())
SHELL_TOOL_NAMES = {"Bash", "Shell", "PowerShell", "shell_command", "functions.shell_command"}

SEGMENT_OPERATORS = {";", ";;", "&", "&&", "||", "|", "|&", "\n"}
# Prefixes that run their argument as the command. Their own options are
# skipped up to the first word that is not an option or an assignment.
WRAPPERS = {"env", "exec", "time", "nice", "sudo", "command", "nohup", "xargs"}
# Wrapper options whose value is the next word.
WRAPPER_VALUE_OPTIONS = {
    "env": {"-u", "--unset", "-C", "--chdir", "-S", "--split-string"},
    "exec": {"-a"},
    "time": {"-f", "--format", "-o", "--output"},
    "nice": {"-n", "--adjustment"},
    "sudo": {"-u", "--user", "-g", "--group", "-C", "-D", "-h", "-p", "-r", "-t", "-U"},
    "xargs": {"-a", "-d", "-E", "-I", "-L", "-n", "-P", "-s", "--arg-file", "--delimiter", "--max-args"},
}
SHELLS = {"bash", "sh", "zsh", "dash"}
ENV_ASSIGN_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")
HEREDOC_RE = re.compile(r"<<-?\s*(['\"]?)([A-Za-z_][A-Za-z0-9_]*)\1")
MAX_DEPTH = 4


def strip_heredocs(command):
    """Drop heredoc bodies: their lines are data, not commands."""
    out = []
    lines = command.split("\n")
    i = 0
    while i < len(lines):
        line = lines[i]
        out.append(line)
        i += 1
        for match in HEREDOC_RE.finditer(line):
            end = match.group(2)
            while i < len(lines) and lines[i].strip() != end:
                i += 1
            i += 1  # the terminator
    return "\n".join(out)


def tokenize_segments(command):
    """Yield the token list of each top-level command in `command`."""
    try:
        lex = shlex.shlex(strip_heredocs(command), posix=True, punctuation_chars=";&|\n")
        lex.whitespace = " \t\r"
        lex.whitespace_split = True
        tokens = list(lex)
    except ValueError:
        # Unbalanced quotes: inspect rough segments rather than allow all.
        tokens = []
        for raw in re.split(r"&&|\|\||[;|\n]", command):
            tokens.extend(raw.split())
            tokens.append(";")
    current = []
    for token in tokens:
        if token in SEGMENT_OPERATORS or set(token) <= set(";&|\n"):
            if current:
                yield current
            current = []
        else:
            current.append(token)
    if current:
        yield current


def command_words(tokens):
    """Strip assignments and wrapper prefixes; return the words that run."""
    while tokens:
        if ENV_ASSIGN_RE.match(tokens[0]):
            tokens = tokens[1:]
        elif os.path.basename(tokens[0]) in WRAPPERS:
            takes_value = WRAPPER_VALUE_OPTIONS.get(os.path.basename(tokens[0]), set())
            tokens = tokens[1:]
            while tokens and (tokens[0].startswith("-") or ENV_ASSIGN_RE.match(tokens[0])):
                tokens = tokens[2:] if tokens[0] in takes_value else tokens[1:]
        else:
            break
    return tokens


def shell_c_script(tokens):
    """The script of `bash -c '<script>'`, or None."""
    if os.path.basename(tokens[0]) not in SHELLS:
        return None
    for i, token in enumerate(tokens[1:], 1):
        if token.startswith("-") and not token.startswith("--") and "c" in token:
            return tokens[i + 1] if i + 1 < len(tokens) else None
        if not token.startswith("-"):
            return None
    return None


def check_command(command, depth=0):
    """Return (tool, reason) for the first forbidden invocation, else None."""
    for segment in tokenize_segments(command):
        tokens = command_words(segment)
        if not tokens:
            continue
        script = shell_c_script(tokens)
        if script is not None:
            if depth < MAX_DEPTH:
                found = check_command(script, depth + 1)
                if found:
                    return found
            continue
        head = os.path.basename(tokens[0])
        if head == "soldr" or tokens[:2] == ["uv", "pip"]:
            continue
        if tokens[:2] == ["uv", "run"]:
            target = uv_run_target(tokens)
            if target in RUST_TOOLS:
                return target, f"Use `soldr {target} ...` instead of `uv run {target} ...`."
            continue
        if head in RUST_TOOLS:
            return head, (
                f"Use `soldr {head} ...` instead of bare `{head}` (AGENTS.md). soldr pins the "
                f"toolchain and routes compiles through its cache; bare Rust tools bypass both."
            )
        if head in PYTHON_TOOLS:
            if head.startswith("pip"):
                return head, f"Use `uv pip ...` instead of bare `{head}` (AGENTS.md)."
            return head, f"Use `uv run ...` instead of bare `{head}` (AGENTS.md)."
    return None


UV_RUN_FLAGS_WITH_VALUES = {
    "--directory",
    "--env-file",
    "--extra",
    "--from",
    "--group",
    "--index-url",
    "--project",
    "--python",
    "--with",
    "--with-editable",
    "--with-requirements",
    "-p",
}


def uv_run_target(tokens):
    """The command `uv run` runs, after its own options."""
    i = 2
    while i < len(tokens):
        token = tokens[i]
        if token in UV_RUN_FLAGS_WITH_VALUES:
            i += 2
        elif token.startswith("-"):
            i += 1
        else:
            return token
    return ""


def extract_command(data):
    tool_input = data.get("tool_input", {})
    if not isinstance(tool_input, dict):
        return ""
    for key in ("command", "script", "cmd"):
        value = tool_input.get(key)
        if isinstance(value, str) and value.strip():
            return value
    return ""


def main():
    if os.environ.get("CLUD_SESSION_ID"):
        return
    try:
        data = json.loads(sys.stdin.read().lstrip("﻿"))
    except json.JSONDecodeError:
        return
    if not isinstance(data, dict) or data.get("tool_name", "") not in SHELL_TOOL_NAMES:
        return
    result = check_command(extract_command(data))
    if result:
        json.dump(
            {
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": result[1],
                }
            },
            sys.stdout,
        )


if __name__ == "__main__":
    main()
