#!/usr/bin/env -S uv run --script
"""Host-only apt mirror preference; no package or repository changes.

    sudo -E uv run --no-project ci/host_apt_mirrors.py [mirror-list]

The mirror list defaults to /etc/apt/apt-mirrors.txt. The step runs as root
(the workflows call it through `sudo`), so the script itself stays small and
rewrites the file only when it recognizes the runner image's layout.

It recognizes only the runner-images configure-apt-sources.sh layout: the
Azure archive at priority 1 backed by the HTTPS Ubuntu archive at priority 2,
one entry each. Additional metadata, duplicate known URIs and every other
image layout keep their policy, as does a missing or symlinked list.

Lower priority numbers are tried first (obs-rust/obs-studio#962).
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

DEFAULT_MIRRORS = Path("/etc/apt/apt-mirrors.txt")
AZURE = r"http://azure\.archive\.ubuntu\.com/ubuntu/"
ARCHIVE = r"https://archive\.ubuntu\.com/ubuntu/"
BLANK = r"[ \t]"  # Python's spelling of the shell's [[:blank:]]
SED_BLANK = "[[:blank:]]"

# grep's `^<url>([[:blank:]]|$)`: the line is the URI alone or the URI plus
# fields -- `priority:1` alone, not `priority:10` or `priority:1 arch:amd64`.
_URI_LINE = r"({blank}|$)"
# ... and the exact priority assignment the swap rewrites.
_PRIORITY = r"{blank}+priority:{number}$"


def lines_of(text: str) -> list[str]:
    """The file's lines, the way grep counts them (a last line without \\n counts)."""
    return text.splitlines()


def matching(lines: list[str], pattern: str) -> list[str]:
    """The lines matching an anchored pattern."""
    compiled = re.compile(pattern)
    return [line for line in lines if compiled.match(line)]


def recognized(lines: list[str]) -> bool:
    """Only the runner-images layout: one Azure URI and one archive URI, at 1 and 2."""
    azure_uri = AZURE + _URI_LINE.format(blank=BLANK)
    archive_uri = ARCHIVE + _URI_LINE.format(blank=BLANK)
    azure_priority = AZURE + _PRIORITY.format(blank=BLANK, number=1)
    archive_priority = ARCHIVE + _PRIORITY.format(blank=BLANK, number=2)
    return (
        len(matching(lines, azure_uri)) == 1
        and len(matching(lines, archive_uri)) == 1
        and bool(matching(lines, azure_priority))
        and bool(matching(lines, archive_priority))
    )


def promote_archive(path: Path) -> int:
    """Swap the Azure and archive priorities in place; sed's exit code, unchanged.

    Lower priority numbers are tried first, so the swap keeps the Azure and
    security fallback routes and promotes the HTTPS archive this image already
    trusts. sed edits in place, so the file's mode is preserved and no
    temporary file is left in the directory.
    """
    azure = f"s@^({AZURE}{SED_BLANK}+)priority:1$@\\1priority:2@"
    archive = f"s@^({ARCHIVE}{SED_BLANK}+)priority:2$@\\1priority:1@"
    result = subprocess.run(
        ["sed", "-i", "-E", "-e", azure, "-e", archive, "--", str(path)], check=False
    )
    return result.returncode


def configure(mirrors: Path) -> int:
    if not mirrors.is_file() or mirrors.is_symlink():
        print("host apt: keeping missing or linked mirror configuration")
        return 0
    lines = lines_of(mirrors.read_text(encoding="utf-8", errors="surrogateescape"))
    if not recognized(lines):
        print("host apt: keeping existing or unrecognized mirror priorities")
        return 0
    code = promote_archive(mirrors)
    if code != 0:
        # set -e parity: a failed rewrite is the script's exit code, never a
        # success message over a list that was not updated.
        return code
    print("host apt: preferring existing HTTPS Ubuntu archive; fallbacks retained")
    return 0


def main(argv: list[str]) -> int:
    if len(argv) > 2:
        print("usage: host_apt_mirrors.py [mirror-list]", file=sys.stderr)
        return 2
    mirrors = Path(argv[1]) if len(argv) == 2 else DEFAULT_MIRRORS
    return configure(mirrors)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
