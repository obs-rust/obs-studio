#!/usr/bin/env -S uv run --script
"""Build rnnoise's training-data generator, `denoise_training` (#962).

    uv run plugins/obs-filters/rnnoise/src/compile.py

Ported off the 3-line shell script `compile.sh`. The upstream training flow
already lives beside it as Python (`rnn_train.py`), so this build step is
Python too. The script has no CI caller -- it is the developer helper that
regenerates the training data generator -- so nothing else in the tree changes
when it moves.

The `gcc` line is kept token for token: the same flags, the same six source
files, `-o denoise_training` and `-lm`, run with the working directory set to
this directory so `-I../include` and the relative sources resolve as before.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent

# The shell's single gcc invocation, kept verbatim.
GCC = "gcc"
FLAGS = ("-DTRAINING=1", "-Wall", "-W", "-O3", "-g", "-I../include")
SOURCES = ("denoise.c", "kiss_fft.c", "pitch.c", "celt_lpc.c", "rnn.c", "rnn_data.c")
OUTPUT = "denoise_training"
LIBS = ("-lm",)


def argv() -> list[str]:
    """The gcc command line, exactly as the shell script wrote it."""
    return [GCC, *FLAGS, *SOURCES, "-o", OUTPUT, *LIBS]


def main() -> int:
    """Run gcc from this directory and return its exit status."""
    return subprocess.run(argv(), cwd=HERE, check=False).returncode


if __name__ == "__main__":
    sys.exit(main())
