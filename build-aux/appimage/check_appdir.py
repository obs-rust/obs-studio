#!/usr/bin/env -S uv run --script
"""Check that everything AppRun points OBS at is really inside the AppDir:

    uv run --no-project build-aux/appimage/check_appdir.py <AppDir>

assemble_appdir.py runs it to fail the build, and test-appimage.sh runs it again
on the extracted AppImage. Libraries that the dynamic loader can see are covered
by assemble_appdir.py; this is about what is found at runtime through environment
variables and configuration files instead (PipeWire modules, SPA plugins,
client.conf, gconv, Qt's X11 and Wayland plugins), where a missing file is not
noticed until OBS uses it (obs-rust/obs-studio#426: no client.conf, so
pw_context_new() returned NULL and OBS crashed on start with a PipeWire screen
capture in the scene).

Ported off the 136-line shell script in #962; assemble_appdir.py and
test-appimage.sh call this file through `uv run --no-project`.
"""

from __future__ import annotations

import os
import re
import sys
from fnmatch import fnmatch
from pathlib import Path

# `export NAME="$HERE..."` lines: the shell expanded them in a clean shell and
# read the result back with `env`. `[A-Z_]+` and the literal `$HERE` are what the
# grep selected, so a conditional `... && export NAME="$HERE..."` is not one.
_EXPORT_RE = re.compile(r'^export ([A-Z_]+)="\$HERE')
# `$NAME` and `${NAME}` / `${NAME:-default}`: the only expansions those lines use.
_VAR_RE = re.compile(r"\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-([^}]*))?\}|\$([A-Za-z_][A-Za-z0-9_]*)")
# `context.modules` entries carry `name = libpipewire-module-...`.
_MODULE_RE = re.compile(r"name *= *(libpipewire-module-[A-Za-z0-9_-]*)")
# `context.spa-libs` maps a factory glob to a path below SPA_PLUGIN_DIR.
_SPA_RE = re.compile(r"^[^#]*= *([a-z0-9_-]+/libspa-[a-z0-9_-]*)")
# SRT_VERSION is a bare x.y.z string in .rodata; the shell read it with `grep -xE`.
_SRT_VERSION_RE = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+")
_SRT_SONAME_RE = re.compile(rb"libsrt[A-Za-z0-9_-]*\.so(?:\.[0-9]+)*")
_SRT_NEEDED_RE = re.compile(rb"libsrt[A-Za-z0-9_-]*\.so")

# The Qt plugins AppRun needs: X11 and Wayland platforms, and what the Wayland
# one needs to show a window (xdg-shell) and draw into it (wayland-egl).
QT_PLUGINS = (
    "platforms/libqxcb.so",
    "platforms/libqwayland.so",
    "wayland-shell-integration/libxdg-shell.so",
    "wayland-graphics-integration-client/libqt-plugin-wayland-egl.so",
)
# PipeWire modules a stream cannot connect without; the shared SPA support plugin.
PW_REQUIRED = (
    "libpipewire-module-protocol-native.so",
    "libpipewire-module-client-node.so",
    "libpipewire-module-adapter.so",
)
SPA_REQUIRED = "support/libspa-support.so"
# Releases before this have public advisories (GHSA-6xg9-784j-24rm, ...).
SRT_MIN = "1.5.7"


def _expand(value: str, env: dict[str, str]) -> str:
    """Expand `$NAME`, `${NAME}` and `${NAME:-default}` against `env`."""
    def replace(match: re.Match[str]) -> str:
        braced, default, bare = match.group(1), match.group(2), match.group(3)
        if braced is None:
            return env.get(bare, "")
        current = env.get(braced, "")
        # `${NAME:-default}` uses the default when NAME is unset or empty.
        return current if current else (default or "")

    return _VAR_RE.sub(replace, value)


def _exported(apprun: Path, appdir: Path) -> dict[str, str]:
    """The environment AppRun's top-level `export NAME="$HERE..."` lines make.

    The shell ran those lines through `env -i HERE=... PATH=... sh -c ...` and
    read `env`; the only variables it could reference are HERE and PATH, so the
    expansion does not depend on the caller's environment.
    """
    env = {"HERE": str(appdir), "PATH": os.environ.get("PATH", "")}
    result: dict[str, str] = {}
    for line in apprun.read_text(encoding="utf-8", errors="replace").splitlines():
        match = _EXPORT_RE.match(line)
        if match is None:
            continue
        raw = line.split('="', 1)[1].rstrip()
        result[match.group(1)] = _expand(raw[:-1] if raw.endswith('"') else raw, env)
    return result


