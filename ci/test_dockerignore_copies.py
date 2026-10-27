#!/usr/bin/env -S uv run --script
"""Every file a root-context Dockerfile COPYs is in the build context.

.dockerignore excludes everything (`*`) and lists the files the images built
from the repo root read. A COPY of an unlisted file fails only when the image
is built (`bosn run --task appimage`: "install_zccache.py: not found").
"""

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parent.parent
# The Dockerfiles bosn.toml builds with the repo root as their context.
ROOT_CONTEXT_DOCKERFILES = (
    "build-aux/appimage/manylinux/Dockerfile",
)


def allowed() -> set[str]:
    lines = (ROOT / ".dockerignore").read_text().splitlines()
    return {line[1:].strip() for line in lines if line.startswith("!")}


def copied_sources(dockerfile: str) -> list[str]:
    sources = []
    for line in (ROOT / dockerfile).read_text().splitlines():
        words = line.split()
        if not words or words[0].upper() not in ("COPY", "ADD"):
            continue
        args = [w for w in words[1:] if not w.startswith("--")]
        if any(w.startswith("--from") for w in words[1:]):
            continue  # from another stage, not the context
        sources.extend(args[:-1])
    return sources


class DockerignoreCopiesTest(unittest.TestCase):
    def test_every_copied_file_is_in_the_build_context(self):
        listed = allowed()
        for dockerfile in ROOT_CONTEXT_DOCKERFILES:
            for source in copied_sources(dockerfile):
                with self.subTest(dockerfile=dockerfile, source=source):
                    self.assertIn(source, listed, f"add !{source} to .dockerignore")

    def test_the_dockerfiles_copy_something(self):
        for dockerfile in ROOT_CONTEXT_DOCKERFILES:
            with self.subTest(dockerfile=dockerfile):
                self.assertTrue(copied_sources(dockerfile))


if __name__ == "__main__":
    unittest.main()
