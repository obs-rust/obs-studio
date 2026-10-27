#!/usr/bin/env -S uv run --script
"""NixOS lane of the release gate (obs-rust/obs-studio#926): run the packaged
AppImage the way NixOS users do, through nixpkgs' appimageTools.wrapType2
(no FHS, no driver-path workaround), under Xvfb, and check that OBS starts:
the host-library preflight passes, OBS reaches its main window and keeps
running, and VA-API is reported unavailable rather than stopping the start
(NixOS has no libva where the loader looks; #959).
Runs inside the nixos/nix image; nothing is compiled here except Nix's own
wrapper around the already-built AppImage and the NVML stand-in.

    uv run build-aux/appimage/test_appimage_nixos.py <AppImage> <out-dir>
"""

from __future__ import annotations

import glob
import os
import re
import shutil
import stat
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
# The nixpkgs the wrapper and the test tools come from (the revision a current
# NixOS 26.05 system runs).
NIXPKGS = "github:NixOS/nixpkgs/c5c4a43b0e8056328ec4529f735cabdb8f1942bb"
NIX = ("nix", "--extra-experimental-features", "nix-command flakes")
DRIVER = Path("/run/opengl-driver")
NVML = DRIVER / "lib/libnvidia-ml.so.1"
WRAPPED = Path("/tmp/obs")
OBS = WRAPPED / "bin/obs-studio-rust"
# Set in the phase that runs inside the nix shell that carries Xvfb and dbus.
INNER = "OBS_NIXOS_START_PHASE"
SHAKEDOWN_SECONDS = 45


def run(command: list[str], env: dict[str, str] | None = None, check: bool = False) -> subprocess.CompletedProcess:
    result = subprocess.run(command, capture_output=True, text=True, env=env)
    if check and result.returncode != 0:
        raise SystemExit(
            f"{' '.join(command)}\nexited {result.returncode}:\n{result.stderr}{result.stdout}"
        )
    return result


def child_environment(**updates: str) -> dict[str, str]:
    environment = os.environ.copy()
    environment.update(updates)
    return environment


def group(title: str) -> None:
    print(f"::group::{title}", flush=True)


def endgroup() -> None:
    print("::endgroup::", flush=True)


def locate(appimage: str) -> Path:
    """The AppImage's path; the workflow step hands over a glob."""
    matches = sorted(glob.glob(appimage))
    if not matches:
        raise SystemExit(f"test-appimage-nixos: no AppImage matches {appimage}")
    return Path(matches[0]).resolve()


def wrap(appimage: Path) -> None:
    """Wrap the AppImage with appimageTools.wrapType2, as a NixOS user would."""
    group("Wrap the AppImage with appimageTools.wrapType2")
    expression = f"""
  (builtins.getFlake "{NIXPKGS}").legacyPackages.x86_64-linux.appimageTools.wrapType2 {{
    pname = "obs-studio-rust"; version = "release-gate"; src = {appimage};
  }}"""
    run([*NIX, "build", "--impure", "-o", str(WRAPPED), "--expr", expression], check=True)
    print("\n".join(sorted(entry.name for entry in (WRAPPED / "bin").iterdir())))
    endgroup()


def link_tree(source: Path, target: Path) -> None:
    """A directory of links into source, like `cp -rs source/. target`."""
    for entry in source.rglob("*"):
        destination = target / entry.relative_to(source)
        if entry.is_dir():
            destination.mkdir(parents=True, exist_ok=True)
        else:
            destination.parent.mkdir(parents=True, exist_ok=True)
            if destination.is_symlink():
                destination.unlink()
            destination.symlink_to(entry)


def driver_setup(out: Path) -> None:
    """The graphics driver the NixOS way (/run/opengl-driver)."""
    group("Graphics driver the NixOS way (/run/opengl-driver)")
    # NixOS's hardware.graphics puts the driver closure at /run/opengl-driver;
    # AppRun points EGL, GBM, VA-API and Vulkan there (#722). No GPU here:
    # Mesa's software rasterizer, as the other lanes use.
    mesa = Path(run([*NIX, "build", "--no-link", "--print-out-paths", f"{NIXPKGS}#mesa"], check=True).stdout.strip())
    # A directory of links into Mesa rather than one link, so the NVIDIA
    # stand-in below can sit beside Mesa's libraries.
    DRIVER.mkdir(parents=True, exist_ok=True)
    link_tree(mesa, DRIVER)
    for entry in [DRIVER, *DRIVER.rglob("*")]:
        if not entry.is_symlink():
            entry.chmod(entry.stat().st_mode | stat.S_IWUSR)
    print("\n".join(sorted(p.name for p in (DRIVER / "share/glvnd/egl_vendor.d").iterdir())))
    # The NVIDIA driver puts NVML there too, outside the loader path. A
    # stand-in (fake-nvml.c) records the path OBS's GPU usage sampler loads it
    # from (#1028); NixOS has no other copy for it to find.
    run(
        [*NIX, "shell", f"{NIXPKGS}#gcc", "--command", "gcc", "-O2", "-Wall", "-Wextra", "-Werror",
         "-shared", "-fPIC", "-o", str(NVML), str(HERE / "fake-nvml.c"), "-ldl"],
        check=True,
    )
    os.environ["OBS_TEST_NVML_MARKER"] = str(out.resolve() / "nvml-loaded")
    Path(os.environ["OBS_TEST_NVML_MARKER"]).unlink(missing_ok=True)
    os.environ["LIBGL_ALWAYS_SOFTWARE"] = "1"
    endgroup()


