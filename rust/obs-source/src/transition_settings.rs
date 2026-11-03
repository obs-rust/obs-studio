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
//!   written; [`save`] gives the same record for plain values;
//! - the ten functions themselves ([`is_active`], [`set_scale_type`],
//!   [`get_scale_type`], [`set_alignment`], [`get_alignment`], [`set_size`],
//!   [`get_size`], [`enable_fixed`], [`fixed`] and [`save_to`]), generic over
//!   a [`Transition`] (the transition seam: one method per field read or
//!   write, the check and the lock) and, for the save, a [`SaveData`].
//!
//! The C shims live in the `obs-source-seam` crate, which implements
//! [`Transition`] over `libobs/obs-source-transition-seam.h` and [`SaveData`]
//! over `obs_data_set_*()`; only libobs-rust links it, so this crate calls
//! nothing in libobs. The locking is the C's: the setters and getters touch
//! the fields without the transition's lock, and the save reads the active
//! child and every saved field under it.
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

/// The fields, checks and lock of a transition that its settings use: in
/// libobs, the transition seam (`obs-source-transition-seam.h`), one call
/// per method. Every method is one read, one write or one lock call, as the
/// C's own reads and writes are.
pub trait Transition {
    /// A child in a slot, as the seam hands it back.
    type Child: Copy;

