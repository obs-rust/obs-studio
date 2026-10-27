#!/usr/bin/env -S uv run --script
"""Delete the objects ninja recorded with no header dependencies (#2244).

    uv run --script rust/tools/linux-validate/drop_depless_objects.py <build-dir>

Every C/C++ object includes at least one header, so "#deps 0" means ninja read
an empty depfile. Such an object is never rebuilt after a header edit, and
Tier 2 would test old code. No build tree yet (or no ninja): nothing to do.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

# A depfile-less object: no leading space, and the reason field follows.
_DEPLESS = re.compile(r"^(\S[^:\n]*): #deps 0,", re.MULTILINE)


def depless_objects(build: Path) -> list[Path]:
    """The objects `ninja -t deps` lists with zero recorded dependencies."""
    try:
        deps = subprocess.run(
            ["ninja", "-C", str(build), "-t", "deps"],
            check=True, capture_output=True, text=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError):
        return []
    return [build / name for name in _DEPLESS.findall(deps)]


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    removed = 0
    for obj in depless_objects(Path(argv[1])):
        try:
            obj.unlink()
        except FileNotFoundError:
            pass  # Already gone; ninja will still rebuild it.
        except OSError as error:
            print(f"could not remove {obj}: {error}", file=sys.stderr)
            return 1
        removed += 1
    if removed:
        print(f"== {removed} objects had no recorded header dependencies; removed so they are compiled again (#2244)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