def start_phase(out: Path) -> int:
    """Start OBS under Xvfb, let it run, and record whether it kept running."""
    group("Start OBS under Xvfb")
    display = ":99"
    runtime, home = Path("/tmp/xdg"), Path("/tmp/home")
    for directory in (runtime, home):
        directory.mkdir(parents=True, exist_ok=True)
    runtime.chmod(0o700)
    env = child_environment(DISPLAY=display, XDG_RUNTIME_DIR=str(runtime), HOME=str(home))
    xvfb = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", "1280x720x24"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=env,
    )
    time.sleep(3)
    dbus_daemon = Path(shutil.which("dbus-daemon") or "dbus-daemon")
    session = dbus_daemon.parent.parent / "share/dbus-1/session.conf"
    with (out / "obs-stdout.log").open("w") as log:
        obs = subprocess.Popen(
            ["dbus-run-session", f"--config-file={session}", "--", str(OBS), "--verbose", "--disable-shutdown-check"],
            stdout=log, stderr=subprocess.STDOUT, env=env,
        )
        time.sleep(SHAKEDOWN_SECONDS)
        state = "running" if obs.poll() is None else f"exited {obs.returncode}"
        (out / "state").write_text(state + "\n")
        subprocess.run(["pkill", "-TERM", "-f", "[u]sr/bin/obs-studio-rust-gui"], check=False)
        time.sleep(5)
        xvfb.terminate()
    endgroup()
    return 0


def errors(log: str, state: str, marker: str | None) -> list[str]:
    """The failures in a NixOS start, as identifiers; empty when OBS started."""
    found = []
    if state != "running":
        found.append("state")
    if re.search(r"cannot start|needs these libraries from your system", log):
        found.append("preflight")
    if not re.search(r"Switched to scene|Loaded scenes|Created scene collection", log):
        found.append("scene")
    if not re.search(r"Loading up OpenGL on adapter", log):
        found.append("opengl")
    if not (marker and str(NVML) in marker.splitlines()):
        found.append("nvml")
    if re.search(r"error while loading shared libraries|Segmentation fault|SIGSEGV", log):
        found.append("crash")
    return found


def message(kind: str, log_path: Path, state: str, marker: str | None) -> str:
    return {
        "state": f"OBS did not keep running on NixOS ({state}); see {log_path}",
        "preflight": "the host-library preflight refused to start OBS on NixOS",
        "scene": "OBS did not reach its main window",
        "opengl": "OBS did not initialize OpenGL",
        "nvml": f"OBS's GPU usage sampler did not load NVML from {NVML.parent} ({marker or 'not loaded'})",
        "crash": "OBS crashed or could not load its libraries on NixOS",
    }[kind]


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    out = Path(argv[2])
    out.mkdir(parents=True, exist_ok=True)
    if os.environ.get(INNER):
        return start_phase(out)

    wrap(locate(argv[1]))
    driver_setup(out)
    # Xvfb, dbus and the process tools live in the nix shell, so the start
    # phase runs there, in this same file, under the same `uv run` interpreter
    # (sys.executable) the outer phase already uses: the lane needs nix only
    # for the X tools, not for Python (#637).
    started = run(
        [*NIX, "shell", f"{NIXPKGS}#xorg.xorgserver", f"{NIXPKGS}#dbus",
         f"{NIXPKGS}#procps", f"{NIXPKGS}#coreutils", "--command",
         sys.executable, str(Path(__file__).resolve()), *argv[1:]],
        env=child_environment(**{INNER: "1"}),
    )
    print(started.stdout, end="")
    if started.returncode != 0:
        print(started.stderr, end="", file=sys.stderr)

    log_path = out / "obs-stdout.log"
    log = log_path.read_text(errors="replace") if log_path.exists() else ""
    state_path = out / "state"
    state = state_path.read_text().strip() if state_path.exists() else "unknown"
    marker_path = Path(os.environ["OBS_TEST_NVML_MARKER"])
    marker = marker_path.read_text() if marker_path.exists() else None
    print(f"OBS after {SHAKEDOWN_SECONDS} s: {state}")
    found = errors(log, state, marker)
    for kind in found:
        print(f"::error::{message(kind, log_path, state, marker)}")
    for line in log.splitlines():
        if re.search(r"hostlib: va|Loading up OpenGL on adapter|OBS-Studio-Rust [0-9]", line):
            print(line)
    if found:
        print("\n".join(log.splitlines()[-40:]))
        return 1
    print("NixOS: OBS AppImage started through appimageTools.wrapType2")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