def _under_appdir(path: str, appdir: Path) -> bool:
    """The shell's case pattern `"$APPDIR"/*` over the path's first component."""
    return path.startswith(f"{appdir}/")


def _relative(path: str, appdir: Path) -> str:
    """`${f#"$APPDIR"/}`: drop the AppDir prefix for a message."""
    return path[len(f"{appdir}/"):] if _under_appdir(path, appdir) else path


def _check_exported_paths(exported: dict[str, str], appdir: Path) -> tuple[int, list[str]]:
    """Every AppDir path AppRun exports must exist (and dirs must be non-empty)."""
    count = 0
    errors: list[str] = []
    for name, value in exported.items():
        first = value.split(":", 1)[0]
        if not _under_appdir(first, appdir):
            continue
        count += 1
        path = Path(first)
        if not path.exists():
            errors.append(f"AppRun exports {name}={first}, which is not in the AppImage")
        elif path.is_dir() and not any(path.iterdir()) and name != "GIO_MODULE_DIR":
            # (GIO_MODULE_DIR is empty on purpose: see AppRun)
            errors.append(f"AppRun exports {name}={first}, which is empty")
    if count == 0:
        errors.append("found no AppDir paths in AppRun's environment block")
    return count, errors


def _spa_libs(text: str) -> list[str]:
    """The paths in the `context.spa-libs = { ... }` block of client.conf."""
    lines = text.splitlines()
    start = next((index for index, line in enumerate(lines) if "context.spa-libs" in line), None)
    if start is None:
        return []
    libs: list[str] = []
    for line in lines[start:]:
        match = _SPA_RE.match(line)
        if match is not None:
            libs.append(match.group(1))
        if "}" in line:
            break
    return libs


def _check_client_conf(conf: str, module_dir: str, spa_dir: str) -> tuple[list[str], list[str]]:
    """The modules and SPA plugins client.conf loads must be bundled."""
    text = Path(conf).read_text(encoding="utf-8", errors="replace")
    notes: list[str] = []
    errors: list[str] = []
    # context.modules: name = libpipewire-module-...; optional ones carry "ifexists".
    for block in text.replace("\n", "").split("}"):
        if "{" not in block:
            continue
        found = _MODULE_RE.findall(block)
        if not found:
            continue
        # The shell's `sed 's/.*name *= *\(...\).*/\1/'` took the last match.
        module = found[-1]
        if not os.path.exists(f"{module_dir}/{module}.so"):
            if "ifexists" in block:
                notes.append(f"optional PipeWire module {module} is not bundled")
            else:
                errors.append(f"client.conf loads {module}, which is not in {module_dir}")
    for lib in _spa_libs(text):
        if not os.path.exists(f"{spa_dir}/{lib}.so"):
            errors.append(f"client.conf maps a SPA factory to {lib}, which is not in {spa_dir}")
    return notes, errors


def _check_pipewire(exported: dict[str, str]) -> tuple[list[str], list[str]]:
    """client.conf and everything it loads must be bundled. Returns (notes, errors)."""
    module_dir = exported.get("PIPEWIRE_MODULE_DIR", "")
    spa_dir = exported.get("SPA_PLUGIN_DIR", "")
    conf = f"{exported.get('PIPEWIRE_CONFIG_DIR', '')}/client.conf"
    notes: list[str] = []
    errors: list[str] = []
    if not os.path.isfile(conf):
        errors.append(f"PipeWire client.conf is not bundled (expected {conf}); pw_context_new() fails without it")
    else:
        notes, errors = _check_client_conf(conf, module_dir, spa_dir)
    for rel in PW_REQUIRED:
        if not os.path.exists(f"{module_dir}/{rel}"):
            errors.append(f"{rel} is not bundled; OBS cannot connect a PipeWire stream without it")
    if not os.path.exists(f"{spa_dir}/{SPA_REQUIRED}"):
        errors.append(f"{SPA_REQUIRED} is not bundled; OBS cannot connect a PipeWire stream without it")
    return notes, errors


