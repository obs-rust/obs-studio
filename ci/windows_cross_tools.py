"""MSVC syntax adapters over soldr's discovered LLVM/xwin toolchain."""
import shlex
import subprocess
from pathlib import Path

TARGET = "x86_64-pc-windows-msvc"


def sdk_libraries(sdk: Path) -> list[Path]:
    paths = [sdk / "crt/lib/x86_64", *(sdk / "sdk/lib" / part / "x86_64" for part in ("um", "ucrt"))]
    for path in paths:
        if not path.is_dir():
            raise FileNotFoundError(path)
    return paths


def linker_driver(driver: str, root: Path, bundle: Path, env: dict) -> str:
    """Rustdoc requires link.exe syntax; soldr's printed driver speaks GNU."""
    sdk = subprocess.run(["soldr", "cc", "--target", TARGET, "--print-sysroot"],
                         cwd=root, env=env, text=True, capture_output=True, check=True).stdout.strip()
    linker = subprocess.run([driver, "-print-prog-name=lld-link"], cwd=root, env=env,
                            text=True, capture_output=True, check=True).stdout.strip()
    flags = [f"/libpath:{path}" for path in sdk_libraries(Path(sdk))]
    wrapper = bundle / "producer-tools/doctest-link"
    wrapper.parent.mkdir(parents=True, exist_ok=True)
    wrapper.write_text(f'#!/bin/sh\nexec {shlex.join([linker, *flags])} "$@"\n')
    wrapper.chmod(0o755)
    return str(wrapper)
