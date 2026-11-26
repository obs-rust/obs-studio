# Flatpak compiler cache migration

The OBS manifest targets `org.freedesktop.Sdk//26.08`. The existing
[SDK PCH qualification](ci-speed-flatpak-pch-1760.md) records GCC 16.2.0 and
ccache 4.14 from the production SDK. The SDK executable is
`/usr/bin/ccache`, inside the SDK's `/usr`. The workflow queries that
executable via `flatpak run --command=ccache`. Phase 1 did not inspect a
live SDK checkout or establish whether it contains zccache.

The [pinned GitHub action](https://github.com/flatpak/flatpak-github-actions/blob/79327416609af08178ad73b352877e51450790b3/flatpak-builder/index.js#L234)
adds `--ccache` for `cache: true`, independently of Actions cache restore
and save. Flatpak Builder creates compiler-name links to `/usr/bin/ccache`,
mounts its compiler cache under `/run/ccache`, prepends `/run/ccache/bin`
to PATH, and sets `CCACHE_DIR=/run/ccache`. The host state lives under
`.flatpak-builder/ccache`. See the upstream
[environment setup](https://github.com/flatpak/flatpak-builder/blob/1.4.8/src/builder-context.c#L1081-L1103)
and [build sandbox mount](https://github.com/flatpak/flatpak-builder/blob/1.4.8/src/builder-module.c#L1185-L1204).

The OBS module explicitly chooses `/usr/bin/cc` and `/usr/bin/c++`, so
it bypasses the compiler-name links. Its `-DENABLE_CCACHE=ON` relies on
`cmake/common/ccache.cmake` and the OBS PCH helper for the validated
compiler-launcher path. Remove those only alongside the manifest switch.
`ci/flatpak_cache_probe.py` isolates builder state by category.
`ci/flatpak_compiler_cache.py` fingerprints the SDK and its compilers,
checks the generated links, transfers compiler state, and queries stats.
`ci/flatpak_compiler_diagnostics.py` configures and summarizes ccache
debug logs. All three depend on the current layout or reporting contract.

## Options

1. **Build-only SDK or manifest module.** Install a checksum-pinned,
   architecture-specific zccache release ahead of the OBS module. Use an
   existing SDK extension only after verifying its contents and provenance.
   A manifest module can stage the binary under `/app`; arrange final
   cleanup so the tool is excluded from the application artifact. This
   adds source pins and per-architecture qualification, with a reproducible
   origin in offline builds.
2. **Host binary exposed with `flatpak run --filesystem`.** Grant read-only
   access to a pinned host binary and separate write access to a cache
   directory for an SDK experiment. The permission applies to that
   invocation; the builder starts its own module sandboxes and resets host
   filesystem access. A production integration would need explicit build
   sandbox access, and must prove architecture, loader compatibility, and
   daemon/cache access. This adds runner-specific setup and permissions.
3. **Launcher staged inside the OBS build.** Provide the verified archive
   as an OBS module source, unpack it under the mounted module build tree,
   and configure C and C++ launchers to that sandbox-visible absolute path.
   This avoids a new SDK and keeps the tool out of the installed files,
   but requires lifecycle wiring for a persistent cache directory, with
   additional wiring if other modules are to share the launcher. A CMake
   launcher setting alone supplies no executable.

## Recommendation

Prototype option 3 for the OBS module first. Disable the builder's
compiler-name wrappers in the experiment and measure any lost reuse in
other modules before changing the production action. Use option 1 if
several modules need the same tool. Keep option 2 for SDK diagnostics.

Validate the pinned binary inside the exact SDK commit on each shipping
architecture before changing transport. Run cold and warm builds with
isolated counters, flush the daemon before archiving, and verify restored
hits. Repeat the existing PCH freshness checks, including same-size,
same-timestamp edits. Preserve the SDK/compiler cache identity checks and
verify the packaged application using the saved profile required by
`AGENTS.md`.
