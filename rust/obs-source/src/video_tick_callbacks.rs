//! Safe core of `libobs/obs-source-video-tick-callbacks.c.h`: the
//! per-source callback steps of `obs_source_video_tick()`, its only
//! caller (the decisions of when they run are [`crate::video_tick`]'s).
//!
//! - [`deferred_update`] (`obs_source_deferred_update()`): only when the
//!   source has data and its type an `update` callback, the deferred count
//!   is loaded, `update` called with the data and settings, the count
//!   swapped to 0 only if it still holds the value loaded before the call
//!   (so an update deferred again from inside the callback survives), and
//!   `source_update` / `update` sent. Otherwise nothing at all happens, the
//!   signal included.
//! - [`run_step`] (`activate_source()`, `deactivate_source()`,
//!   `show_source()`, `hide_source()`): the type's callback with the
//!   source's data when both are set, then the step's global and local
//!   signals, always.
//!
//! The callbacks, the deferred count and `obs_source_dosignal()` stay C,
//! reached through the [`Host`] trait.
//!
//! No intentional differences from C.

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

use core::ffi::{CStr, c_long};

/// One of the four edge steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// `activate_source()`.
    Activate,
    /// `deactivate_source()`.
    Deactivate,
    /// `show_source()`.
    Show,
    /// `hide_source()`.
    Hide,
}

impl Step {
    /// The global and the source's signal the step sends.
    #[must_use]
    pub const fn signals(self) -> (&'static CStr, &'static CStr) {
        match self {
            Self::Activate => (c"source_activate", c"activate"),
            Self::Deactivate => (c"source_deactivate", c"deactivate"),
            Self::Show => (c"source_show", c"show"),
            Self::Hide => (c"source_hide", c"hide"),
        }
    }
}

/// The source operations the steps perform, in the C's terms.
pub trait Host {
    /// A live, non-null source.
    type Source: Copy;
    /// `source->context.data && source->info.update`.
    fn can_update(&self, source: Self::Source) -> bool;
    /// `os_atomic_load_long(&source->defer_update_count)`.
    fn load_defer_count(&self, source: Self::Source) -> c_long;
    /// `source->info.update(source->context.data, source->context.settings)`.
    fn call_update(&self, source: Self::Source);
    /// `os_atomic_compare_swap_long(&source->defer_update_count, expected,
    /// 0)`.
    fn swap_defer_count(&self, source: Self::Source, expected: c_long);
    /// `source->context.data && source->info.<step>`.
    fn has_callback(&self, source: Self::Source, step: Step) -> bool;
    /// `source->info.<step>(source->context.data)`.
    fn call(&self, source: Self::Source, step: Step);
    /// `obs_source_dosignal(source, global, local)`.
    fn signal(&self, source: Self::Source, global: &'static CStr, local: &'static CStr);
}

/// `obs_source_deferred_update(source)`.
pub fn deferred_update<H: Host>(_host: &H, _source: H::Source) {
    todo!("obs_source_deferred_update")
}

/// `activate_source()`, `deactivate_source()`, `show_source()` or
/// `hide_source()` for `step`.
pub fn run_step<H: Host>(_host: &H, _source: H::Source, _step: Step) {
    todo!("activate_source, deactivate_source, show_source, hide_source")
}
