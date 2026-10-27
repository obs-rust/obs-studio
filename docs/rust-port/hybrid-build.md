# Hybrid application build

The default `libobs` is one shared library containing completed Rust ports and
remaining legacy C/C++. Frontend, plugins, installation and packaging use it
through the unchanged `OBS::libobs` target and public C headers. Rust and its
pinned toolchain are required. There is no automatic C fallback.

Use the normal platform preset and build commands. `OBS_LIBOBS_BACKEND=HYBRID`
is the default. `OBS_BRAND_RUST=ON` selects OBS-Studio-Rust identity independently
of implementation; the deprecated `ENABLE_RUST_LIBOBS` input supplies the initial
branding default for existing callers. For an existing build cache, set branding
explicitly when changing it.

## Compatibility tests and temporary rollback

`./unittest` retains Rust unit/differential coverage and the unchanged C tests
against both implementations. The Tier 2 harness explicitly configures:

```sh
cmake --preset ubuntu-ci -DOBS_BUILD_C_REFERENCE=ON
cmake --build build_ubuntu --target libobs libobs-c-reference cmocka-tests
```

The C reference is excluded from `all` and never installed. It has the same
filename and ABI identity in `build_ubuntu/libobs-c-reference`; the test harness
selects that directory for `OFF`, and production hybrid libobs for `ON`, verifying
which library the loader actually chose. Unported objects are shared, and both
complete test suites and export comparisons remain required on supported hosts.

Temporary first-release rollback uses a separate build directory configured with
`-DOBS_LIBOBS_BACKEND=C`. It builds and installs the original C implementation
without the libobs Rust archive. This is an explicit recovery path, not a second
normal application configuration. Keep it until the release retirement criteria
are met. Other Rust features may still require Rust tooling.

## Cargo native build (#1332)

The Cargo build engine uses the same `libobs/build/source-sets.json` as CMake.
Build `libobs-rust` externally in release mode, then supply its archive through
`OBS_BUILD_RUST_ARCHIVE` to normal `obs-build`/bundled `obs-sys` builds. A nested
Cargo build inside a build script would deadlock on Cargo's build lock.
`build_hybrid_with_options` accepts the archive directly; `build_pair` explicitly
builds both variants for compatibility tests. `build_c_reference_with_options`
and bundled `OBS_SYS_C_ROLLBACK=1` are explicit reference/rollback entry points.

## Flatpak and packaged validation

Before flatpak-builder, run `build-aux/prepare_flatpak_rust.py` to vendor the
locked Cargo inputs. The existing workflow does this before entering the offline
sandbox. The manifest supplies checksum-pinned Rust and Corrosion, and retains
Flatpak's existing application identity through `OBS_BRAND_RUST=OFF`.

Run packaged artifacts with saved profiles, screen capture, camera and microphone
as required by AGENTS.md. `test-appimage.sh` also checks that bundled libobs
contains Rust code; branding alone cannot prove which implementation shipped.
Keep the existing C++ obs-websocket plugin unchanged and run its Tier 4 suite
against the real hybrid application.

## Migrating another unit

Follow testing-policy.md: claim, characterize if needed, RED, GREEN, ABI/layout
and differential coverage. Move the original C include into a ported group in
the canonical manifest; link its Rust shim through `libobs-rust`. Confirm the
production build excludes that C implementation and the explicit reference still
includes it. Preserve C oracle sources and all required tiers. Remove originals
only under release-criteria.md after the rollback retirement gates pass.

Nightlies publish the hybrid build automatically; a public release happens
when the maintainer dispatches certify and the automated CI gates pass (see
release-criteria.md, #1818).
