#!/usr/bin/env -S uv run --no-project --script
"""Build, sign and round-trip the Windows BETA MSI installer (#3476).

    uv run --no-project ci/windows_msi.py build --build-dir build_x64 --config RelWithDebInfo
    uv run --no-project ci/windows_msi.py sign --msi pkg/OBS-Studio-Rust-...-windows-x64.msi
    uv run --no-project ci/windows_msi.py roundtrip --msi ... [--require-signature]

`build` installs the pinned WiX .NET tool and its UI extension, then runs
`cpack -G WIX` in an already built and packaged build directory, so the MSI
holds the same install tree as the portable ZIP (minus the PDBs, see
cmake/windows/cpack-wix-generator.cmake). It prints the MSI path.

`sign` signs the MSI with signtool when `WINDOWS_SIGNING_PFX_BASE64` (and
`WINDOWS_SIGNING_PFX_PASSWORD`) are set; without them it leaves the MSI
unsigned and says so. It writes `signed=true|false` to `GITHUB_OUTPUT`.

`roundtrip` is what a user does: a silent per-machine install into a chosen
directory (`INSTALL_ROOT`, the directory the installer UI lets you pick),
then checks obs64.exe, the Start menu shortcut and the BETA entry in
Add/Remove Programs; then a silent uninstall and checks all three are gone.
It checks `Get-AuthenticodeSignature` first: `Valid` is required with
`--require-signature`, otherwise an unsigned MSI is reported as a warning.
"""

from __future__ import annotations

import argparse
import base64
import os
from pathlib import Path
import subprocess
import sys
import tempfile

WIX_VERSION = "4.0.6"
PRODUCT_NAME = "OBS-Studio-Rust BETA"
EXECUTABLE = Path("bin/64bit/obs64.exe")
TIMESTAMP_URL = "http://timestamp.digicert.com"


def run(command: list[str], cwd: Path | None = None) -> int:
    print("+ " + " ".join(command), flush=True)
    return subprocess.run(command, cwd=cwd, check=False).returncode


def github_output(key: str, value: str) -> None:
    path = os.environ.get("GITHUB_OUTPUT")
    if path:
        with open(path, "a", encoding="utf-8") as output:
            output.write(f"{key}={value}\n")


def find_msi(build_dir: Path) -> Path:
    found = sorted(build_dir.glob("*.msi"))
    if len(found) != 1:
        raise SystemExit(
            f"expected one MSI in {build_dir}, found {[p.name for p in found]}"
        )
    return found[0]


def build(arguments: argparse.Namespace) -> int:
    build_dir = Path(arguments.build_dir)
    for command in (
        ["dotnet", "tool", "update", "--global", "wix", "--version", WIX_VERSION],
        ["wix", "extension", "add", "--global", f"WixToolset.UI.wixext/{WIX_VERSION}"],
    ):
        if run(command) != 0:
            return 1
    for stale in build_dir.glob("*.msi"):
        stale.unlink()
    if run(["cpack", "-G", "WIX", "-C", arguments.config], cwd=build_dir) != 0:
        return 1
    print(find_msi(build_dir))
    return 0


def signtool() -> str:
    configured = os.environ.get("SIGNTOOL")
    if configured:
        return configured
    kits = Path("C:/Program Files (x86)/Windows Kits/10/bin")
    tools = sorted(kits.glob("10.*/x64/signtool.exe"))
    if not tools:
        raise SystemExit("signtool.exe not found in the Windows SDK")
    return str(tools[-1])


def sign(arguments: argparse.Namespace) -> int:
    pfx = os.environ.get("WINDOWS_SIGNING_PFX_BASE64", "").strip()
    if not pfx:
        print(
            "::warning::WINDOWS_SIGNING_PFX_BASE64 is not set; the MSI stays unsigned"
        )
        github_output("signed", "false")
        return 0
    with tempfile.TemporaryDirectory() as directory:
        certificate = Path(directory) / "signing.pfx"
        certificate.write_bytes(base64.b64decode(pfx))
        status = run(
            [
                signtool(),
                "sign",
                "/fd",
                "sha256",
                "/tr",
                TIMESTAMP_URL,
                "/td",
                "sha256",
                "/d",
                PRODUCT_NAME,
                "/f",
                str(certificate),
                "/p",
                os.environ.get("WINDOWS_SIGNING_PFX_PASSWORD", ""),
                arguments.msi,
            ]
        )
    if status != 0:
        return 1
    github_output("signed", "true")
    return 0


