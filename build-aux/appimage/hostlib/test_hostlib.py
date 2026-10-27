#!/usr/bin/env -S uv run --script
"""Tests the host-library loader (obs-rust/obs-studio#959) on a small AppDir:
a program that links libva.so.2 and libva-drm.so.2 the way FFmpeg does goes
through build.py, then runs with the host's libva, without it, with a bad
candidate, with a copy inside the "AppImage", and through the preflight.
Needs a C/C++ compiler, patchelf, binutils, uv, libva-devel and libEGL.

    uv run --no-project build-aux/appimage/hostlib/test_hostlib.py
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent

# No device needed: a bad fd gives a NULL display from the real libva too, so
# this probes loading and the fallback, not the GPU.
PROBE_C = """\
#include <stdio.h>
#include <va/va.h>
#include <va/va_drm.h>
int main(void)
{
\t/* No device needed: a bad fd gives a NULL display from the real libva too,
\t * so this probes loading and the fallback, not the GPU. */
\tVADisplay d = vaGetDisplayDRM(-1);
\tint major = 0, minor = 0;
\tVAStatus s = d ? vaInitialize(d, &major, &minor) : VA_STATUS_ERROR_INVALID_DISPLAY;
\t/* vaCreateSurfaces is versioned (VA_API_0.33.0): the shim must define that version. */
\tif (d && s == VA_STATUS_SUCCESS)
\t\tvaCreateSurfaces(d, VA_RT_FORMAT_YUV420, 16, 16, NULL, 0, NULL, 0);
\tprintf("display=%s status=0x%x str=%s\\n", d ? "yes" : "null", s, vaErrorStr(VA_STATUS_ERROR_UNIMPLEMENTED));
\treturn 0;
}
"""


class Report:
    """The script's PASS/FAIL lines; any failure fails the run at the end."""

    def __init__(self) -> None:
        self.failures = 0

    def check(self, condition: object, label: str, detail: str = "") -> bool:
        passed = bool(condition)
        if passed:
            print(f"PASS {label}")
        else:
            self.failures += 1
            print(f"FAIL {label}" + (f": {detail.strip()}" if detail.strip() else ""))
        return passed


