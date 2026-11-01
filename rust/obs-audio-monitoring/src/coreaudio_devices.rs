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

#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]
#![expect(clippy::todo, reason = "RED: the port is not written yet")]

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
    let _ = (allow_inputs, output_stream_bytes);
    todo!()
}

/// How many device ids of `id_size` bytes `enum_audio_devices()` walks,
/// given the size the first call reported (the array's size) and the size
/// the second call wrote: the ids both cover.
#[must_use]
pub fn count(first_size: u32, second_size: u32, id_size: usize) -> u32 {
    let _ = (first_size, second_size, id_size);
    todo!()
}

/// Whether a monitoring device id is `"default"`, the default output.
#[must_use]
pub fn is_default(id: &[u8]) -> bool {
    let _ = id;
    todo!()
}

/// Whether two devices whose names were looked up match: both names were
/// found and are equal. Devices are matched by name, not uid.
#[must_use]
pub fn names_match(name1: Option<&[u8]>, name2: Option<&[u8]>) -> bool {
    let _ = (name1, name2);
    todo!()
}

/// Whether a device uid names a virtual device: it contains one of
/// [`VIRTUAL_NAMES`], ignoring ASCII case.
#[must_use]
pub fn is_virtual(uid: &[u8]) -> bool {
    let _ = uid;
    todo!()
}
