#!/usr/bin/env -S uv run --script
"""The host-library loader test runs on stubs (#962).

build-aux/appimage/hostlib/test-hostlib.sh builds a probe against the libva
shims and checks what the loader prints for each candidate. The port to
Python must keep every check, so run it with a stub toolchain, a stub
patchelf/ldconfig and a fake probe that answers the way hostlib.c does.

    uv run --no-project ci/test_hostlib_harness.py
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "build-aux/appimage/hostlib/test_hostlib.py"

# The real messages, from hostlib.c: "does not load: <reason>", "is inside
# the AppImage (<real>)", "lacks <symbol> (too old?)", "hostlib: va -> <path>"
# and "hostlib: va unavailable (<reason>): searched N candidates".
PROBE = """#!/bin/sh
va=${OBS_HOSTLIB_VA:-}
if [ -n "$va" ]; then
  case "$va" in
    *nonexistent*)
      echo 'hostlib: va unavailable (not found): searched 1 candidates'
      echo 'display=null status=0x5 str=(null)'
      exit 0 ;;
  esac
  if [ "$OBS_HOSTLIB_DEBUG" = 1 ]; then
    case "$va" in
      */AppDir/usr/lib/*) echo "$va: is inside the AppImage (/usr/lib/libva.so.2)"; exit 0 ;;
      *libva-old*) echo "$va: lacks vaInitialize (too old?)"; exit 0 ;;
    esac
    echo "$va: does not load: wrong ELF class"
    exit 0
  fi
fi
echo 'hostlib: va -> /usr/lib/x86_64-linux-gnu/libva.so.2'
echo 'display=null status=0x0 str=the requested function is not implemented'
"""

CC = """#!/bin/sh
out=
previous=
for argument in "$@"; do
  if [ "$previous" = "-o" ]; then out=$argument; fi
  previous=$argument
done
case " $* " in
  *" -shared "*) : >"$out" ;;
  *) cp "$STUB_PROBE" "$out"; chmod +x "$out" ;;
esac
"""

PATCHELF = """#!/bin/sh
printf '%s\\n' libobs-va.so.2 libobs-va-drm.so.2 libc.so.6 libva-drm.so.2
"""

LDCONFIG = """#!/bin/sh
printf '\\tlibva.so.2 (libc6,x86-64) => %s\\n' "$STUB_LIBVA_REAL"
"""

UV = """#!/bin/sh
for argument in "$@"; do app=$argument; done
mkdir -p "$app/usr/bin"
cp "$STUB_PREFLIGHT" "$app/usr/bin/obs-hostlib-check"
chmod +x "$app/usr/bin/obs-hostlib-check"
echo 'hostlib: probe links libobs-va.so.2'
"""

PREFLIGHT = """#!/bin/sh
if [ "${OBS_HOSTLIB_EGL:-}" = /nonexistent/libEGL.so.1 ]; then
  echo 'obs-hostlib-check: cannot load libEGL.so.1' >&2
  exit 127
fi
echo 'obs-hostlib-check: 1 required libraries load'
"""


class HostlibHarnessTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        real = self.root / "libva.so.2"
        real.write_bytes(b"\x7fELF host libva")
        self.stub("cc", CC)
        self.stub("patchelf", PATCHELF)
        self.stub("ldconfig", LDCONFIG)
        self.stub("uv", UV)
        self.probe = self.root / "probe.sh"
        self.probe.write_text(PROBE)
        self.probe.chmod(0o755)
        preflight = self.root / "preflight.sh"
        preflight.write_text(PREFLIGHT)
        preflight.chmod(0o755)
        self.env = dict(
            os.environ,
            PATH=f"{self.bin}{os.pathsep}{os.environ['PATH']}",
            STUB_PROBE=str(self.probe),
            STUB_PREFLIGHT=str(preflight),
            STUB_LIBVA_REAL=str(real),
        )
        for name in ("OBS_HOSTLIB_VA", "OBS_HOSTLIB_VA_DRM", "OBS_HOSTLIB_EGL", "OBS_HOSTLIB_DEBUG"):
            self.env.pop(name, None)

    def stub(self, name: str, body: str) -> Path:
        path = self.bin / name
        path.write_text(body)
        path.chmod(0o755)
        return path

    def run_script(self, **env: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(SCRIPT)],
            env={**self.env, **env},
            capture_output=True,
            text=True,
            timeout=120,
        )

    def test_every_loader_check_passes_on_the_stubs(self) -> None:
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        for label in (
            "build.py",
            "probe links the shims, not libva",
            "obs-hostlib-check built",
            "no version warning from the loader",
            "host libva found",
            "the real vaErrorStr answered",
            "missing libva reported",
            "vaGetDisplayDRM stub returns NULL",
            "vaErrorStr stub returns NULL",
            "bad candidate rejected with reason",
            "AppImage copy refused",
            "too-old candidate rejected",
            "preflight passes with libEGL",
            "missing libEGL: exit 127, named",
        ):
            with self.subTest(label=label):
                self.assertIn(f"PASS {label}", result.stdout)
        self.assertIn("test-hostlib: all passed", result.stdout)
        self.assertNotIn("FAIL ", result.stdout)

    def test_the_probe_binary_is_built_from_the_probe_source(self) -> None:
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        # uv was handed build.py and the AppDir it must populate.
        self.assertIn("hostlib: probe links libobs-va.so.2", result.stdout)

    def test_the_appimage_lane_runs_the_python_harness(self) -> None:
        workflow = (ROOT / ".github/workflows/appimage.yaml").read_text()
        self.assertIn("uv run --no-project build-aux/appimage/hostlib/test_hostlib.py", workflow)
        self.assertNotIn("test-hostlib.sh", workflow)

    def test_a_build_failure_stops_before_the_loader_checks(self) -> None:
        self.stub("uv", "#!/bin/sh\necho 'hostlib: no compilers' >&2\nexit 1\n")
        result = self.run_script()
        self.assertEqual(result.returncode, 1)
        self.assertIn("FAIL build.py", result.stdout + result.stderr)
        self.assertNotIn("host libva found", result.stdout)

    def test_a_loader_version_warning_fails_the_shim_check(self) -> None:
        # The warning comes from the runtime loader, so the fake probe prints it.
        self.probe.write_text(PROBE.replace(
            "echo 'hostlib: va -> ",
            "echo 'probe: no version information available (required by probe)'\necho 'hostlib: va -> ",
        ))
        result = self.run_script()
        self.assertIn("FAIL no version warning from the loader", result.stdout)
        self.assertEqual(result.returncode, 1)
        self.assertIn("test-hostlib: 1 failed", result.stdout)


if __name__ == "__main__":
    unittest.main()
