# Windows Tier 2 Linux producer probe

Branch-only infrastructure prototype. No workflow consumes this graph, and its
four-test inventory does not qualify a complete Windows Tier 2 run-only job.
Linux builds every artifact; there is no native Windows compilation fallback.

```sh
cmake -S cmake/cross -B build_cross_win-tier2 -G Ninja \
  -DOBS_CROSS_TARGET=win-x64 -DOBS_CROSS_WINDOWS_TIER2=ON
SOLDR_RUSTC_WRAPPER=none cmake --build build_cross_win-tier2 \
  --target tier2-cross-probe -j 6
uv run --no-project --python 3.14 -m unittest discover \
  -s ci -p test_windows_tier2_probe.py
```

The graph builds the original C and hybrid `obs.dll` separately, the real
w32-pthreads DLL, pinned cmocka, generated core/version objects, the native
Windows VERSIONINFO template, and the canonical PCH/unity source groups.
Three unchanged core tests (`test_bitstream`, `test_darray`, `test_serializer`)
and one Rust-static test (`test_async_conversion_plan_rust`) consume their
complete existing records from `test/cmocka/build/test-graph.json`.
Cargo checks source freshness every build. No C tests or public headers change.

The opt-in producer provisions soldr-toolchain's published
`compiler-rt/21.1.5/windows-x64/bundle.tar.zst` unit (generation
`source-d9139e614bd7-p5`). `ci/windows_tier2_toolchain.py` pins the archive and
builtins SHA256, retains the LLVM license, and creates a build-local resource
overlay preserving the managed resource headers/support files. It never modifies
the shared LLVM installation. Clang config options precede the soldr driver's
xwin includes, putting managed intrinsic headers first. The same config selects
`--rtlib=compiler-rt` and `-resource-dir`. Because CMake's GNU Windows link rules
use `-nostdlib`, the C reference explicitly links the runtime path resolved by
clang's `-print-libgcc-file-name` with that config. Rust-static consumers retain
their own compiler-builtins, avoiding duplicate compiler-runtime definitions.
No OBS-local SSE shim is used by this producer. The original C reference never
links the Rust implementation. Both cores link the canonical zlib dependency.

cmocka uses real target link probes with the release CRT configuration. The
compile-only cross toolchain's static probes otherwise falsely report POSIX
functions such as `siglongjmp` as available. Its Windows DLL builds successfully
with these probes; dependency source files remain unchanged.

Soldr currently exposes the target's GNU-style clang driver. Registry requirement
facts select the Windows MSVC ABI in a local function, following obs-build's
target facts. This does not turn the compiler into clang-cl: the actual frontend
and compiler/linker paths are retained in `tier2-probe-toolchain.txt`. GNU-style
clang targeting `x86_64-pc-windows-msvc` is sufficient; clang-cl is not required.
Additional missing toolchain units belong in `zackees/soldr-toolchain`.

Stage only after all six runtime images have built. Supply LLVM's readobj and
directories containing the actual dependency DLLs. The hosted-runner mode records
exact external VC runtime imports instead of distributing Microsoft DLL bytes:

```sh
uv run ci/windows_tier2_probe.py stage --build build_cross_win-tier2 \
  --output build_cross_win-tier2-bundle --readobj /path/to/llvm-readobj \
  --dll-root build_cross_win-tier2 --dll-root .deps/obs-deps-2026-10-25-x64/bin \
  --hosted-vc-runtime
uv run ci/windows_tier2_probe.py verify --bundle build_cross_win-tier2-bundle
```

Staging checks nonempty identical COFF export sets, AMD64 PE32+ headers, normal
and delay imports, the transitive dependency DLL closure, and copied bytes.
It preserves both isolated core directories and CTest properties. Duplicate
ordinary dependency basenames with differing bytes, unresolved DLL imports,
changed files and path traversal fail. Reference/hybrid `obs.dll` variants are
intentional. Windows system/API-set imports are excluded. Without the explicit
hosted flag, missing Visual C++ DLLs fail staging as before. With it, only the
named release VC14 DLL allowlist can be external; debug runtimes and every other
unresolved import still fail. The staged closure and exports are checked again
before publishing the manifest.

The repository's release packager uses `InstallRequiredSystemLibraries` from a
native Visual Studio installation; it does not establish a public standalone
Microsoft DLL mirror license. Redistribution permission for the upstream local
recipe remains unsettled, so this prototype does not copy those bytes.
The actual [`windows-2025-vs2026` runner image](https://github.com/actions/runner-images/blob/main/images/windows/Windows2025-VS2026-Readme.md#microsoft-visual-c)
lists the x64 VC++ 2022 Minimum/Additional Runtime (14.51.36247 in the image
inspected on 2026-11-18). On Windows, `verify` requires every declared runtime
from `%SystemRoot%/System32`, verifies AMD64 PE headers and recursive imports,
and reports hashes of the installed bytes. No compiler or runtime installer is
invoked. Linux verification reports installed-runtime verification as pending;
it cannot prove that hosted binaries loaded. `--runtime-root` permits checking
an explicitly provisioned directory without copying it into the artifact.

The manifest always says `complete: false`, `coverage: probe-only`. `verify`
checks its files and materializes build/source/CMake root tokens in CTest
metadata. There is deliberately no full-suite run subcommand. Embedded source
and build paths are inventoried, not silently declared relocatable.

Before switching the existing Windows job, qualify the CI-pinned soldr and the
hosted runtime contract, build every native owner/renderer/test target, independently
compare same-commit CTest names/commands/properties, split runtime fixture and
native-contract builds into Linux preparation and Windows assertions, and run
all four entire CTest passes plus the native loader/resource negative controls
on Windows with compiler tools unavailable. Keep symbol parity, actual enabled
core modules, complete runtime resources and existing crash diagnostics.

The next slice expands the canonical registry and its owner/renderer closure,
then compares all names, commands and CTest properties with a clean, same-commit
native Windows configure. Keep all four full CTest passes (CMake C/hybrid and
Cargo C/hybrid). Runtime fixture builds, native contract probes and the bundled
obs-sys check need Linux preparation plus Windows assertion receipts before
activation. The consumer must fail on absent receipts and execute with compiler
tools unavailable. A successful four-test bundle is evidence for slice 1 only.
