//! Aggregates every Rust port's C ABI shim into one staticlib for libobs.
//!
//! libobs must link exactly one Rust staticlib: each staticlib carries its
//! own copy of `std`, so linking two would duplicate symbols.

#![forbid(unsafe_code)]

pub use obs_util::ffi;
// Each port crate's ffi is re-exported so its no_mangle shims land in the staticlib.
pub use obs_audio_controls::ffi as audio_controls_ffi;
pub use obs_audio_monitoring::ffi as audio_monitoring_ffi;
pub use obs_callback::ffi as callback_ffi;
pub use obs_codec::ffi as codec_ffi;
pub use obs_core::ffi as core_ffi;
pub use obs_data::ffi as data_ffi;
pub use obs_graphics::ffi as graphics_ffi;
pub use obs_hotkey::ffi as hotkey_ffi;
pub use obs_media_io::ffi as media_io_ffi;
pub use obs_missing_files::ffi as missing_files_ffi;
pub use obs_output::ffi as output_ffi;
pub use obs_properties::ffi as properties_ffi;
pub use obs_scene::ffi as scene_ffi;
