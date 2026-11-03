//! Safe core of `libobs/obs-source-transition-settings.c.h`: the settings
//! of a transition (`obs_transition_is_active()`, the scale type, alignment
//! and size setters and getters, `obs_transition_save()` and the fixed
//! duration switch), moved out of `libobs/obs-source-transition.c.h`.
//!
//! - [`log_name`]: the function name each setter and getter hands
//!   `transition_valid()`, which logs it for a NULL source;
//! - [`Settings::INVALID`]: what the getters answer for a source that is not
//!   a valid transition;
//! - [`save_each`]: the six keys `obs_transition_save()` writes, in the C's
//!   order, each value asked of the caller ([`SaveReads`]) just before it is
//!   written; [`save`] gives the same record for plain values.
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
    clippy::unreachable,
    clippy::todo
)]

use std::ffi::CStr;

/// `OBS_TRANSITION_SCALE_MAX_ONLY` (`enum obs_transition_scale_type`).
pub const SCALE_MAX_ONLY: i32 = 0;

/// Whether the host's C makes `enum obs_transition_mode` and `enum
/// obs_transition_scale_type` `unsigned int` (GCC and Clang, MinGW
/// included) rather than `int` (MSVC and clang-cl): it decides how
/// [`save_each`] widens a stored value with its top bit set.
pub const ENUMS_ARE_UNSIGNED: bool = crate::platform::ENUMS_ARE_UNSIGNED;

/// `(int64_t)` of an enum field holding `value`'s bits: zero-extended where
/// the enum is `unsigned int`, sign-extended where it is `int`.
#[must_use]
pub fn enum_to_i64(value: i32) -> i64 {
    if ENUMS_ARE_UNSIGNED {
        i64::from(u32::from_ne_bytes(value.to_ne_bytes()))
    } else {
        i64::from(value)
    }
}

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
pub const fn log_name(func: Func) -> &'static CStr {
    match func {
        Func::SetScaleType => c"obs_transition_set_scale_type",
        Func::GetScaleType => c"obs_transition_get_scale_type",
        Func::SetAlignment => c"obs_transition_set_alignment",
        Func::GetAlignment => c"obs_transition_get_alignment",
        Func::SetSize => c"obs_transition_set_size",
        // intentional difference: the C passes "obs_transition_set_size"
        Func::GetSize => c"obs_transition_get_size",
        Func::EnableFixed => c"obs_transition_enable_fixed",
        Func::Fixed => c"obs_transition_fixed",
    }
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

/// The active child as `obs_transition_save()` finds it under the lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Child<'a> {
    /// An empty slot: saved as `""`.
    Empty,
    /// A child and its `context.name`.
    Named(&'a CStr),
    /// A child whose `context.name` is NULL: the C passes that NULL to
    /// `obs_data_set_string()`.
    Unnamed,
}

/// The reads of `obs_transition_save()`, each asked by [`save_each`] just
/// before its value is written, as the C reads each field in its call.
pub trait SaveReads<'a> {
    /// The active child.
    fn source_a(&mut self) -> Child<'a>;
    /// `transition_alignment`.
    fn alignment(&mut self) -> u32;
    /// `transition_mode`, as its `int`.
    fn mode(&mut self) -> i32;
    /// `transition_scale_type`, as its `int`.
    fn scale_type(&mut self) -> i32;
    /// `transition_cx`.
    fn cx(&mut self) -> u32;
    /// `transition_cy`.
    fn cy(&mut self) -> u32;
}

/// What `obs_transition_save()` reads under the transition's lock, as plain
/// values.
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

impl<'a> SaveReads<'a> for Saved<'a> {
    fn source_a(&mut self) -> Child<'a> {
        self.source_a.map_or(Child::Empty, Child::Named)
    }
    fn alignment(&mut self) -> u32 {
        self.alignment
    }
    fn mode(&mut self) -> i32 {
        self.mode
    }
    fn scale_type(&mut self) -> i32 {
        self.scale_type
    }
    fn cx(&mut self) -> u32 {
        self.cx
    }
    fn cy(&mut self) -> u32 {
        self.cy
    }
}

/// One value `obs_transition_save()` writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value<'a> {
    /// `obs_data_set_string()`.
    Str(&'a CStr),
    /// `obs_data_set_string()` with a NULL value ([`Child::Unnamed`]).
    Null,
    /// `obs_data_set_int()`.
    Int(i64),
}

/// `obs_transition_save()`'s writes, in its order, each value read from
/// `reads` just before `write` gets it: `transition_source_a` (the child's
/// name, `""` for an empty slot), then `transition_alignment`,
/// `transition_mode`, `transition_scale_type`, `transition_cx` and
/// `transition_cy` as integers (the `uint32_t` fields zero-extended, the
/// enums by [`enum_to_i64`], as the C's `(int64_t)` casts do).
pub fn save_each<'a>(
    reads: &mut impl SaveReads<'a>,
    mut write: impl FnMut(&'static CStr, Value<'a>),
) {
    write(
        c"transition_source_a",
        match reads.source_a() {
            Child::Empty => Value::Str(c""),
            Child::Named(name) => Value::Str(name),
            Child::Unnamed => Value::Null,
        },
    );
    write(
        c"transition_alignment",
        Value::Int(i64::from(reads.alignment())),
    );
    write(c"transition_mode", Value::Int(enum_to_i64(reads.mode())));
    write(
        c"transition_scale_type",
        Value::Int(enum_to_i64(reads.scale_type())),
    );
    write(c"transition_cx", Value::Int(i64::from(reads.cx())));
    write(c"transition_cy", Value::Int(i64::from(reads.cy())));
}

/// The keys and values [`save_each`] writes for `saved`, in its order.
#[must_use]
pub fn save(mut saved: Saved<'_>) -> [(&'static CStr, Value<'_>); 6] {
    let mut out = [(c"", Value::Int(0)); 6];
    let mut slots = out.iter_mut();
    save_each(&mut saved, |key, value| {
        if let Some(slot) = slots.next() {
            *slot = (key, value);
        }
    });
    out
}
