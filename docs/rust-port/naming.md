# Naming

One name per thing, so releases, crates and tags can't be confused (#320).
`ci/test_naming.py` checks the crate rules in `./lint`.

## Product

- Display name: **OBS-Studio-Rust**. Slug: **`obs-studio-rust`**.
- Release titles, artifact names and the app tag `obs-studio-rust-vX.Y.Z[-pre]`
  use them. `--version` keeps the upstream OBS version ("based on OBS ...").
- The command-line client (an obs-websocket v5 client for streaming control)
  is part of the product: crate and binary `obs-studio-rust` in
  `rust/obs-studio-rust/`. It gets no tag family of its own: published
  binaries go on the app's `obs-studio-rust-v*` releases.

## Repository layout (#1051)

- **The upstream tree stays where upstream has it:** `libobs/`, `libobs-*/`,
  `frontend/`, `plugins/`, `shared/`, `deps/`, `cmake/`, `test/`. We still
  merge `upstream/master`, so C/C++ files are never moved to restructure;
  fixes and ports edit them in place. The only rename is in place, to the
  build-role names in [BUILD.md](../../BUILD.md) (`foo.c` to `foo.c.h`,
  same directory and stem, in a `git mv`-only commit), which merges and
  `git log --follow` still track. A path whose OS its name
  doesn't say gets an entry in `ci/platform_paths.py`, not a rename.
- **All Rust lives in the `rust/` Cargo workspace**, one crate per ported
  component (`rust/tools/` is excluded). The table below is where a port goes.
- **Host code in C** stays in the OS-named files upstream already has. Each
  libobs area has one host facade: `util/platform.h` for `libobs/util/`, and
  `libobs/obs-host/` for the libobs core, with one header per capability
  (`com.h`, `graphics-thread.h`, `shared-texture.h`, ...). A host that has
  the capability defines it in its own file (`obs-windows.c`, `obs-nix.c`,
  `obs-cocoa.m`). A host that lacks it includes `obs-host/noop/<capability>.hpp`
  at the end of its host file (`obs-nix.c.h`, ...; see [BUILD.md](../../BUILD.md)),
  so no `#ifdef` picks between them.
  This is the C form of a crate's `src/platform/mod.rs`.
- **When a C component is fully ported**, its C sources are deleted and its
  CMake target links the Rust staticlib. No moved copy of the C is kept in
  the product tree; the one exception is the differential-test oracle copy
  under `rust/obs-c-oracle/legacy/` that the
  [testing policy](testing-policy.md) (Tier 3) requires.
- **Later, not now:** once we stop merging upstream, the remaining C/C++
  (mostly `frontend/` and `plugins/`) may move under one root directory in a
  single mechanical PR (only `git mv` plus CMake, CI and docs paths), landed
  in a quiet window, under a neutral name (libobs is C, not C++). That gets
  its own issue.

| C/C++ component | Rust crate |
|---|---|
| `libobs/util/` | `rust/obs-util` |
| `libobs/graphics/` | `rust/obs-graphics` |
| `libobs/media-io/` | `rust/obs-media-io` |
| `libobs/obs-nal.c`, `obs-avc.c`, `obs-hevc.c`, `obs-av1.c` | `rust/obs-codec` |
| `libobs/obs-data.c` | `rust/obs-data` |
| `libobs/obs-properties.c` | `rust/obs-properties` |
| `libobs/obs-hotkey-name-map.c`, `obs-hotkey-bindings.c.h`, `obs-hotkey-translations.c.h`, `obs-hotkey-press.c.h`, `obs-nix-hotkey-state.c.h`, `obs-nix-portal-shortcut-ids.c.h` | `rust/obs-hotkey` |
| `libobs/callback/` | `rust/obs-callback` |
| `libobs/obs-missing-files.c.h` | `rust/obs-missing-files` |
| `libobs/obs-output-delay.c.h`, `obs-output-interleave.c.h`, `obs-output-caption-text.c.h` | `rust/obs-output` |
| `libobs/obs-scene*.c.h` | `rust/obs-scene` |
| `libobs/audio-monitoring/` | `rust/obs-audio-monitoring` |
| `libobs/obs-audio-controls.c.h` | `rust/obs-audio-controls` |
| `libobs/obs.c.h` and the units extracted from it (`obs-video-plan.c.h`) | `rust/obs-core` |
| `plugins/obs-nvenc/` (CUDA/NVENC loader) | `rust/obs-nvenc`, linked through `rust/obs-nvenc-plugin` |
| GPU acceleration status (frontend, #846) | `rust/obs-gpu-status`, linked through `rust/obs-gpu-status-frontend` |
| all libobs ports, as one C ABI drop-in | `rust/libobs-rust` |

A new libobs area gets a new `obs-<area>` row here in the PR that starts it.

## Rust crates (`rust/<dir>/`, directory name = package name)

| Kind | Name | Examples |
|---|---|---|
| libobs port | `obs-<area>`, named after the libobs directory | `obs-util`, `obs-graphics`, `obs-media-io`, `obs-codec` |
| libobs staticlib (C ABI drop-in) | `libobs-rust` | |
| Plugin staticlib | `obs-<plugin>-plugin` | `obs-nvenc-plugin` |
| Test oracle (dev-only) | `obs-c-oracle` | |
| Product binary | `obs-studio-rust` | |

No other crate takes a `-rust` suffix. A new tool is `obs-<tool>`.

## Tags

- OBS version tags (`X.Y.Z`, `X.Y.Z-betaN`, ...) come from upstream; every
  `git describe` that names an OBS version matches only those
  (`ci/test_describe_match.py`).
- `obs-studio-rust-v*` is the only tag family this fork adds. The retired
  `obs-stream-rust-v*` tag (0.1.0, the CLI's old name) stays for history; no
  new ones are cut.
