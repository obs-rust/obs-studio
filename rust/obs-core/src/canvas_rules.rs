//! Safe core of `libobs/obs-canvas-rules.c.h`: the canvas flag rules and
//! setter checks of `libobs/obs-canvas.c.h`.
//!
//! - [`main_flags`] and [`public_flags`]: the flags of
//!   `obs_create_main_canvas()`, and the mask of `obs_canvas_create()`,
//!   `obs_canvas_create_private()` and `obs_load_canvas()` (which truncates
//!   the saved `int64` to `uint32_t` first, in C);
//! - [`main_view`] and [`mix_audio`]: the view type and the mix's
//!   `mix_audio`, in `obs_canvas_create_internal()` and
//!   `obs_canvas_reset_video_internal()`;
//! - [`is_public`]: a public canvas is named, unnamed, renamed through the
//!   hash table and signalled on `obs->signals`;
//! - [`holds_scenes`]: whether the canvas holds a reference to each scene
//!   in it, in `obs_canvas_destroy()`, `obs_canvas_insert_source()` and
//!   `obs_canvas_remove_source()`;
//! - [`saves`], in `obs_save_canvas()`;
//! - [`size_set`] and [`format_set`], in
//!   `obs_canvas_has_valid_video_info()`;
//! - [`resets`], in `obs_canvas_reset_video()`;
//! - [`global_signals`], in `obs_canvas_rename_source()` and
//!   `obs_canvas_set_channel()`;
//! - [`channel_ok`], in `obs_canvas_set_channel()`;
//! - [`name_empty`] and [`renames`], in `obs_canvas_set_name()`;
//! - [`removable`], [`mark_removed`] and [`removed`], in
//!   `obs_canvas_remove()` and `obs_canvas_removed()`.
//!
//! Each decision is its own function, asked at its own point, so
//! `obs-canvas.c` keeps every read where it was: `canvas->flags` is read
//! without a lock and `obs_canvas_remove()` writes it, so each check gets
//! the value its own read gave, and a check behind a `||` or `&&` is still
//! asked only when the C reached it (`obs_source_is_scene()` only on a
//! `SCENE_REF` canvas, `obs_video_active()` only for a canvas that may
//! reset, `strcmp()` only for a canvas that may be renamed, each video info
//! field only after the one before it was set). The C keeps the allocations
//! and frees, the context data calls, the mutexes, the view, the mix and its
//! list, the hash walks (in the #2038 and #2049 order), every reference, the
//! pointer checks, the calldata, every signal, the activation calls and the
//! logs. Nothing here keeps state, takes a lock or calls libobs.
//!
//! The flags are `enum obs_canvas_flags` (`obs.h`) and the internal
//! `REMOVED` bit, as the C's `uint32_t`; the video format is
//! `enum video_format`, as the C's `int`.
//!
//! Characterized, not endorsed (`test_canvas_rules_steps`), and kept: a
//! saved flags value above 32 bits or negative keeps the public flags of its
//! low word (the C truncates before the mask); a video format outside the
//! enum counts as set; a removed canvas can still be reset and renamed.
//!
//! No intentional differences from the C.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

use core::ffi::c_int;

/// `MAIN`: the main canvas, created by libobs; it cannot be renamed, reset
/// or removed, and a caller cannot set it.
pub const MAIN: u32 = 1 << 0;
/// `ACTIVATE`: the canvas's sources become active when they are visible.
pub const ACTIVATE: u32 = 1 << 1;
/// `MIX_AUDIO`: the audio of the canvas's channels is mixed into the output.
pub const MIX_AUDIO: u32 = 1 << 2;
/// `SCENE_REF`: the canvas holds references to its scenes.
pub const SCENE_REF: u32 = 1 << 3;
/// `EPHEMERAL`: the canvas is not saved.
pub const EPHEMERAL: u32 = 1 << 4;
/// `PROGRAM`: `ACTIVATE | MIX_AUDIO | SCENE_REF`.
pub const PROGRAM: u32 = ACTIVATE | MIX_AUDIO | SCENE_REF;
/// `PREVIEW`: `EPHEMERAL`.
pub const PREVIEW: u32 = EPHEMERAL;
/// `DEVICE`: `ACTIVATE | EPHEMERAL`.
pub const DEVICE: u32 = ACTIVATE | EPHEMERAL;
/// The internal flag that marks a removed canvas
/// (`OBS_CANVAS_RULES_REMOVED`).
pub const REMOVED: u32 = 1 << 31;
/// The flags a caller or a saved canvas may set
/// (`OBS_CANVAS_RULES_PUBLIC`): `MAIN`, `REMOVED` and unknown bits are
/// dropped.
pub const PUBLIC: u32 = ACTIVATE | MIX_AUDIO | SCENE_REF | EPHEMERAL;
/// `VIDEO_FORMAT_NONE` (`OBS_CANVAS_RULES_FORMAT_NONE`).
pub const FORMAT_NONE: c_int = 0;
/// `MAX_CHANNELS` (`OBS_CANVAS_RULES_MAX_CHANNELS`).
pub const MAX_CHANNELS: u32 = 64;

