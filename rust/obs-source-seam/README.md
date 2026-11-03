# obs-source-seam

The C ABI of the `obs-source` ports that call back into libobs through a
seam. Each module here implements a safe `obs-source` core's traits over a
private libobs adapter and exports the C functions under their original names.

| C file | Safe core | Shims | Seam |
|---|---|---|---|
| `libobs/obs-source-transition-settings.c.h` | `obs_source::transition_settings` | `ffi::source_transition_settings` | `libobs/obs-source-transition-seam.h` |
| `libobs/obs-source-transition-slots.c.h` | `obs_source::transition_slots` | `ffi::source_transition_slots` | `libobs/obs-source-transition-seam.h`, and the libobs source calls through `Calls` |

## Why a crate of its own

The seam's functions are hidden in libobs (not `EXPORT`). The
`test_*_steps_rust` C tests link libobs and the Rust staticlib on their own,
and the linker pulls in each `obs-source` object they use. A shim that calls
the seam, compiled into one of those objects, leaves its seam calls
unresolved there, and its `obs_data_*()` calls (resolved to the Rust
`obs-data` inside the staticlib) pull in most of the staticlib and its system
libraries. Only `libobs-rust` links this crate, so those tests never load it,
and the `obs-source` cores stay free of calls into libobs.

## Tests

The Tier 1 tests of each core are in `rust/obs-source/tests/source/`; the
Tier 3 parity tests of these shims are in `tests/source_seam/`, a module of
the Core state test binary (`obs-tests-core`, in `rust/obs-data/tests/core.rs`).
