# `build-aux` folder

This folder contains:
- Various formatting scripts:
  - `run-swift-format` which formats Swift files
  - `format-manifest.py` which formats Flatpak manifest JSON files
- The Flatpak manifest used to build OBS Studio
- Files used for Steam packaging

C/C++ and CMake formatting are not here: the local gate's lint lane runs
`clang-format` and `gersemi` from pinned `uvx` tools
(`uv run ci/local_gate.py --lane lint`, documented in `AGENTS.md`).

## Packaging: version numbers

A build carries two version numbers. Distribution packages (AUR, deb, rpm, ...)
should set only the first one:

- `OBS_RUST_VERSION` is the OBS-Studio-Rust release, e.g. `0.1.0-beta.20261014`
  (`<MAJOR>.<MINOR>.<PATCH>[-<suffix>]`). Pass it as `-DOBS_RUST_VERSION=...` or
  as an environment variable. In a Rust build (`ENABLE_RUST_LIBOBS` on) it names
  the release in the startup log and the About dialog; when it is unset, they
  show the libobs-rust crate version.
- `OBS_VERSION_OVERRIDE` is the OBS version libobs reports to plugins
  (`obs_get_version_string()`). Plugins compare it against the OBS version they
  need: DistroAV, for example, refuses to work below 31.1.1. Leave it unset in a
  git checkout: CMake takes it from the upstream OBS tags with
  `git describe --tags --match '[0-9]*.[0-9]*.[0-9]*'` (33.0.0-beta6 plus the
  commit count on master today). Set it only for a source tree without `.git`,
  to the OBS version that `git describe` gives for that commit.

A release log starts with both: `OBS-Studio-Rust 0.1.0-beta.20261014 (based on
OBS 33.0.0-beta6-1098-g9e2325ea4)`. CMake stops with an error when
`OBS_VERSION_OVERRIDE` is older than the libobs API major version in
`libobs/obs-config.h` (`LIBOBS_API_MAJOR_VER`), because that is almost always
the OBS-Studio-Rust version passed in the wrong variable (#1087).


## Formatting scripts

### `run-swift-format`

This script allows to check the formatting and/or format of the Swift files and requires ZSH and `swift-format`.

Example of use:
```sh
./build-aux/run-swift-format
```

### `format-manifest.py`

This script allows to check the formatting and/or format of the Flatpak manifest and its modules.

Example of use:
```sh
uv run ./build-aux/format-manifest.py io.github.obs_rust.OBSStudioRust.json
```