/// `obs_canvas_rules_main_flags()`: `MAIN | PROGRAM`.
#[must_use]
pub fn main_flags() -> u32 {
    MAIN | PROGRAM
}

/// `obs_canvas_rules_public_flags()`: `flags & PUBLIC`.
#[must_use]
pub fn public_flags(flags: u32) -> u32 {
    flags & PUBLIC
}

/// `obs_canvas_rules_main_view()`: a `MAIN_VIEW` (else `AUX_VIEW`) for an
/// [`ACTIVATE`] canvas.
#[must_use]
pub fn main_view(flags: u32) -> bool {
    flags & ACTIVATE != 0
}

/// `obs_canvas_rules_mix_audio()`: [`MIX_AUDIO`].
#[must_use]
pub fn mix_audio(flags: u32) -> bool {
    flags & MIX_AUDIO != 0
}

/// `obs_canvas_rules_public()`: a canvas that is not private.
#[must_use]
pub fn is_public(is_private: bool) -> bool {
    !is_private
}

/// `obs_canvas_rules_holds_scenes()`: [`SCENE_REF`].
#[must_use]
pub fn holds_scenes(flags: u32) -> bool {
    flags & SCENE_REF != 0
}

/// `obs_canvas_rules_saves()`: neither [`EPHEMERAL`] nor [`REMOVED`].
#[must_use]
pub fn saves(flags: u32) -> bool {
    flags & (EPHEMERAL | REMOVED) == 0
}

/// `obs_canvas_rules_size_set()`: a size that is not 0.
#[must_use]
pub fn size_set(size: u32) -> bool {
    size != 0
}

/// `obs_canvas_rules_format_set()`: a format that is not
/// [`FORMAT_NONE`], one outside the enum included.
#[must_use]
pub fn format_set(format: c_int) -> bool {
    format != FORMAT_NONE
}

/// `obs_canvas_rules_resets()`: any canvas but [`MAIN`].
#[must_use]
pub fn resets(flags: u32) -> bool {
    flags & MAIN == 0
}

/// `obs_canvas_rules_global_signals()`: [`MAIN`].
#[must_use]
pub fn global_signals(flags: u32) -> bool {
    flags & MAIN != 0
}

/// `obs_canvas_rules_channel_ok()`: below [`MAX_CHANNELS`].
#[must_use]
pub fn channel_ok(channel: u32) -> bool {
    channel < MAX_CHANNELS
}

/// `obs_canvas_rules_name_empty()`: no name (`None`, the C's `NULL`), or a
/// name whose first byte is 0.
#[must_use]
pub fn name_empty(first_byte: Option<u8>) -> bool {
    first_byte.is_none_or(|byte| byte == 0)
}

/// `obs_canvas_rules_renames()`: any canvas but [`MAIN`].
#[must_use]
pub fn renames(flags: u32) -> bool {
    flags & MAIN == 0
}

/// `obs_canvas_rules_removable()`: neither [`REMOVED`] nor [`MAIN`].
#[must_use]
pub fn removable(flags: u32) -> bool {
    flags & (REMOVED | MAIN) == 0
}

/// `obs_canvas_rules_mark_removed()`: `flags | REMOVED`.
#[must_use]
pub fn mark_removed(flags: u32) -> u32 {
    flags | REMOVED
}

/// `obs_canvas_rules_removed()`: [`REMOVED`].
#[must_use]
pub fn removed(flags: u32) -> bool {
    flags & REMOVED != 0
}
