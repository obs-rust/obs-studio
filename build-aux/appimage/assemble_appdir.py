#!/usr/bin/env -S uv run --script
"""Turn a `cmake --install` tree (prefix /usr) into a self-contained AppDir that
also bundles glibc, so it runs on distributions older and newer than the one
that built it.

    uv run --no-project build-aux/appimage/assemble_appdir.py <AppDir> <validated-libavutil.so.60>

AppDir already holds usr/. Ported off the 306-line shell script in #962;
build.py's `appdir` step runs it in the AppImage build image, whose PATH has uv
like every other step of that build (hostlib/build.py, set_runpaths.py,
check-glibc-floor.py).
"""

from __future__ import annotations

import hashlib
import os
import re
import shutil
import subprocess
import sys
from collections.abc import Iterator
from pathlib import Path

HERE = Path(__file__).resolve().parent

# `head -c4` of an ELF file, the magic the shell compared against '177ELF'.
ELF_MAGIC = b"\x7fELF"
# `find -perm -u+x`: the owner's execute bit, which stat reports for the file.
OWNER_EXECUTE = 0o100
# The oldest glibc the AppImage runs on (#926), the floor check-glibc-floor.py checks.
GLIBC_FLOOR = "2.28"
# The manylinux base's source-built libraries.
DEFAULT_OBS_DEPS = "/opt/obs-deps"

# Libraries that must come from the host because they are the GPU driver stack
# (the AppImage excludelist): the host's Mesa/NVIDIA/VA-API drivers are built
# against the host's copies of these and look for their backends in the host's
# directories, so a bundled copy would shadow them through the relative RPATH
# (obs-rust/obs-studio#625). Everything else is bundled.
HOST_EXCLUDE = (
    r"^(linux-vdso|libcuda|libnvidia|libnvcuvid|libGL\.so|libGLX|libEGL|libOpenGL|"
    r"libGLdispatch|libGLESv|libglapi|libgbm|libdrm|libvulkan|libva(\.so|-)|"
    r"libwayland-(client|egl|server)|libgallium|libLLVM)"
)
# glibc and the C++ runtime are the host's too when the build host is at or
# below the floor. gcc-toolset links the parts of libstdc++ newer than the
# system's 3.4.25 statically, so the host's libstdc++ is enough
# (check-glibc-floor.py checks GLIBCXX too).
GLIBC_EXCLUDE = HOST_EXCLUDE.removesuffix(")") + (
    r"|ld-linux|libc\.so|libm\.so|libmvec|libpthread|libdl\.so|librt\.so|libresolv|"
    r"libnss_|libutil\.so|libanl|libBrokenLocale|libthread_db|libstdc\+\+|libgcc_s)"
)

# The Qt plugin directories the bundle keeps. Qt must load plugins from the
# bundle only (qt.conf below); a plugin that is not bundled is not "found on the
# host instead" but silently missing, or - worse, when the host's Qt differs -
# loaded from the host and crashing (QIcon -> libqsvgicon.so).
QT_PLUGIN_DIRS = (
    "platforms",
    "platforminputcontexts",
    "platformthemes",
    "imageformats",
    "iconengines",
    "styles",
    "generic",
    "networkinformation",
    "tls",
    "xcbglintegrations",
    "egldeviceintegrations",
    "wayland-decoration-client",
    "wayland-graphics-integration-client",
    "wayland-shell-integration",
)
# The plugins OBS cannot live without (SVG icons/themes, X11).
REQUIRED_PLUGINS = (
    "platforms/libqxcb.so",
    "imageformats/libqsvg.so",
    "iconengines/libqsvgicon.so",
)

# `grep -E '[[:space:]]UND[[:space:]].*vaMapBuffer2'` of `readelf --dyn-syms`.
_UNDEFINED_VAMAP = re.compile(r"\sUND\s.*vaMapBuffer2")


class Failure(Exception):
    """A `set -e` exit: the shell stopped the build with this status."""

    def __init__(self, code: int) -> None:
        super().__init__(f"assembling the AppDir failed with status {code}")
        self.code = code


def require(code: int) -> None:
    """`set -e`: a failing command stops the build with its status."""
    if code != 0:
        raise Failure(code)


