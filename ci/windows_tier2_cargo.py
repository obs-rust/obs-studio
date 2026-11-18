#!/usr/bin/env -S uv run --script
"""Linux preparation of the native Windows Tier 2 Cargo products.

OBS_TIER2_SOLDR can select an unreleased soldr with the #4169 clang-cl API.
No Windows executable is run by this producer.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import time

TARGET = "x86_64-pc-windows-msvc"
SELECTOR = "native_libobs::windows_tests::core_builds_from_the_canonical_manifest"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def toolchain(soldr: str, output: Path) -> tuple[dict, dict]:
    """Ask the selected soldr for the managed frontend; never guess SDK paths."""
    executable = shutil.which(soldr)
    if not executable:
        raise ValueError(f"soldr executable unavailable: {soldr}")
    executable = str(Path(executable).absolute())
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    tools = {}
    for name in ("cc", "ar", "rc", "sysroot"):
        command = [
            executable,
            "cc",
            "--target",
            TARGET,
            "--driver-mode=cl",
            f"--print-{name}",
        ]
        tools[name] = Path(subprocess.check_output(command, text=True).strip())
        if not tools[name].exists():
            raise ValueError(f"soldr returned missing {name}: {tools[name]}")
    if not tools["cc"].name.endswith("clang-cl"):
        raise ValueError("require soldr's real targeted clang-cl driver (soldr#4169)")
    # llvm-rc is a real resource compiler. Supply its explicit SDK include
    # directories without changing the managed installation or a global INCLUDE.
    sdk = tools["sysroot"]
    includes = [
        sdk / "crt/include",
        *(sdk / f"sdk/include/{part}" for part in ("ucrt", "shared", "um")),
    ]
    rc = output / "windows-rc"
    command = [
        str(tools["rc"]),
        *(arg for path in includes for arg in ("/I", str(path))),
    ]
    rc.write_text("#!/bin/sh\nexec " + shlex.join(command) + ' "$@"\n')
    rc.chmod(0o755)
    env = {
        "CC_x86_64_pc_windows_msvc": str(tools["cc"]),
        "CXX_x86_64_pc_windows_msvc": str(tools["cc"]),
        "AR_x86_64_pc_windows_msvc": str(tools["ar"]),
        "RC_x86_64_pc_windows_msvc": str(rc),
        "SOLDR_RUSTC_WRAPPER": "none",
        "SOLDR_PORCELAIN": "0",
    }
    receipt = {
        "target": TARGET,
        "frontend": "clang-cl",
        "soldr": executable,
        "soldr_sha256": digest(Path(executable)),
        "tools": {name: str(path) for name, path in tools.items()},
    }
    (output / "toolchain.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return env, receipt


def run(command: list[str], source: Path, output: Path, env: dict, name: str) -> dict:
    """Keep full diagnostics and the real status; no pipe can hide failure."""
    started = time.monotonic()
    with (output / f"{name}.log").open("w") as log:
        result = subprocess.run(
            command,
            cwd=source,
            env=env,
            stdout=log,
            stderr=subprocess.STDOUT,
            timeout=3600,
            check=False,
        )
    receipt = {
        "command": command,
        "exit_code": result.returncode,
        "seconds": time.monotonic() - started,
    }
    (output / f"{name}.json").write_text(json.dumps(receipt, indent=2) + "\n")
    if result.returncode:
        raise RuntimeError(
            f"{name} failed ({result.returncode}); see {output / f'{name}.log'}"
        )
    return receipt


def produce(source: Path, build: Path, output: Path, dependencies: Path) -> dict:
    source, build, output, dependencies = (
        p.resolve() for p in (source, build, output, dependencies)
    )
    selected, tools = toolchain(
        os.environ.get("OBS_TIER2_SOLDR", "soldr"), output / "toolchain"
    )
    soldr = tools["soldr"]
    env = {**os.environ, **selected}
    # The CMake half uses the same remapping at compile time. Keep source/home
    # roots out of diagnostics and runtime __FILE__ values without editing PEs.
    flags = [
        f"/clang:-ffile-prefix-map={source}=source",
        f"/clang:-ffile-prefix-map={output}=build/cargo",
        f"/clang:-ffile-prefix-map={Path.home()}=producer",
    ]
    for name in ("CFLAGS_x86_64_pc_windows_msvc", "CXXFLAGS_x86_64_pc_windows_msvc"):
        env[name] = shlex.join(flags)
    env["CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS"] = shlex.join(
        [
            f"--remap-path-prefix={source}=source",
            f"--remap-path-prefix={Path.home()}=producer",
        ]
    )
    archive_dir = output / "rust"
    archive = archive_dir / TARGET / "release/libobs_rust.lib"
    receipt = {
        "target": TARGET,
        "source_sha": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=source, text=True
        ).strip(),
        "steps": {},
    }
    steps = receipt["steps"]
    steps["rust-archive"] = run(
        [
            soldr,
            "cargo",
            "build",
            "--locked",
            "-p",
            "libobs-rust",
            "--release",
            "--target",
            TARGET,
            "--target-dir",
            str(archive_dir),
        ],
        source,
        output,
        env,
        "rust-archive",
    )
    core = output / "core"
    env.update(
        {
            "OBS_BUILD_WINDOWS_DEPENDENCIES": str(dependencies),
            "OBS_BUILD_WINDOWS_TARGET": TARGET,
            "OBS_BUILD_WINDOWS_OUTPUT": str(core),
            "OBS_BUILD_WINDOWS_RUST_ARCHIVE": str(archive),
            "OBS_BUILD_WINDOWS_PCH": "1",
            "OBS_BUILD_WINDOWS_REFERENCE": str(build / "cores/reference/obs.dll"),
            "OBS_BUILD_WINDOWS_EXPORT_TOOL": shutil.which("llvm-readobj")
            or str(Path(tools["tools"]["ar"]).with_name("llvm-readobj")),
            "OBS_BUILD_WINDOWS_GRAPHICS": "1",
            "OBS_BUILD_WINDOWS_D3D11_REFERENCE": str(build / "libobs-d3d11.dll"),
            "OBS_BUILD_WINDOWS_OPENGL_REFERENCE": str(build / "libobs-opengl.dll"),
        }
    )
    # The existing assertion test runs on Linux and requests the explicit target
    # from NativeBuild. It compiles resources/probes but never executes a PE.
    steps["native-contract"] = run(
        [
            soldr,
            "cargo",
            "test",
            "--locked",
            "-p",
            "obs-studio-rust",
            "--features",
            "libobs-integration",
            "--test",
            "obs-tests-app",
            SELECTOR,
            "--",
            "--exact",
            "--nocapture",
        ],
        source,
        output,
        env,
        "native-contract",
    )
    env.update(
        {
            "OBS_SYS_NATIVE_DEPENDENCIES": str(dependencies),
            "OBS_BUILD_RUST_ARCHIVE": str(archive),
            "OBS_SYS_UNITY": "1",
            "OBS_SYS_PCH": "0",
        }
    )
    steps["bundled-obs-sys"] = run(
        [
            soldr,
            "cargo",
            "check",
            "--locked",
            "-p",
            "obs-sys",
            "--features",
            "bundled",
            "--target",
            TARGET,
            "--target-dir",
            str(output / "tests"),
        ],
        source,
        output,
        env,
        "bundled-obs-sys",
    )
    steps["application-test"] = run(
        [
            soldr,
            "cargo",
            "test",
            "--locked",
            "-p",
            "obs-studio-rust",
            "--features",
            "libobs-integration",
            "--test",
            "obs-tests-app",
            "--no-run",
            "--target",
            TARGET,
            "--target-dir",
            str(output / "tests"),
        ],
        source,
        output,
        env,
        "application-test",
    )
    (output / "cargo-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return receipt


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--source", type=Path, default=Path(__file__).resolve().parents[1]
    )
    parser.add_argument("--build", type=Path)
    parser.add_argument("--dependencies", type=Path)
    args = parser.parse_args()
    if args.build and args.dependencies:
        print(
            json.dumps(
                produce(args.source, args.build, args.output, args.dependencies),
                indent=2,
            )
        )
    else:
        env, receipt = toolchain(
            os.environ.get("OBS_TIER2_SOLDR", "soldr"), args.output
        )
        print(json.dumps({"environment": env, "receipt": receipt}, indent=2))


if __name__ == "__main__":
    main()
