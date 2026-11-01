//! Safe core of `libobs/audio-monitoring/osx/coreaudio-device-rules.c.h`:
//! the device rules of the CoreAudio monitoring device list
//! (`osx/coreaudio-enum-devices.c`).
//!
//! - `obs_enum_audio_monitoring_device()`: [`listed`];
//! - `enum_audio_devices()`: [`count`];
//! - `devices_match()`: [`is_default`] and [`names_match`];
//! - `get_desktop_default_id()` and `find_loopback_cb()`: [`is_virtual`],
//!   the negation of the C's `device_is_input()`.
//!
//! `coreaudio-enum-devices.c` keeps every AudioObject call, the CFString
//! conversions, every allocation and free, the logs and the callbacks, and
//! reads each value where it did. These keep no state and call nothing in
//! CoreAudio or libobs. Strings are their bytes without the NUL.
//!
//! Nothing here can panic on any input. One intentional difference from
//! the C: [`count`] takes the smaller of the two sizes, so a device that
//! went away between `enum_audio_devices()`'s two calls is not walked (the
//! C keeps the first size and reads ids the second call never wrote).

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// The virtual device names `device_is_input()` looks for in a uid, in the
/// C's order.
pub const VIRTUAL_NAMES: [&str; 8] = [
    "soundflower",
    "wavtap",
    "soundsiphon",
    "ishowu",
    "blackhole",
    "loopback",
    "groundcontrol",
    "vbcable",
];

/// Whether the device list shows a device whose output stream list is
/// `output_stream_bytes` long: always with inputs, else only a device with
/// output streams.
#[must_use]
pub fn listed(allow_inputs: bool, output_stream_bytes: u32) -> bool {
    allow_inputs || output_stream_bytes != 0
}

/// How many device ids of `id_size` bytes `enum_audio_devices()` walks,
/// given the size the first call reported (the array's size) and the size
/// the second call wrote: the ids both cover.
///
/// A zero `id_size`, which the C divides by, counts none; the caller passes
/// `sizeof(AudioDeviceID)`.
#[must_use]
pub fn count(first_size: u32, second_size: u32, id_size: usize) -> u32 {
    // The intentional difference: the C counts first_size alone.
    let bytes = usize::try_from(first_size.min(second_size)).unwrap_or(usize::MAX);
    let ids = bytes.checked_div(id_size).unwrap_or(0);
    // Never more than a u32 of bytes, so it fits.
    u32::try_from(ids).unwrap_or(u32::MAX)
}

/// Whether a monitoring device id is `"default"`, the default output.
#[must_use]
pub fn is_default(id: &[u8]) -> bool {
    crate::pulse_monitor::is_default(id)
}

/// Whether two devices whose names were looked up match: both names were
/// found and are equal. Devices are matched by name, not uid.
#[must_use]
pub fn names_match(name1: Option<&[u8]>, name2: Option<&[u8]>) -> bool {
    matches!((name1, name2), (Some(a), Some(b)) if a == b)
}

/// Whether a device uid names a virtual device: it contains one of
/// [`VIRTUAL_NAMES`], ignoring ASCII case.
///
/// Only ASCII case is folded, as libobs's `astrstri()` does with
/// `tolower()` in the C locale; bytes above 127 are compared as they are.
#[must_use]
pub fn is_virtual(uid: &[u8]) -> bool {
    VIRTUAL_NAMES
        .iter()
        .any(|name| contains_ignore_ascii_case(uid, name.as_bytes()))
}

/// `astrstri(haystack, needle) != NULL` in the C locale.
fn contains_ignore_ascii_case(haystack: &[u8], needle: &[u8]) -> bool {
    // astrstri() finds an empty needle anywhere, and windows(0) would panic.
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle))
}
