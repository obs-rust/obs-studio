//! Safe core of `libobs/obs-source-update.c.h`: a source's settings update.
//!
//! The source, its settings object, its `defer_update_count` and the plugin's
//! callback stay C-owned; the shim reaches them through the live adapter
//! `obs-source-update-fields.c.h` and `obs_data_apply()`. No lock is taken,
//! as in C, and the core holds nothing of the source while the callback or
//! the signal handlers run.

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

use core::ffi::CStr;
use obs_util::platform::CLongAtomic;
use std::sync::atomic::Ordering::SeqCst;

/// `OBS_SOURCE_VIDEO` from `obs-source.h`.
pub const OBS_SOURCE_VIDEO: u32 = 1 << 0;

/// What `obs_source_update()` reads and calls on a valid source, each at
/// the point the C did.
pub trait Source {
    /// `obs_data_apply(context.settings, settings)`; called only for
    /// non-NULL incoming settings.
    fn apply_settings(&self);
    /// `info.output_flags`.
    fn output_flags(&self) -> u32;
    /// `defer_update_count`, the native `long` the video tick reads.
    fn defer_update_count(&self) -> &CLongAtomic;
    /// `context.data && info.update`.
    fn can_update(&self) -> bool;
    /// `info.update(context.data, context.settings)`.
    fn call_update(&self);
    /// `obs_source_dosignal(source, "source_update", "update")`.
    fn signal_update(&self);
}

/// `obs_source_update()`: a NULL source (`None`) logs through `log` and does
/// nothing else. Otherwise non-NULL incoming settings (`has_settings`) are
/// merged first; then a video source only counts one deferred update for
/// its next video tick, and any other source with data and an `update`
/// callback runs it and then sends `source_update`.
pub fn update(source: Option<&impl Source>, has_settings: bool, log: impl FnOnce(&'static CStr)) {
    let Some(source) = source else {
        log(c"obs_source_update");
        return;
    };
    if has_settings {
        source.apply_settings();
    }
    if source.output_flags() & OBS_SOURCE_VIDEO != 0 {
        source.defer_update_count().fetch_add(1, SeqCst);
    } else if source.can_update() {
        source.call_update();
        source.signal_update();
    }
}
