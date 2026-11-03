//! Safe core of `libobs/obs-source-transition-settings.c.h`: the settings
//! of a transition (`obs_transition_is_active()`, the scale type, alignment
//! and size setters and getters, `obs_transition_save()` and the fixed
//! duration switch), moved out of `libobs/obs-source-transition.c.h`.
//!
//! - [`log_name`]: the function name each setter and getter hands
//!   `transition_valid()`, which logs it for a NULL source;
//! - [`Settings::INVALID`]: what the getters answer for a source that is not
//!   a valid transition;
//! - [`save`]: the six keys `obs_transition_save()` writes, in the C's order,
//!   with their values.
//!
//! The C shims ([`crate::ffi::source_transition_settings`]) keep every read
//! and write of the transition's fields through the transition seam
//! (`obs-source-transition-seam.h`), under the C's conditions: the setters
//! and getters touch the fields without the transition's lock, and the save
//! reads the active child and every saved field under it. Nothing here keeps
//! state, takes a lock or calls libobs.
//!
//! One intentional difference from the C: `obs_transition_get_size()` logs
//! under its own name. The C passes `"obs_transition_set_size"`, so the
//! debug line for a NULL source names the setter.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use std::ffi::CStr;

/// `OBS_TRANSITION_SCALE_MAX_ONLY` (`enum obs_transition_scale_type`).
pub const SCALE_MAX_ONLY: i32 = 0;

/// The functions of the settings file that check their source with
/// `transition_valid()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    /// `obs_transition_set_scale_type()`.
    SetScaleType,
    /// `obs_transition_get_scale_type()`.
    GetScaleType,
    /// `obs_transition_set_alignment()`.
    SetAlignment,
    /// `obs_transition_get_alignment()`.
    GetAlignment,
    /// `obs_transition_set_size()`.
    SetSize,
    /// `obs_transition_get_size()`.
    GetSize,
    /// `obs_transition_enable_fixed()`.
    EnableFixed,
    /// `obs_transition_fixed()`.
    Fixed,
}

/// The name `func` passes to `transition_valid()`: its own name, which the
/// NULL check logs. `GetSize` gives `"obs_transition_get_size"`, where the C
/// passes `"obs_transition_set_size"` (intentional difference).
#[must_use]
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn log_name(func: Func) -> &'static CStr {
    let _ = func;
    todo!()
}

/// The values the getters read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// `transition_scale_type`, as its `int`.
    pub scale_type: i32,
    /// `transition_alignment`.
    pub alignment: u32,
    /// `transition_cx`.
    pub cx: u32,
    /// `transition_cy`.
    pub cy: u32,
    /// `transition_use_fixed_duration`.
    pub use_fixed_duration: bool,
}

impl Settings {
    /// What every getter answers for a NULL source or one that is not a
    /// transition: `OBS_TRANSITION_SCALE_MAX_ONLY`, alignment 0, `0x0` (the
    /// size getter writes both) and not fixed.
    pub const INVALID: Self = Self {
        scale_type: SCALE_MAX_ONLY,
        alignment: 0,
        cx: 0,
        cy: 0,
        use_fixed_duration: false,
    };
}

/// What `obs_transition_save()` reads under the transition's lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Saved<'a> {
    /// The name of the child in the active slot, `None` for an empty slot.
    pub source_a: Option<&'a CStr>,
    /// `transition_alignment`.
    pub alignment: u32,
    /// `transition_mode`, as its `int`.
    pub mode: i32,
    /// `transition_scale_type`, as its `int`.
    pub scale_type: i32,
    /// `transition_cx`.
    pub cx: u32,
    /// `transition_cy`.
    pub cy: u32,
}

/// One value `obs_transition_save()` writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value<'a> {
    /// `obs_data_set_string()`.
    Str(&'a CStr),
    /// `obs_data_set_int()`.
    Int(i64),
}

/// The keys and values `obs_transition_save()` writes, in its order:
/// `transition_source_a` (the child's name, `""` for an empty slot), then
/// `transition_alignment`, `transition_mode`, `transition_scale_type`,
/// `transition_cx` and `transition_cy` as integers (the `uint32_t` fields
/// zero-extended, the enums sign-extended, as the C's conversions to
/// `long long` do).
#[must_use]
#[expect(clippy::todo, reason = "RED: the port follows")]
pub fn save(saved: Saved<'_>) -> [(&'static CStr, Value<'_>); 6] {
    let _ = saved;
    todo!()
}
