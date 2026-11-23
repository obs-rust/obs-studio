//! The libobs version getters of `libobs/obs-version.c.h`.
//!
//! The numbers are read at compile time from `libobs/obs-config.h`, the
//! public header the C takes `LIBOBS_API_VER` from, so a version bump there
//! reaches the Rust without a second edit.

/// `libobs/obs-config.h`, the one source of the API version numbers.
pub const OBS_CONFIG_H: &str = include_str!("../../../libobs/obs-config.h");

/// `MAKE_SEMANTIC_VERSION(major, minor, patch)`: `major << 24 | minor << 16 | patch`.
#[must_use]
pub const fn make_semantic_version(major: u32, minor: u32, patch: u32) -> u32 {
    let _ = (major, minor, patch);
    todo!()
}

/// `LIBOBS_API_MAJOR_VER` from `obs-config.h`.
pub const LIBOBS_API_MAJOR_VER: u32 = 0;
/// `LIBOBS_API_MINOR_VER` from `obs-config.h`.
pub const LIBOBS_API_MINOR_VER: u32 = 0;
/// `LIBOBS_API_PATCH_VER` from `obs-config.h`.
pub const LIBOBS_API_PATCH_VER: u32 = 0;

/// `LIBOBS_API_VER`, what `obs_get_version()` returns.
pub const LIBOBS_API_VER: u32 = 0;

/// The value of the line `#define <name> <decimal>` in `header`, or `None`
/// when there is no such line or its value is not a decimal `u32`.
#[must_use]
pub const fn define_value(header: &str, name: &str) -> Option<u32> {
    let _ = (header, name);
    None
}