def run(command: list[str], env: dict[str, str] | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(command, capture_output=True, text=True, env=env)


def child_environment(**updates: str) -> dict[str, str]:
    """This process's environment plus the variables a child needs."""
    environment = os.environ.copy()
    environment.update(updates)
    return environment


def host_library(soname: str) -> Path:
    """The host's copy of a library, as the dynamic linker lists it."""
    listing = run(["ldconfig", "-p"]).stdout
    found = re.search(rf".*{re.escape(soname)} \(libc6,x86-64\) => (.+)", listing)
    if not found:
        raise SystemExit(f"test-hostlib: ldconfig does not list {soname}")
    return Path(found.group(1))


def main() -> int:
    report = Report()
    with tempfile.TemporaryDirectory(prefix="test-hostlib-") as work_s:
        work = Path(work_s)
        app = work / "AppDir"
        (app / "usr/bin").mkdir(parents=True)
        (app / "usr/lib").mkdir(parents=True)
        (work / "probe.c").write_text(PROBE_C, encoding="utf-8")
        compiled = run([
            "cc", "-o", str(app / "usr/bin/probe"), str(work / "probe.c"),
            "-lva", "-lva-drm", "-Wl,-rpath,$ORIGIN/../lib",
        ])
        if compiled.returncode != 0:
            report.check(False, "probe builds", compiled.stderr + compiled.stdout)
            return 1

        # --- build ----------------------------------------------------------
        build = run(["uv", "run", "--no-project", str(HERE / "build.py"), str(app)])
        output = build.stderr + build.stdout
        if build.returncode != 0:
            report.check(False, "build.py", output)
            return 1
        report.check(True, "build.py")
        print(output, end="" if output.endswith("\n") else "\n")
        needed = run(["patchelf", "--print-needed", str(app / "usr/bin/probe")]).stdout.split()
        report.check(
            "libobs-va.so.2" in needed and "libobs-va-drm.so.2" in needed and "libva.so.2" not in needed,
            "probe links the shims, not libva",
            f"probe's DT_NEEDED: {needed}",
        )
        report.check(
            os.access(app / "usr/bin/obs-hostlib-check", os.X_OK),
            "obs-hostlib-check built",
            "no obs-hostlib-check",
        )

        def probe(**env: str) -> str:
            result = subprocess.run(
                [str(app / "usr/bin/probe")],
                capture_output=True,
                text=True,
                env=child_environment(APPDIR=str(app), **env),
            )
            return result.stdout + result.stderr

        # --- 1. the host's libva --------------------------------------------
        out = probe()
        report.check(
            "no version information" not in out,
            "no version warning from the loader",
            f"loader warns about the shim's versions: {out}",
        )
        report.check("hostlib: va -> /" in out, "host libva found", f"host libva: {out}")
        report.check(
            re.search(r"str=.*unimplemented|str=the requested function is not implemented", out),
            "the real vaErrorStr answered",
            f"vaErrorStr with host libva: {out}",
        )

        # --- 2. no libva: the stubs -----------------------------------------
        out = probe(
            OBS_HOSTLIB_VA="/nonexistent/libva.so.2",
            OBS_HOSTLIB_VA_DRM="/nonexistent/libva-drm.so.2",
        )
        for pattern, label in (
            ("hostlib: va unavailable", "missing libva reported"),
            ("display=null", "vaGetDisplayDRM stub returns NULL"),
            ("str=(null)", "vaErrorStr stub returns NULL"),
        ):
            report.check(pattern in out, label, f"{label.split()[0]}: {out}")

        # --- 3. a candidate that does not load (wrong ELF class) -------------
        bad = work / "libva.so.2"
        bad.write_bytes(b"\x7fELF\x01\x01\x01")  # a truncated 32-bit header
        out = probe(OBS_HOSTLIB_VA=str(bad), OBS_HOSTLIB_DEBUG="1")
        report.check(f"{bad}: does not load" in out, "bad candidate rejected with reason", f"bad candidate: {out}")

        # --- 4. a copy inside the AppImage is refused ------------------------
        inside = app / "usr/lib/libva.so.2"
        shutil.copyfile(host_library("libva.so.2"), inside)  # follows the symlink
        out = probe(OBS_HOSTLIB_VA=str(inside), OBS_HOSTLIB_DEBUG="1")
        report.check("is inside the AppImage" in out, "AppImage copy refused", f"AppImage copy: {out}")
        inside.unlink()

        # --- 5. a library without the required symbols -----------------------
        (work / "old.c").write_text("int vaUnrelated(void) { return 0; }\n", encoding="utf-8")
        old = work / "libva-old.so.2"
        built = run(["cc", "-shared", "-fPIC", "-o", str(old), str(work / "old.c")])
        if built.returncode != 0:
            report.check(False, "old candidate builds", built.stderr)
            return 1
        out = probe(OBS_HOSTLIB_VA=str(old), OBS_HOSTLIB_DEBUG="1")
        report.check("lacks vaInitialize" in out, "too-old candidate rejected", f"old candidate: {out}")

        # --- 6. preflight ----------------------------------------------------
        preflight = [str(app / "usr/bin/obs-hostlib-check")]
        report.check(
            run(preflight, env=child_environment(APPDIR=str(app))).returncode == 0,
            "preflight passes with libEGL",
            "preflight failed",
        )
        without = run(preflight, env=child_environment(APPDIR=str(app), OBS_HOSTLIB_EGL="/nonexistent/libEGL.so.1"))
        out = without.stdout + without.stderr
        report.check(
            without.returncode == 127 and "libEGL.so.1" in out,
            "missing libEGL: exit 127, named",
            f"preflight without libEGL (rc {without.returncode}): {out}",
        )

    if report.failures:
        print(f"test-hostlib: {report.failures} failed")
        return 1
    print("test-hostlib: all passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
