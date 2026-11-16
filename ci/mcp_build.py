#!/usr/bin/env -S uv run --script
"""Build the native MCP application without consuming protocol stdin/stdout."""
from pathlib import Path
from ci.host_platform import is_linux
import os
import json
import shlex
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def enter_nix_shell(soldr, env):
    env["OBS_MCP_SOLDR"] = soldr
    keep = ["OBS_MCP_SOLDR", "OBS_MCP_TARGET_DIR", "XDG_RUNTIME_DIR", "DISPLAY",
            "WAYLAND_DISPLAY", "XDG_SESSION_TYPE", "DBUS_SESSION_BUS_ADDRESS",
            "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "NIX_LD", "NIX_LD_LIBRARY_PATH",
            "OBS_MCP_GRAPHICS_MODULE", "OBS_MCP_MODULE_BIN", "OBS_MCP_MODULE_DATA",
            "OBS_MCP_DATA_PATH", "LIBGL_ALWAYS_SOFTWARE", "__EGL_VENDOR_LIBRARY_FILENAMES"]
    command = ["nix-shell", "--pure", str(ROOT / "ci/mcp-shell.nix")]
    for name in keep:
        command += ["--keep", name]
    command += ["--run", shlex.join([*shlex.split("uv run --no-project --python python3 --script"),
                 str(ROOT / "mcp"), *sys.argv[1:]])]
    os.execvpe(command[0], command, env)


def run_build(command, env):
    library = None
    with subprocess.Popen(command, cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
                          stdout=subprocess.PIPE, stderr=sys.stderr, text=True) as process:
        for line in process.stdout:
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                print(line, end="", file=sys.stderr)
                continue
            if message.get("reason") == "build-script-executed":
                for key, value in message.get("env", []):
                    if key == "OBS_SYS_LIBRARY":
                        library = Path(value)
            rendered = message.get("message", {}).get("rendered")
            if rendered:
                print(rendered, end="", file=sys.stderr)
        status = process.wait()
    return status, library


def build_native(soldr, env, target):
    common = [soldr, "cargo", "build", "--locked", "--manifest-path", str(ROOT / "Cargo.toml")]
    archive = target / "release" / ("libobs_rust.lib" if os.name == "nt" else "liblibobs_rust.a")
    binary_dir = target / "debug"
    commands = [common + ["--release", "-p", "libobs-rust"],
                common + ["-p", "obs-studio-rust", "--features", "bundled-native", "--message-format=json-render-diagnostics"]]
    for index, command in enumerate(commands):
        if index:
            env["OBS_BUILD_RUST_ARCHIVE"] = str(archive)
        if index and is_linux():
            flags = env.get("CARGO_ENCODED_RUSTFLAGS")
            if flags is None:
                flags = "\x1f".join(shlex.split(env.get("RUSTFLAGS", "")))
            env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(filter(None, [
                flags, "-Clink-arg=-Wl,-rpath," + str(binary_dir)]))
        status, library = run_build(command, env)
        if status:
            return status
        if index and is_linux():
            if library is None or not library.is_file():
                print("mcp: native build did not report its libobs library", file=sys.stderr)
                return 1
            # An attach client may rebuild while a listener maps the old inode.
            with tempfile.TemporaryDirectory(dir=binary_dir, prefix=".libobs-") as staging:
                staged = Path(staging) / library.name
                shutil.copy2(library, staged)
                os.replace(staged, binary_dir / library.name)
    return build_runtime(soldr, env, target, library)


def build_runtime(soldr, env, target, library):
    output = target / "runtime"
    request = target / "mcp-runtime-request.json"
    request.write_text(json.dumps({"root": str(ROOT), "core": str(library), "output": str(output)}))
    command = [soldr, "cargo", "run", "--locked", "--manifest-path", str(ROOT / "Cargo.toml"),
               "-p", "obs-build", "--bin", "obs-build", "--", "mcp-runtime", "--request", str(request)]
    status, _ = run_build(command, env)
    if status:
        return status
    manifest = json.loads((output / "manifest.json").read_text())
    for field, variable in (("graphics_module", "OBS_MCP_GRAPHICS_MODULE"),
                            ("module_bin", "OBS_MCP_MODULE_BIN"),
                            ("module_data", "OBS_MCP_MODULE_DATA"),
                            ("data_path", "OBS_MCP_DATA_PATH")):
        if not env.get(variable):
            env[variable] = manifest[field]
    return 0


def main() -> int:
    env = os.environ.copy()
    soldr = env.get("OBS_MCP_SOLDR") or shutil.which("soldr")
    if not soldr:
        print("mcp: install soldr first (see docs/ci-tooling.md)", file=sys.stderr)
        return 1
    if Path("/etc/NIXOS").exists() and env.get("OBS_MCP_NIX_SHELL") != "1":
        enter_nix_shell(soldr, env)
    # Outside target/: soldr refuses to touch target/ while a lock under it exists,
    # and .mcp.json starts this build in every session opened in the checkout.
    target = Path(env.get("OBS_MCP_TARGET_DIR", ROOT / "target-mcp")).resolve()
    env["CARGO_TARGET_DIR"] = str(target)
    env.setdefault("CARGO_BUILD_JOBS", "4")
    env.pop("OBS_BUILD_RUST_ARCHIVE", None)
    if env.get("OBS_MCP_NIX_SHELL") == "1":
        host = subprocess.check_output([soldr, "rustc", "-vV"], env=env, text=True)
        triple = next(line.removeprefix("host: ") for line in host.splitlines()
                      if line.startswith("host: "))
        linker = shutil.which(env.get("CC", "cc"))
        if linker:
            env["CARGO_TARGET_" + triple.upper().replace("-", "_") + "_LINKER"] = linker
    status = build_native(soldr, env, target)
    if status:
        return status
    binary = target / "debug" / ("obs-studio-rust.exe" if os.name == "nt" else "obs-studio-rust")
    args = sys.argv[1:]
    if args == ["--build"]:
        print(binary)
        return 0
    os.execve(binary, [str(binary), *(args or ["--mcp"])], env)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