    /// `transition_valid(source, func)`: false for no source (the C logs
    /// `func` at debug level) and for one that is not a transition.
    fn valid(source: Option<&Self>, func: &'static CStr) -> bool;
    /// `transition_active()`: `transitioning_audio || transitioning_video`.
    fn active(&self) -> bool;
    /// `transition_sources[idx]`, `None` for an empty slot.
    fn source(&self, idx: usize) -> Option<Self::Child>;
    /// The child's `context.name`, `None` for a NULL name.
    fn child_name(&self, child: Self::Child) -> Option<&CStr>;
    /// `transition_mode`, as its `int`.
    fn mode(&self) -> i32;
    /// `transition_scale_type`, as its `int`.
    fn scale_type(&self) -> i32;
    /// Writes `transition_scale_type`.
    fn set_scale_type(&mut self, value: i32);
    /// `transition_alignment`.
    fn alignment(&self) -> u32;
    /// Writes `transition_alignment`.
    fn set_alignment(&mut self, value: u32);
    /// `transition_cx`.
    fn cx(&self) -> u32;
    /// Writes `transition_cx`.
    fn set_cx(&mut self, value: u32);
    /// `transition_cy`.
    fn cy(&self) -> u32;
    /// Writes `transition_cy`.
    fn set_cy(&mut self, value: u32);
    /// `transition_use_fixed_duration`.
    fn use_fixed_duration(&self) -> bool;
    /// Writes `transition_use_fixed_duration`.
    fn set_use_fixed_duration(&mut self, value: bool);
    /// Writes `transition_fixed_duration`.
    fn set_fixed_duration(&mut self, value: u32);
    /// Locks `transition_mutex`.
    fn lock(&self);
    /// Unlocks `transition_mutex`.
    fn unlock(&self);
}

/// Where `obs_transition_save()` writes: `obs_data_set_string()` and
/// `obs_data_set_int()`.
pub trait SaveData {
    /// `obs_data_set_string(data, key, value)`, `None` passing NULL.
    fn set_string(&mut self, key: &'static CStr, value: Option<&CStr>);
    /// `obs_data_set_int(data, key, value)`.
    fn set_int(&mut self, key: &'static CStr, value: i64);
}

/// `source` when `transition_valid()` accepts it for `func`.
fn checked<T: Transition>(source: Option<&T>, func: Func) -> Option<&T> {
    if T::valid(source, log_name(func)) {
        source
    } else {
        None
    }
}

/// [`checked`] for a setter.
fn checked_mut<T: Transition>(source: Option<&mut T>, func: Func) -> Option<&mut T> {
    if T::valid(source.as_deref(), log_name(func)) {
        source
    } else {
        None
    }
}

/// `obs_transition_is_active()`: no check of its source, as the C has none.
#[must_use]
pub fn is_active<T: Transition>(transition: &T) -> bool {
    transition.active()
}

/// `obs_transition_set_scale_type()`: a valid transition stores `value`
/// through the scale type rule ([`crate::transition_load::scale_type`], the
/// seam's `valid_scale_type()`), out of range as max-only.
pub fn set_scale_type<T: Transition>(transition: Option<&mut T>, value: i32) {
    if let Some(t) = checked_mut(transition, Func::SetScaleType) {
        t.set_scale_type(crate::transition_load::scale_type(i64::from(value)));
    }
}

/// `obs_transition_get_scale_type()`.
#[must_use]
pub fn get_scale_type<T: Transition>(transition: Option<&T>) -> i32 {
    checked(transition, Func::GetScaleType).map_or(Settings::INVALID.scale_type, T::scale_type)
}

/// `obs_transition_set_alignment()`: stored as given, unchecked.
pub fn set_alignment<T: Transition>(transition: Option<&mut T>, alignment: u32) {
    if let Some(t) = checked_mut(transition, Func::SetAlignment) {
        t.set_alignment(alignment);
    }
}

/// `obs_transition_get_alignment()`.
#[must_use]
pub fn get_alignment<T: Transition>(transition: Option<&T>) -> u32 {
    checked(transition, Func::GetAlignment).map_or(Settings::INVALID.alignment, T::alignment)
}

/// `obs_transition_set_size()`: the width, then the height.
pub fn set_size<T: Transition>(transition: Option<&mut T>, cx: u32, cy: u32) {
    if let Some(t) = checked_mut(transition, Func::SetSize) {
        t.set_cx(cx);
        t.set_cy(cy);
    }
}

/// `obs_transition_get_size()`: what it writes to `*cx` and `*cy`, both 0
/// for an invalid source. Logs as `obs_transition_get_size` (intentional
/// difference).
#[must_use]
pub fn get_size<T: Transition>(transition: Option<&T>) -> (u32, u32) {
    checked(transition, Func::GetSize).map_or((Settings::INVALID.cx, Settings::INVALID.cy), |t| {
        let cx = t.cx();
        (cx, t.cy())
    })
}

/// `obs_transition_enable_fixed()`: the switch, then the duration.
pub fn enable_fixed<T: Transition>(transition: Option<&mut T>, enable: bool, duration: u32) {
    if let Some(t) = checked_mut(transition, Func::EnableFixed) {
        t.set_use_fixed_duration(enable);
        t.set_fixed_duration(duration);
    }
}

/// `obs_transition_fixed()`.
#[must_use]
pub fn fixed<T: Transition>(transition: Option<&T>) -> bool {
    checked(transition, Func::Fixed)
        .map_or(Settings::INVALID.use_fixed_duration, T::use_fixed_duration)
}

/// [`save_to`]'s reads of a locked transition, each just before its write.
struct LockedReads<'a, T: Transition> {
    transition: &'a T,
    child: Option<T::Child>,
}

impl<'a, T: Transition> SaveReads<'a> for LockedReads<'a, T> {
    fn source_a(&mut self) -> Child<'a> {
        match self.child {
            None => Child::Empty,
            Some(child) => self
                .transition
                .child_name(child)
                .map_or(Child::Unnamed, Child::Named),
        }
    }
    fn alignment(&mut self) -> u32 {
        self.transition.alignment()
    }
    fn mode(&mut self) -> i32 {
        self.transition.mode()
    }
    fn scale_type(&mut self) -> i32 {
        self.transition.scale_type()
    }
    fn cx(&mut self) -> u32 {
        self.transition.cx()
    }
    fn cy(&mut self) -> u32 {
        self.transition.cy()
    }
}

/// `obs_transition_save()`: no check of its source, as the C has none. Under
/// the transition's lock it finds the active child (slot B while the
/// transition runs, [`crate::transition_start::active_index`]), then writes
/// the six values in [`save_each`]'s order, each read just before its
/// write, and unlocks.
pub fn save_to<T: Transition, D: SaveData>(transition: &T, data: &mut D) {
    transition.lock();
    let child = transition.source(crate::transition_start::active_index(transition.active()));
    let mut reads = LockedReads { transition, child };
    save_each(&mut reads, |key, value| match value {
        Value::Str(s) => data.set_string(key, Some(s)),
        Value::Null => data.set_string(key, None),
        Value::Int(n) => data.set_int(key, n),
    });
    transition.unlock();
}
