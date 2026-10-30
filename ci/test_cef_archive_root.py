#!/usr/bin/env -S uv run --script
"""The dependency archives are not all flat (#2418).

The CEF 8078 macOS builds keep `cef_binary_<version>_macos_<arch>/` at the
archive root, while obs-deps, Qt6 and the older CEF 7871 archives put their
files at the root. `cmake/common/buildspec_common.cmake` extracted straight
into `.deps/<destination>`, so on macOS `CEF_ROOT_DIR/include/cef_version.h`
did not exist and configure failed with `Could NOT find CEF ... found
CEF_LIBRARY_RELEASE-NOTFOUND` (Scheduled run 41010694789), after the download
and its SHA-256 check had succeeded.

`_obs_extract_dependency` lands both layouts in `.deps/<destination>`: it
extracts into a staging directory and moves a lone top-level directory's
contents up.

    uv run python -m unittest ci.test_cef_archive_root
"""

from __future__ import annotations

import io
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BUILDSPEC = ROOT / "cmake/common/buildspec_common.cmake"
DESTINATION = "cef_binary_8078_macos_arm64"


def write_archive(path: Path, members: dict[str, str]) -> None:
    """Write an xz-compressed tar with `members` as regular files."""
    with tarfile.open(path, "w:xz") as archive:
        for name, text in members.items():
            data = text.encode()
            info = tarfile.TarInfo(name)
            info.size = len(data)
            archive.addfile(info, io.BytesIO(data))


def extract(archive: Path, dependencies_dir: Path) -> subprocess.CompletedProcess[str]:
    script = (
        f'include("{BUILDSPEC}")\n'
        f'_obs_extract_dependency("{archive}" "{dependencies_dir}" "{DESTINATION}")\n'
    )
    script_path = dependencies_dir.parent / "extract.cmake"
    script_path.write_text(script)
    return subprocess.run(
        ["cmake", "-P", str(script_path)],
        capture_output=True,
        text=True,
        check=False,
    )


class CefArchiveRootTest(unittest.TestCase):
    def setUp(self) -> None:
        if shutil.which("cmake") is None:
            self.skipTest("cmake is not installed")
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.dependencies = self.root / ".deps"
        self.dependencies.mkdir()

    def assert_landed(self, dependencies_dir: Path) -> None:
        destination = dependencies_dir / DESTINATION
        self.assertTrue((destination / "include/cef_version.h").is_file(), "no cef_version.h")
        self.assertTrue((destination / "Release/Chromium Embedded Framework.framework").is_dir())
        self.assertTrue((destination / "build/libcef_dll_wrapper/libcef_dll_wrapper.a").is_file())
        self.assertTrue((destination / ".bazelversion").is_file(), "dotfile dropped")

    def test_nested_archive_is_flattened(self) -> None:
        archive = self.root / "nested.tar.xz"
        write_archive(
            archive,
            {
                f"{DESTINATION}/include/cef_version.h": "#define CEF_VERSION_MAJOR 8078\n",
                f"{DESTINATION}/Release/Chromium Embedded Framework.framework/Info.plist": "",
                f"{DESTINATION}/build/libcef_dll_wrapper/libcef_dll_wrapper.a": "",
                f"{DESTINATION}/.bazelversion": "1.0\n",
            },
        )
        result = extract(archive, self.dependencies)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_landed(self.dependencies)

    def test_flat_archive_is_kept(self) -> None:
        archive = self.root / "flat.tar.xz"
        write_archive(
            archive,
            {
                "include/cef_version.h": "#define CEF_VERSION_MAJOR 7871\n",
                "Release/Chromium Embedded Framework.framework/Info.plist": "",
                "build/libcef_dll_wrapper/libcef_dll_wrapper.a": "",
                ".bazelversion": "1.0\n",
            },
        )
        result = extract(archive, self.dependencies)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_landed(self.dependencies)

    def test_stale_destination_is_replaced(self) -> None:
        # A cached 7871 tree must not sit under the 8078 destination name.
        stale = self.dependencies / DESTINATION / "include"
        stale.mkdir(parents=True)
        (stale / "cef_version.h").write_text("#define CEF_VERSION_MAJOR 7871\n")
        archive = self.root / "nested.tar.xz"
        write_archive(
            archive,
            {
                f"{DESTINATION}/include/cef_version.h": "#define CEF_VERSION_MAJOR 8078\n",
                f"{DESTINATION}/Release/Chromium Embedded Framework.framework/Info.plist": "",
                f"{DESTINATION}/build/libcef_dll_wrapper/libcef_dll_wrapper.a": "",
                f"{DESTINATION}/.bazelversion": "1.0\n",
            },
        )
        result = extract(archive, self.dependencies)
        self.assertEqual(result.returncode, 0, result.stderr)
        version = (self.dependencies / DESTINATION / "include/cef_version.h").read_text()
        self.assertIn("8078", version)


if __name__ == "__main__":
    unittest.main()
