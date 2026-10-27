//! C ABI for `libobs/obs-canvas-rules.c.h`, the canvas flag rules and setter
//! checks, declared in `libobs/obs-canvas-rules.h` without `EXPORT`.
//!
//! `obs-canvas.c.h` (still C) reads the canvas's flags, its private flag,
//! the video info fields and the channel at each point, passes the value in
//! and acts on what comes back. So no canvas, view or mix is mirrored, and
//! these shims take plain values, take no lock and call nothing in libobs.
//! The one pointer is `obs_canvas_set_name()`'s name: its first byte is read
//! only when it is not NULL, as the C's `!name || !*name`. All seventeen stay
//! hidden (`rust-hidden.txt`).
//!
//! The video format is `enum video_format`, which the C passes as `int`.
//!
//! No intentional differences from C.

use core::ffi::{c_char, c_int};

use crate::canvas_rules as rules;

/// `uint32_t obs_canvas_rules_main_flags(void)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_main_flags() -> u32 {
    rules::main_flags()
}

/// `uint32_t obs_canvas_rules_public_flags(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_public_flags(flags: u32) -> u32 {
    rules::public_flags(flags)
}

/// `bool obs_canvas_rules_main_view(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_main_view(flags: u32) -> bool {
    rules::main_view(flags)
}

/// `bool obs_canvas_rules_mix_audio(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_mix_audio(flags: u32) -> bool {
    rules::mix_audio(flags)
}

/// `bool obs_canvas_rules_public(bool is_private)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_public(is_private: bool) -> bool {
    rules::is_public(is_private)
}

/// `bool obs_canvas_rules_holds_scenes(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_holds_scenes(flags: u32) -> bool {
    rules::holds_scenes(flags)
}

/// `bool obs_canvas_rules_saves(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_saves(flags: u32) -> bool {
    rules::saves(flags)
}

/// `bool obs_canvas_rules_size_set(uint32_t size)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_size_set(size: u32) -> bool {
    rules::size_set(size)
}

/// `bool obs_canvas_rules_format_set(int format)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_format_set(format: c_int) -> bool {
    rules::format_set(format)
}

/// `bool obs_canvas_rules_resets(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_resets(flags: u32) -> bool {
    rules::resets(flags)
}

/// `bool obs_canvas_rules_global_signals(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_global_signals(flags: u32) -> bool {
    rules::global_signals(flags)
}

/// `bool obs_canvas_rules_channel_ok(uint32_t channel)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_channel_ok(channel: u32) -> bool {
    rules::channel_ok(channel)
}

/// `bool obs_canvas_rules_name_empty(const char *name)`.
///
/// # Safety
///
/// `name` is NULL or points to at least one readable byte (a C string).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_canvas_rules_name_empty(name: *const c_char) -> bool {
    let first_byte = if name.is_null() {
        None
    } else {
        // SAFETY: the caller guarantees a C string, so its first byte is
        // readable.
        Some(unsafe { name.cast::<u8>().read() })
    };
    rules::name_empty(first_byte)
}

/// `bool obs_canvas_rules_renames(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_renames(flags: u32) -> bool {
    rules::renames(flags)
}

/// `bool obs_canvas_rules_removable(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_removable(flags: u32) -> bool {
    rules::removable(flags)
}

/// `uint32_t obs_canvas_rules_mark_removed(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_mark_removed(flags: u32) -> u32 {
    rules::mark_removed(flags)
}

/// `bool obs_canvas_rules_removed(uint32_t flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_canvas_rules_removed(flags: u32) -> bool {
    rules::removed(flags)
}
