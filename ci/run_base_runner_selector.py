#!/usr/bin/env -S uv run --script
"""Launch runner selection from immutable BASE objects, without head imports.

The hosted workflow checks out BASE before invoking this launcher. Keep the
selector and membership verifier together in an isolated module directory.
"""

import os
from pathlib import Path
import re
import subprocess
import tempfile


def main() -> int:
    base = os.environ["BASE"]
    if re.fullmatch(r"[0-9a-f]{40}", base) is None:
        raise ValueError("invalid immutable base SHA")
    with tempfile.TemporaryDirectory(prefix="dell-selector-", dir=os.environ.get("RUNNER_TEMP")) as directory:
        trusted = Path(directory)
        for script in ("select_mq_runner.py", "pr_attestation.py"):
            source = subprocess.run(
                ["git", "show", f"{base}:ci/{script}"],
                capture_output=True, check=True,
            ).stdout
            (trusted / script).write_bytes(source)
        return subprocess.run(
            ["uv", "run", "--no-project", str(trusted / "select_mq_runner.py")],
            check=False,
        ).returncode


if __name__ == "__main__":
    raise SystemExit(main())