def capture(command: list[str]) -> tuple[int, str, str]:
    """Run a tool, capturing stdout and stderr separately (the shell's `$(...)`).

    A tool that is not installed reports the shell's 127 with no output, so a
    caller that treated a failure as empty output (`|| true`, `2>/dev/null`)
    keeps that behaviour.
    """
    try:
        result = subprocess.run(command, capture_output=True, text=True)
    except FileNotFoundError:
        return 127, "", ""
    return result.returncode, result.stdout, result.stderr


def is_elf(path: Path) -> bool:
    """`head -c4` of the file, against the ELF magic."""
    try:
        with path.open("rb") as handle:
            return handle.read(len(ELF_MAGIC)) == ELF_MAGIC
    except OSError:
        return False


def sed_substitute(text: str, marker: str, value: str) -> str:
    """One `sed -e "s|marker|value|"`: the first occurrence on each line."""
    return "\n".join(line.replace(marker, value, 1) for line in text.split("\n"))


def sha256_file(path: Path) -> str:
    """The file's digest, as `sha256sum` prints it."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def copy_tree_into(source: Path, destination: Path) -> None:
    """`mkdir -p destination && cp -a source/. destination/`."""
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copytree(source, destination, symlinks=True, dirs_exist_ok=True)


def find_program(name: str) -> str | None:
    """`command -v`: an absolute path or a name on PATH."""
    return shutil.which(name)


def elf_files(root: Path) -> Iterator[Path]:
    """`find root -type f \\( -name '*.so' -o -name '*.so.*' -o -perm -u+x \\)` that is an ELF."""
    for directory, dirnames, filenames in os.walk(root):
        dirnames.sort()
        for name in sorted(filenames):
            path = Path(directory) / name
            if path.is_symlink():
                continue
            in_glob = name.endswith(".so") or ".so." in name
            if not in_glob and not path.stat().st_mode & OWNER_EXECUTE:
                continue
            if is_elf(path):
                yield path


def executable_files(root: Path) -> Iterator[Path]:
    """`find root -type f -perm -u+x ! -name '*.so' ! -name '*.so.*'`."""
    for directory, dirnames, filenames in os.walk(root):
        dirnames.sort()
        for name in sorted(filenames):
            path = Path(directory) / name
            if path.is_symlink() or name.endswith(".so") or ".so." in name:
                continue
            if path.stat().st_mode & OWNER_EXECUTE:
                yield path


# --- the compatibility library ----------------------------------------------


def validate_compat(compat_lib: Path) -> None:
    """Audit the FFmpeg libavutil built against the glibc 2.28 image's old libva.

    Do not build an image that will fail on older host VA libraries: the SONAME
    must be libavutil.so.60 and the library must not still require vaMapBuffer2,
    which old libva lacks.
    """
    _, dynamic, _ = capture(["readelf", "-d", str(compat_lib)])
    if "[libavutil.so.60]" not in dynamic:
        print("::error::compatibility library has the wrong libavutil SONAME", file=sys.stderr)
        raise Failure(2)
    _, symbols, _ = capture(["readelf", "--dyn-syms", "--wide", str(compat_lib)])
    if any(_UNDEFINED_VAMAP.search(line) for line in symbols.splitlines()):
        print("::error::compatibility library still requires vaMapBuffer2", file=sys.stderr)
        raise Failure(2)


# --- the layout -------------------------------------------------------------


def gui_selection(appdir: Path, libdir: Path) -> tuple[str, str, Path, Path]:
    """GUI name, APP_ID, DATA_ROOT and MODULE_ROOT, per what the build installed."""
    if not (appdir / "usr/bin/obs-studio-rust-gui").is_file():
        return ("obs", "com.obsproject.Studio", appdir / "usr/share/obs", libdir / "obs-modules")
    # AppDir aliases serve bundled dependency discovery and its test probes.
    # The installed Debian package retains only the private library locations.
    private = libdir / "obs-studio-rust"
    for library in sorted(private.glob("lib*.so*")):
        if library.exists():
            (libdir / library.name).symlink_to(Path("obs-studio-rust") / library.name)
    return (
        "obs-studio-rust-gui",
        "io.github.obs_rust.OBSStudioRust",
        appdir / "usr/share/obs-studio-rust",
        private / "obs-modules",
    )


def system_libdir() -> Path:
    """`gcc -print-multiarch`: /usr/lib/<triplet> on Debian/Ubuntu, else /usr/lib64."""
    _, multiarch, _ = capture(["gcc", "-print-multiarch"])
    multiarch = multiarch.strip()
    if multiarch:
        candidate = Path(f"/usr/lib/{multiarch}")
        if candidate.is_dir():
            return candidate
    return Path("/usr/lib64")  # AlmaLinux (manylinux_2_28)


def version_key(value: str) -> tuple[int, ...]:
    """A `sort -V` key for the dotted versions `getconf` and the floor use."""
    return tuple(int(part) for part in re.findall(r"\d+", value))


def decide_bundle_glibc() -> int:
    """1 when the build host's glibc is newer than the floor (#926), else 0.

    A build host at or below the floor means every supported host has a newer
    glibc than the one OBS was linked against, so glibc and the C++ runtime come
    from the host and nothing of glibc is bundled (no loader, no interpreter
    patching, no gconv). Above it (the Ubuntu 26.04 build), glibc is bundled.
    """
    _, version, _ = capture(["getconf", "GNU_LIBC_VERSION"])
    build = re.sub(r"(?m)^glibc ", "", version.rstrip("\n"))
    last = max((build, GLIBC_FLOOR), key=version_key)
    bundle = 0 if last == GLIBC_FLOOR else 1
    print(f"build glibc {build}, floor {GLIBC_FLOOR}: bundle glibc={bundle}")
    return bundle


# --- Qt plugins -------------------------------------------------------------


def qt_plugins_dir(obs_deps: str, syslib: Path) -> Path:
    """Ask the Qt we built against where its plugins are.

    Never search the file system for them: a system Qt 5 in /usr/lib64/qt5/plugins
    matched first once, and Qt 5 plugins next to Qt 6 libraries would crash at
    load.
    """
    for qtpaths in (f"{obs_deps}/qt6/bin/qtpaths6", f"{obs_deps}/qt6/bin/qtpaths", "qtpaths6"):
        found = find_program(qtpaths)
        if found is None:
            continue
        code, out, _ = capture([found, "--query", "QT_INSTALL_PLUGINS"])
        if code == 0:
            plugins = Path(out.strip())
            if plugins.is_dir():
                return plugins
            break
    return syslib / "qt6/plugins"


def find_plugin(plugins: Path, need: str) -> Path | None:
    """`find "$QT_PLUGINS/.." /usr/lib/*/qt6 /usr/lib64/qt6 -path "*/$need" | head -1`."""
    directory, name = need.rsplit("/", 1)
    roots = [plugins.parent, Path("/usr/lib64/qt6"), *sorted(Path("/usr/lib").glob("*/qt6"))]
    for root in roots:
        if not root.is_dir():
            continue
        for path in sorted(root.rglob(name)):
            if path.parent.name == directory:
                return path
    return None


def report_svg_packages() -> None:
    """`dpkg -L libqt6svg6 | grep -i plugin >&2 || true`: a hint for the missing package."""
    _, listing, _ = capture(["dpkg", "-L", "libqt6svg6"])
    for line in listing.splitlines():
        if "plugin" in line.lower():
            print(line, file=sys.stderr)


def bundle_qt_plugins(appdir: Path, libdir: Path, syslib: Path, obs_deps: str) -> None:
    """Bundle the Qt plugin directories and fail without the required ones."""
    plugins = qt_plugins_dir(obs_deps, syslib)
    if not (plugins / "platforms/libqxcb.so").is_file():
        print(f"::error::no Qt 6 plugins (looked at {plugins})", file=sys.stderr)
        raise Failure(1)
    print(f"Qt plugins from: {plugins.resolve()}")
    destination = libdir / "qt6/plugins"
    destination.mkdir(parents=True, exist_ok=True)
    for name in QT_PLUGIN_DIRS:
        if (plugins / name).is_dir():
            shutil.copytree(plugins / name, destination / name, symlinks=True, dirs_exist_ok=True)
    # The ones OBS cannot live without (SVG icons/themes, X11). Search the whole
    # system if the package put them somewhere unexpected, and fail the build if
    # they do not exist.
    for need in REQUIRED_PLUGINS:
        target = destination / need
        if target.exists():
            continue
        found = find_plugin(plugins, need)
        if found is None:
            print(
                f"::error::Qt plugin {need} not found; without it OBS has no SVG icons "
                "or loads the host's copy",
                file=sys.stderr,
            )
            report_svg_packages()
            raise Failure(1)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(found.resolve(), target)
    # Qt's GTK 3 platform theme would pull GTK 3 into the bundle; OBS brings its
    # own themes and Qt keeps the xdg-desktop-portal one. (The Ubuntu Qt never
    # had it.)
    (destination / "platformthemes/libqgtk3.so").unlink(missing_ok=True)
    for path in sorted(destination.rglob("*.so")):
        print(path.relative_to(libdir))
    # Qt must only ever look at the bundled plugins (see AppRun).
    (appdir / "usr/bin/qt.conf").write_text("[Paths]\nPrefix = ..\nPlugins = lib/qt6/plugins\n")


# --- dlopen()ed runtime pieces that ldd cannot see --------------------------


def pkg_config(variable: str, package: str, fallback: str) -> str:
    """`pkg-config --variable=<variable> <package> || echo <fallback>`."""
    code, out, _ = capture(["pkg-config", f"--variable={variable}", package])
    return out.strip() if code == 0 else fallback


def bundle_runtime(libdir: Path, syslib: Path, bundle: int) -> None:
    """gconv, libxkbcommon-x11 and the PipeWire/SPA pieces AppRun points OBS at."""
    if bundle == 1:
        gconv = syslib / "gconv"
        if not gconv.is_dir():
            print(f"::error::{gconv} is missing; cannot bundle the gconv modules", file=sys.stderr)
            raise Failure(1)
        copy_tree_into(gconv, libdir / "gconv")
        for so in ("libnss_files.so.2", "libnss_dns.so.2", "libresolv.so.2"):
            if (syslib / so).exists():
                shutil.copy(syslib / so, libdir / so)
    if (syslib / "libxkbcommon-x11.so.0").exists():
        shutil.copy(syslib / "libxkbcommon-x11.so.0", libdir / "libxkbcommon-x11.so.0")
    # The bundled libpipewire loads its modules, SPA plugins and client.conf from
    # the directories AppRun points it at. Without them pw_context_new() returns
    # NULL and OBS crashes as soon as a PipeWire capture starts
    # (obs-rust/obs-studio#426), so a build host that lacks them must fail here.
    spa_dir = Path(pkg_config("plugindir", "libspa-0.2", str(syslib / "spa-0.2")))
    pw_modules = Path(pkg_config("moduledir", "libpipewire-0.3", str(syslib / "pipewire-0.3")))
    pw_share = Path(pkg_config("prefix", "libpipewire-0.3", "/usr")) / "share/pipewire"
    needed = (
        (spa_dir / "support/libspa-support.so", "libspa-0.2-modules"),
        (pw_modules / "libpipewire-module-protocol-native.so", "libpipewire-0.3-modules"),
        (pw_share / "client.conf", "pipewire-bin"),
    )
    for path, package in needed:
        if not path.exists():
            print(f"::error::{path} not found; install {package} on the build host", file=sys.stderr)
            raise Failure(1)
    copy_tree_into(spa_dir, libdir / "spa-0.2")
    copy_tree_into(pw_modules, libdir / "pipewire-0.3")
    copy_tree_into(pw_share, libdir.parent / "share/pipewire")


def record_modules(data_root: Path, module_root: Path) -> None:
    """`find MODULE_ROOT -name '*.so' -printf '%f\\n' | sort > appimage-modules.txt`."""
    data_root.mkdir(parents=True, exist_ok=True)
    if not module_root.exists():
        print(f"::error::no modules under {module_root}", file=sys.stderr)
        raise Failure(1)
    names = sorted(path.name for path in module_root.rglob("*.so"))
    (data_root / "appimage-modules.txt").write_text("".join(f"{name}\n" for name in names))


# --- bundle the closure of every ELF in the AppDir --------------------------


def bundle_deps(path: Path, libdir: Path, exclude: re.Pattern[str]) -> tuple[list[str], list[str]]:
    """Copy what `ldd` resolves and the EXCLUDE set keeps on the host.

    Returns the copied AppDir paths and the `not found` lines the shell failed on.
    """
    _, out, _ = capture(["ldd", str(path)])
    copied: list[str] = []
    for line in out.splitlines():
        if "=> /" not in line:
            continue
        fields = line.split()
        library = fields[2] if len(fields) > 2 else ""
        name = Path(library).name
        if exclude.search(name):
            continue
        if not (libdir / name).exists():
            shutil.copy(library, libdir / name)
            copied.append(str(libdir / name))
    return copied, [line for line in out.splitlines() if "not found" in line]


def walk_closure(appdir: Path, libdir: Path, exclude: re.Pattern[str]) -> None:
    """Bundle the shared-library closure of every ELF in the AppDir, breadth first."""
    queue = list(elf_files(appdir / "usr"))
    while queue:
        discovered: list[str] = []
        for path in queue:
            copied, unresolved = bundle_deps(path, libdir, exclude)
            if unresolved:
                print(f"::error::unresolved dependency for {path}", file=sys.stderr)
                for line in unresolved:
                    print(line, file=sys.stderr)
                raise Failure(1)
            discovered.extend(copied)
        queue = [Path(entry) for entry in discovered]


# --- the rest of glibc, the C/C++ runtime and the loader --------------------


def bundle_glibc_rest(libdir: Path, syslib: Path, names: list[str]) -> None:
    """Bundle every library of the package the bundled libc.so.6 comes from.

    The closure holds only the parts of glibc that OBS links. The host's GPU
    drivers run on the bundled loader and libc too and link more of it (the
    NVIDIA driver's libcuda and libEGL_nvidia: librt, libpthread, libdl). Taken
    from the host, those belong to its glibc and need private symbols of its
    version: on a host with a newer glibc libcuda did not load and NVENC was
    missing.
    """
    _, ownership, _ = capture(["dpkg-query", "-S", str(syslib / "libc.so.6")])
    lines = ownership.splitlines()
    glibc_pkg = ":".join(lines[0].split(":")[:2]) if lines else ""
    if not glibc_pkg:
        print(
            f"::error::no package owns {syslib / 'libc.so.6'}; cannot bundle the rest of glibc",
            file=sys.stderr,
        )
        raise Failure(1)
    _, listing, _ = capture(["dpkg-query", "-L", glibc_pkg])
    pattern = re.compile(rf"^{re.escape(str(syslib))}/[^/]+\.so(\.[0-9]+)*$")
    for line in listing.splitlines():
        if not pattern.match(line):
            continue
        name = Path(line).name
        if name.startswith("ld-linux") or name.startswith("libthread_db"):
            continue  # the loader is bundled below, in lib64; libthread_db is the debugger's
        if not (libdir / name).exists():
            shutil.copy(line, libdir / name)
            print(f"glibc ({glibc_pkg}): {name}")
        names.append(name)


def move_glibc_runtime(libdir: Path, glibc_dir: Path, names: list[str]) -> None:
    """glibc, libstdc++ and libgcc_s go to their own directory (see AppRun).

    They must be switchable: a host with a newer glibc runs its own Mesa/LLVM
    (built against that glibc and a newer libstdc++), which cannot load against
    the bundled, older ones (obs-rust/obs-studio#621). AppRun chooses at start.
    They are therefore NOT in usr/lib: the RPATH of every ELF file ends with a
    fixed directory that AppRun fills with the bundled copies, or leaves empty
    so that the host's are found.
    """
    for name in sorted({*names, "libstdc++.so.6", "libgcc_s.so.1"}):
        if (libdir / name).exists():
            shutil.move(str(libdir / name), str(glibc_dir / name))


def glibc_dir_hash(glibc_dir: Path) -> str:
    """`cd glibc && find . -type f | sort | xargs sha256sum | sha256sum | cut -c1-12`.

    The directory name identifies this exact C/C++ runtime, so that a rebuild
    with a different libstdc++ never reuses a stale copy of it in /tmp.
    """
    digest = hashlib.sha256()
    regular = (path for path in glibc_dir.rglob("*") if path.is_file() and not path.is_symlink())
    for path in sorted(regular, key=lambda p: p.relative_to(glibc_dir).as_posix()):
        relative = path.relative_to(glibc_dir).as_posix()
        digest.update(f"{sha256_file(path)}  ./{relative}\n".encode())
    return digest.hexdigest()[:12]


def write_glibc_versions(glibc_dir: Path) -> None:
    """What AppRun compares the host against: the glibc version and the newest GLIBCXX."""
    _, version, _ = capture([str(glibc_dir / "ld-linux-x86-64.so.2"), "--version"])
    first = version.splitlines()[0] if version.splitlines() else ""
    match = re.search(r"release version ([0-9]+\.[0-9]+)", first)
    glibc = f"{match.group(1)}\n" if match else ""
    (glibc_dir / "GLIBC_VERSION").write_text(glibc)
    library = glibc_dir / "libstdc++.so.6"
    data = library.read_bytes() if library.exists() else b""
    minors = re.findall(rb"GLIBCXX_3\.4\.([0-9]+)", data)
    glibcxx = f"{max(int(value) for value in minors)}\n" if minors else ""
    (glibc_dir / "GLIBCXX_MINOR").write_text(glibcxx)
    if not glibc or not glibcxx:
        print("::error::cannot determine the bundled glibc/libstdc++ version", file=sys.stderr)
        raise Failure(1)
    print(f"bundled glibc {glibc.strip()}, GLIBCXX_3.4.{glibcxx.strip()}")


def install_loader(appdir: Path, libdir: Path, syslib: Path, glibc_dir: Path) -> str:
    """Every executable gets the bundled loader as interpreter (see AppRun)."""
    (appdir / "lib64").mkdir(parents=True, exist_ok=True)
    shutil.copy(syslib / "ld-linux-x86-64.so.2", appdir / "lib64/ld-linux-x86-64.so.2")
    (libdir / "ld-linux-x86-64.so.2").unlink(missing_ok=True)
    loader_dir = f"/tmp/.obs-rust-appimage-{glibc_dir_hash(glibc_dir)}"
    loader = f"{loader_dir}/ld-linux-x86-64.so.2"
    for path in executable_files(appdir / "usr"):
        if not is_elf(path):
            continue
        if capture(["patchelf", "--print-interpreter", str(path)])[0] != 0:
            continue
        capture(["patchelf", "--set-interpreter", loader, str(path)])
        print(f"interpreter patched: {path.relative_to(appdir)}")
    return loader_dir


def bundle_glibc_section(appdir: Path, libdir: Path, syslib: Path, bundle: int) -> str:
    """The glibc rest, the loader and the interpreter patching; empty at the floor."""
    if bundle != 1:
        # Every ELF keeps the system interpreter and finds glibc and the C++
        # runtime on the host; AppRun has nothing to install.
        return ""
    glibc_names: list[str] = []
    if (libdir / "libc.so.6").exists():
        bundle_glibc_rest(libdir, syslib, glibc_names)
    glibc_dir = appdir / "glibc"
    glibc_dir.mkdir(parents=True, exist_ok=True)
    if (libdir / "libc.so.6").exists():
        move_glibc_runtime(libdir, glibc_dir, glibc_names)
    loader_source = syslib / "ld-linux-x86-64.so.2"
    if not loader_source.exists():
        print(f"::error::{loader_source} is missing; cannot bundle the loader", file=sys.stderr)
        raise Failure(1)
    shutil.copy(loader_source, glibc_dir / "ld-linux-x86-64.so.2")
    write_glibc_versions(glibc_dir)
    return install_loader(appdir, libdir, syslib, glibc_dir)


# --- wiring -----------------------------------------------------------------


def set_runpaths_command(here: Path, appdir: Path, loader_dir: str) -> list[str]:
    """`uv run --no-project set_runpaths.py <AppDir> [<loader_dir>/lib]`."""
    command = ["uv", "run", "--no-project", str(here / "set_runpaths.py"), str(appdir)]
    if loader_dir:
        command.append(f"{loader_dir}/lib")
    return command


def desktop_integration(
    appdir: Path, here: Path, app_id: str, gui_name: str, loader_dir: str, bundle: int
) -> None:
    """Expand AppRun's template and copy the .desktop/.png/.DirIcon."""
    loader = f"{loader_dir}/ld-linux-x86-64.so.2" if loader_dir else ""
    text = (here / "AppRun").read_text()
    for marker, value in (
        ("@LOADER@", loader),
        ("@BUNDLE_GLIBC@", str(bundle)),
        ("@OBS_GUI_NAME@", gui_name),
    ):
        text = sed_substitute(text, marker, value)
    app_run = appdir / "AppRun"
    app_run.write_text(text)
    app_run.chmod(app_run.stat().st_mode | 0o111)
    shutil.copy(appdir / "usr/share/applications" / f"{app_id}.desktop", appdir)
    shutil.copy(appdir / "usr/share/icons/hicolor/256x256/apps" / f"{app_id}.png", appdir)
    diricon = appdir / ".DirIcon"
    diricon.unlink(missing_ok=True)  # `ln -sf` replaces an existing entry
    diricon.symlink_to(f"{app_id}.png")


def final_checks(appdir: Path, here: Path, gui_name: str, bundle: int) -> None:
    """check_appdir.py, check-glibc-floor.py, then the ldd/du sanity output."""
    require(
        subprocess.run(
            ["uv", "run", "--no-project", str(here / "check_appdir.py"), str(appdir)]
        ).returncode
    )
    require(
        subprocess.run(
            [
                "uv",
                "run",
                "--script",
                str(here / "check-glibc-floor.py"),
                str(appdir),
                GLIBC_FLOOR,
            ]
        ).returncode
    )
    print("::group::Libraries still resolved from the host")
    gui = appdir / "usr/bin" / gui_name
    if bundle == 1:
        # (the binary's interpreter now points at the not-yet-installed loader,
        # so ask the bundled one)
        _, out, _ = capture(
            [
                str(appdir / "lib64/ld-linux-x86-64.so.2"),
                "--library-path",
                f"{appdir / 'glibc'}:{appdir / 'usr/lib'}",
                "--list",
                str(gui),
            ]
        )
    else:
        _, out, _ = capture(["ldd", str(gui)])
    for line in out.splitlines():
        if str(appdir) not in line and "=> /" in line:
            print(line)
    print("::endgroup::")
    _, du, _ = capture(["du", "-sh", str(appdir)])
    sys.stdout.write(du)


def assemble(appdir: Path, compat_lib: Path, here: Path) -> None:
    """The script body after the compatibility-library checks, in shell order."""
    libdir = appdir / "usr/lib"
    gui_name, app_id, data_root, module_root = gui_selection(appdir, libdir)
    syslib = system_libdir()
    bundle = decide_bundle_glibc()
    exclude = re.compile(GLIBC_EXCLUDE if bundle == 0 else HOST_EXCLUDE)
    libdir.mkdir(parents=True, exist_ok=True)

    bundle_qt_plugins(appdir, libdir, syslib, os.environ.get("OBS_DEPS", DEFAULT_OBS_DEPS))
    # Empty on purpose: GIO modules from the host are linked against the host's GLib.
    (libdir / "gio/modules").mkdir(parents=True, exist_ok=True)
    bundle_runtime(libdir, syslib, bundle)
    record_modules(data_root, module_root)

    # Install the source-built version first, so closure discovery uses it and
    # cannot replace it with the build host's library requiring a newer VA API.
    require(capture(["install", "-m", "644", str(compat_lib), str(libdir / "libavutil.so.60")])[0])
    # Prepend: on the glibc 2.28 base the prebuilt libraries (/opt/obs-deps/<tool>/lib)
    # are only on the image's LD_LIBRARY_PATH, and the closure must find them to bundle them.
    inherited = os.environ.get("LD_LIBRARY_PATH", "")
    os.environ["LD_LIBRARY_PATH"] = str(libdir) + (f":{inherited}" if inherited else "")
    walk_closure(appdir, libdir, exclude)

    # host libraries: shims for optional ones, the preflight for required ones (#959)
    # After the closure (the shims replace DT_NEEDED entries it found) and before
    # the RPATHs are set (set-runpaths then covers the shims and obs-hostlib-check).
    require(subprocess.run(["uv", "run", "--no-project", str(here / "hostlib/build.py"), str(appdir)]).returncode)

    loader_dir = bundle_glibc_section(appdir, libdir, syslib, bundle)

    # relative RPATHs instead of an exported LD_LIBRARY_PATH (see set_runpaths.py)
    del os.environ["LD_LIBRARY_PATH"]  # patchelf and friends are host programs
    require(subprocess.run(set_runpaths_command(here, appdir, loader_dir)).returncode)

    desktop_integration(appdir, here, app_id, gui_name, loader_dir, bundle)
    final_checks(appdir, here, gui_name, bundle)


def run(argv: list[str]) -> int:
    """`<AppDir> <validated-libavutil.so.60>`; AppDir already holds usr/."""
    compat_lib = Path(argv[1]) if len(argv) > 1 else None
    if compat_lib is None or not compat_lib.is_file():
        print(
            "::error::provide a validated VA-compatible libavutil (build.py's AVUTIL_LIB)",
            file=sys.stderr,
        )
        raise Failure(2)
    validate_compat(compat_lib)
    assemble(Path(argv[0]).resolve(), compat_lib, HERE)
    return 0


def main(argv: list[str]) -> int:
    try:
        return run(argv)
    except Failure as failure:
        return failure.code


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