def signature_status(msi: Path) -> str:
    literal = str(msi).replace("'", "''")
    result = subprocess.run(
        [
            "powershell",
            "-NoProfile",
            "-Command",
            "$ErrorActionPreference = 'Stop'; "
            f"(Get-AuthenticodeSignature -LiteralPath '{literal}' -ErrorAction Stop)"
            ".Status.ToString()",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode:
        raise RuntimeError(
            f"Get-AuthenticodeSignature failed for {msi} "
            f"(PowerShell exit code {result.returncode}); "
            f"stderr: {result.stderr.strip() or '<empty>'}; "
            f"stdout: {result.stdout.strip() or '<empty>'}"
        )
    return result.stdout.strip()


def check_signature(status: str, required: bool) -> str | None:
    """None when acceptable, else the error. Unsigned is a warning unless required."""

    if status == "Valid":
        return None
    if status == "NotSigned" and not required:
        print("::warning::the MSI is not Authenticode signed")
        return None
    return f"Get-AuthenticodeSignature returned {status}, want Valid"


def start_menu_shortcut() -> Path:
    programs = (
        Path(os.environ.get("ProgramData", "C:/ProgramData"))
        / "Microsoft/Windows/Start Menu/Programs"
    )
    return programs / PRODUCT_NAME / f"{PRODUCT_NAME}.lnk"


def uninstall_entries() -> list[str]:
    import winreg  # Windows only

    names = []
    for view in (winreg.KEY_WOW64_64KEY, winreg.KEY_WOW64_32KEY):
        path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"
        with winreg.OpenKey(
            winreg.HKEY_LOCAL_MACHINE, path, 0, winreg.KEY_READ | view
        ) as root:
            for index in range(winreg.QueryInfoKey(root)[0]):
                with winreg.OpenKey(root, winreg.EnumKey(root, index)) as key:
                    try:
                        names.append(str(winreg.QueryValueEx(key, "DisplayName")[0]))
                    except OSError:
                        pass
    return names


def _install(msi: Path, root: Path, logs: Path) -> int:
    return run(
        [
            "msiexec",
            "/i",
            str(msi),
            "/qn",
            "/norestart",
            f"INSTALL_ROOT={root}",
            "/l*v",
            str(logs / "install.log"),
        ]
    )


def _uninstall(msi: Path, logs: Path) -> int:
    return run(
        [
            "msiexec",
            "/x",
            str(msi),
            "/qn",
            "/norestart",
            "/l*v",
            str(logs / "uninstall.log"),
        ]
    )


def _install_failures(root: Path) -> list[str]:
    failures = []
    if not (root / EXECUTABLE).is_file():
        failures.append(f"{root / EXECUTABLE} missing after install")
    if not start_menu_shortcut().is_file():
        failures.append(
            f"Start menu shortcut {start_menu_shortcut()} missing after install"
        )
    if PRODUCT_NAME not in uninstall_entries():
        failures.append(f'"{PRODUCT_NAME}" not listed in Add/Remove Programs')
    return failures


def _uninstall_failures(root: Path) -> list[str]:
    failures = []
    if (root / EXECUTABLE).exists():
        failures.append(f"{root / EXECUTABLE} still present after uninstall")
    if root.exists() and any(root.rglob("*")):
        failures.append(f"{root} not empty after uninstall")
    if start_menu_shortcut().exists():
        failures.append("Start menu shortcut still present after uninstall")
    if PRODUCT_NAME in uninstall_entries():
        failures.append("Add/Remove Programs entry still present after uninstall")
    return failures


def roundtrip(arguments: argparse.Namespace) -> int:
    msi = Path(arguments.msi).resolve()
    try:
        status = signature_status(msi)
    except RuntimeError as error:
        print(f"::error::{error}")
        return 1
    error = check_signature(status, arguments.require_signature)
    if error:
        print(f"::error::{error}")
        return 1

    root = Path(tempfile.mkdtemp(prefix="obs-msi-")) / "Chosen Dir"
    logs = Path(arguments.log_dir).resolve()
    logs.mkdir(parents=True, exist_ok=True)

    failures = []
    if _install(msi, root, logs) != 0:
        failures.append("silent MSI install failed")
    else:
        failures += _install_failures(root)

    if _uninstall(msi, logs) != 0:
        failures.append("silent MSI uninstall failed")
    else:
        failures += _uninstall_failures(root)

    for failure in failures:
        print(f"::error::{failure}")
    if not failures:
        print(f"MSI install/uninstall round trip passed into {root}")
    return 1 if failures else 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)

    build_parser = commands.add_parser("build", help="cpack -G WIX the BETA MSI")
    build_parser.add_argument("--build-dir", required=True)
    build_parser.add_argument("--config", default="RelWithDebInfo")
    build_parser.set_defaults(func=build)

    sign_parser = commands.add_parser("sign", help="signtool-sign the MSI")
    sign_parser.add_argument("--msi", required=True)
    sign_parser.set_defaults(func=sign)

    roundtrip_parser = commands.add_parser(
        "roundtrip", help="silent install, check, silent uninstall, check"
    )
    roundtrip_parser.add_argument("--msi", required=True)
    roundtrip_parser.add_argument("--log-dir", default="msi-logs")
    roundtrip_parser.add_argument("--require-signature", action="store_true")
    roundtrip_parser.set_defaults(func=roundtrip)

    arguments = parser.parse_args(argv)
    return arguments.func(arguments)


if __name__ == "__main__":
    sys.exit(main())
