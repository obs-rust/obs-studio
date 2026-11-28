//! C ABI for `libobs/obs-source-video-tick-callbacks.c.h`: the five
//! per-source callback steps of `obs_source_video_tick()`,
//! `obs_source_deferred_update()`, `activate_source()`,
//! `deactivate_source()`, `show_source()` and `hide_source()` (declared in
//! `obs-internal.h` without `EXPORT`, so the shims stay hidden:
//! `rust-hidden.d`).
//!
//! Runs the safe core ([`crate::video_tick_callbacks`]) over the C-owned
//! source. The callback checks and calls, the deferred count's address and
//! `obs_source_dosignal()` are the adapters of
//! `obs-source-video-tick-callbacks-fields.c.h`; the count is loaded and
//! swapped here with the C's sequentially consistent atomics. Like the C,
//! nothing checks the source: `obs_source_video_tick()` passes a live one.
//!
//! No intentional differences from C.

use core::ffi::{CStr, c_char, c_int, c_long, c_void};
use core::sync::atomic::Ordering::SeqCst;

use obs_util::platform::CLongAtomic;

use crate::video_tick_callbacks::{self, Host, Step};

unsafe extern "C" {
    fn obs_source_tick_callbacks_can_update(source: *const c_void) -> bool;
    fn obs_source_tick_callbacks_defer_count(source: *mut c_void) -> *const CLongAtomic;
    fn obs_source_tick_callbacks_update(source: *mut c_void);
    fn obs_source_tick_callbacks_has(source: *const c_void, step: c_int) -> bool;
    fn obs_source_tick_callbacks_call(source: *mut c_void, step: c_int);
    fn obs_source_tick_callbacks_signal(
        source: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    );
}

/// The adapters' step number.
const fn step_number(step: Step) -> c_int {
    match step {
        Step::Activate => 0,
        Step::Deactivate => 1,
        Step::Show => 2,
        Step::Hide => 3,
    }
}

/// The live libobs source the caller hands over.
struct Libobs;

impl Libobs {
    fn defer_count(source: *mut c_void) -> &'static CLongAtomic {
        // SAFETY: a live source keeps its aligned native long alive for the
        // call; C accesses it atomically too.
        unsafe { &*obs_source_tick_callbacks_defer_count(source) }
    }
}

// SAFETY (every method): `source` is the caller's live, non-null source.
impl Host for Libobs {
    type Source = *mut c_void;
    fn can_update(&self, source: *mut c_void) -> bool {
        // SAFETY: a live source.
        unsafe { obs_source_tick_callbacks_can_update(source) }
    }
    fn load_defer_count(&self, source: *mut c_void) -> c_long {
        Self::defer_count(source).load(SeqCst)
    }
    fn call_update(&self, source: *mut c_void) {
        // SAFETY: only after can_update(); nothing of the source is held
        // across the plugin's code, which may reenter.
        unsafe { obs_source_tick_callbacks_update(source) }
    }
    fn swap_defer_count(&self, source: *mut c_void, expected: c_long) {
        // The C ignores whether the swap happened.
        let _ = Self::defer_count(source).compare_exchange(expected, 0, SeqCst, SeqCst);
    }
    fn has_callback(&self, source: *mut c_void, step: Step) -> bool {
        // SAFETY: a live source; the step number is one the adapter knows.
        unsafe { obs_source_tick_callbacks_has(source, step_number(step)) }
    }
    fn call(&self, source: *mut c_void, step: Step) {
        // SAFETY: only after has_callback() for the same step; the plugin
        // may reenter.
        unsafe { obs_source_tick_callbacks_call(source, step_number(step)) }
    }
    fn signal(&self, source: *mut c_void, global: &'static CStr, local: &'static CStr) {
        // SAFETY: a live source and its signal handlers; static strings.
        unsafe { obs_source_tick_callbacks_signal(source, global.as_ptr(), local.as_ptr()) }
    }
}

/// `obs_source_deferred_update(source)`.
///
/// # Safety
/// `source` is a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_deferred_update(source: *mut c_void) {
    video_tick_callbacks::deferred_update(&Libobs, source);
}

/// `activate_source(source)`.
///
/// # Safety
/// `source` is a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn activate_source(source: *mut c_void) {
    video_tick_callbacks::run_step(&Libobs, source, Step::Activate);
}

/// `deactivate_source(source)`.
///
/// # Safety
/// `source` is a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deactivate_source(source: *mut c_void) {
    video_tick_callbacks::run_step(&Libobs, source, Step::Deactivate);
}

/// `show_source(source)`.
///
/// # Safety
/// `source` is a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn show_source(source: *mut c_void) {
    video_tick_callbacks::run_step(&Libobs, source, Step::Show);
}

/// `hide_source(source)`.
///
/// # Safety
/// `source` is a live source.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hide_source(source: *mut c_void) {
    video_tick_callbacks::run_step(&Libobs, source, Step::Hide);
}
