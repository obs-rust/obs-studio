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

The current installed toolchain does **not** complete this build: the C reference
links with unresolved `__udivti3` and SSE intrinsics such as `_mm_loadu_ps`,
`_mm_shuffle_ps` and `_mm_mul_ps`. Its C/PCH/unity and resource compiles complete.
These compiler support gaps belong in soldr-toolchain. No new in-repository
intrinsic shim, Rust-linked C reference, sanitizer disablement or native compiler
fallback is added. This is a diagnostic prototype, not a completed first slice.

cmocka uses real target link probes with the release CRT configuration. The
compile-only cross toolchain's static probes otherwise falsely report POSIX
functions such as `siglongjmp` as available. Its Windows DLL builds successfully
with these probes; dependency source files remain unchanged.

Soldr currently exposes the target's GNU-style clang driver. Registry requirement
facts select the Windows MSVC ABI in a local function, following obs-build's
target facts. This does not turn the compiler into clang-cl: the actual frontend
and compiler/linker paths are retained in `tier2-probe-toolchain.txt`. A supported
clang-cl driver with matching SDK argument syntax must come from
`zackees/soldr-toolchain`, as must missing target runtime DLLs or compiler-rt.

Stage only after all six runtime images have built. Supply LLVM's readobj and
directories containing the actual dependency DLLs, including the VC runtime:

```sh
uv run ci/windows_tier2_probe.py stage --build build_cross_win-tier2 \
  --output build_cross_win-tier2-bundle --readobj /path/to/llvm-readobj \
  --dll-root build_cross_win-tier2 --dll-root .deps/obs-deps-2026-10-25-x64/bin \
  --dll-root /path/to/provisioned-msvc-runtime
uv run ci/windows_tier2_probe.py verify --bundle build_cross_win-tier2-bundle
```

Staging checks nonempty identical COFF export sets, AMD64 PE32+ headers, normal
and delay imports, the transitive dependency DLL closure, and copied bytes.
It preserves both isolated core directories and CTest properties. Duplicate
ordinary dependency basenames with differing bytes, unresolved DLL imports,
changed files and path traversal fail. Reference/hybrid `obs.dll` variants are
intentional. Windows system/API-set imports are excluded; Visual C++ runtime
DLLs are required. The staged closure and exports are checked again before
publishing the manifest.

The manifest always says `complete: false`, `coverage: probe-only`. `verify`
checks its files and materializes build/source/CMake root tokens in CTest
metadata. There is deliberately no full-suite run subcommand. Embedded source
and build paths are inventoried, not silently declared relocatable.

Before switching the existing Windows job, provision the complete pinned
runtime/toolchain, build every native owner/renderer/test target, independently
compare same-commit CTest names/commands/properties, split runtime fixture and
native-contract builds into Linux preparation and Windows assertions, and run
all four entire CTest passes plus the native loader/resource negative controls
on Windows with compiler tools unavailable. Keep symbol parity, actual enabled
core modules, complete runtime resources and existing crash diagnostics.