def _check_gconv(exported: dict[str, str], appdir: Path) -> list[str]:
    """gconv belongs to glibc: checked only when the AppImage bundles glibc.

    Without it the host's glibc brings its own (#926). The shell read GCONV_PATH
    from the export block, which never carries it (AppRun exports it
    conditionally); an empty value makes the glob `/gconv-modules*`. Kept as-is.
    """
    if not os.path.isdir(f"{appdir}/glibc"):
        return []
    gconv = exported.get("GCONV_PATH", "")
    # `ls "$GCONV"/gconv-modules*`: an empty GCONV_PATH globs from the root.
    if not any(fnmatch(entry.name, "gconv-modules*") for entry in Path(gconv or "/").glob("gconv-modules*")):
        return [f"no gconv-modules index in {gconv}"]
    return []


def _check_qt(exported: dict[str, str]) -> list[str]:
    """The X11 and Wayland platform plugins OBS loads must be bundled."""
    plugins = exported.get("QT_PLUGIN_PATH", "")
    return [f"the Qt plugin {p} is not bundled" for p in QT_PLUGINS if not os.path.exists(f"{plugins}/{p}")]


def _find_srt_libs(appdir: Path) -> list[str]:
    """`find "$APPDIR" \\( -type f -o -type l \\) -name 'libsrt*.so*'`, sorted."""
    found = [
        str(Path(directory, name).relative_to(appdir))
        for directory, _dirnames, filenames in os.walk(appdir)
        for name in filenames
        if fnmatch(name, "libsrt*.so*")
    ]
    return sorted(found)


def _grep_files(root: Path, pattern: re.Pattern[bytes]) -> list[Path]:
    """`grep -rla`: the files under root whose bytes contain the pattern."""
    hits: list[Path] = []
    for directory, _dirnames, filenames in os.walk(root):
        for name in sorted(filenames):
            path = Path(directory, name)
            try:
                data = path.read_bytes()
            except OSError:
                continue
            if pattern.search(data):
                hits.append(path)
    return sorted(hits)


def _check_libsrt(appdir: Path) -> tuple[str, list[str]]:
    """One bundled libsrt, at least SRT_MIN, and no second copy referenced."""
    libs = _find_srt_libs(appdir)
    if len(libs) != 1:
        return "", [f"expected exactly one libsrt in the AppImage, found {len(libs)}: {' '.join(libs)}"]
    lib = f"{appdir}/{libs[0]}"
    name = os.path.basename(lib)
    # `tr '\0' '\n' < "$SRT_LIB"`: SRT_VERSION is a bare x.y.z string in .rodata,
    # read without binutils (the test containers have none).
    data = Path(lib).read_bytes().replace(b"\x00", b"\n")
    versions = [line for line in data.decode("latin-1").split("\n") if _SRT_VERSION_RE.fullmatch(line)]
    if len(versions) != 1:
        return "", [f"cannot read the version of {libs[0]} (version strings found: {' '.join(versions)})"]
    version = versions[0]
    if tuple(int(part) for part in version.split(".")) < tuple(int(part) for part in SRT_MIN.split(".")):
        return "", [f"the bundled {libs[0]} is libsrt {version}; at least {SRT_MIN} is required"]
    # No bundled ELF file may ask for another libsrt (libsrt-gnutls.so.1.5, ...):
    # the loader would map a second copy, from the host if not from the AppImage.
    errors: list[str] = []
    for path in _grep_files(appdir / "usr", _SRT_NEEDED_RE):
        sonames = sorted({match.decode() for match in _SRT_SONAME_RE.findall(path.read_bytes())})
        for soname in sonames:
            if soname != name:
                errors.append(f"{_relative(str(path), appdir)} needs {soname}, a second libsrt next to the bundled {name}")
    return version, errors


def check(appdir: Path) -> int:
    """Exit code: 0 when every runtime file AppRun points at is bundled."""
    exported = _exported(appdir / "AppRun", appdir)
    count, errors = _check_exported_paths(exported, appdir)
    notes, pipewire_errors = _check_pipewire(exported)
    errors += pipewire_errors
    errors += _check_gconv(exported, appdir)
    errors += _check_qt(exported)
    version, libsrt_errors = _check_libsrt(appdir)
    errors += libsrt_errors
    for note in notes:
        print(note)
    for error in errors:
        print(f"::error::{error}")
    if errors:
        print("check-appdir: FAILED", file=sys.stderr)
        return 1
    print(
        f"check-appdir: all {count} AppDir paths that AppRun exports exist; "
        f"PipeWire, gconv and Qt X11/Wayland runtime files are bundled; one libsrt ({version})"
    )
    return 0


def main(argv: list[str]) -> int:
    if not argv:
        print("usage: check_appdir.py <AppDir>", file=sys.stderr)
        return 2
    return check(Path(argv[0]).resolve())


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
