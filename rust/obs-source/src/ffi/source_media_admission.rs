//! C ABI for `libobs/obs-source-media-admission.c.h`, the media action
//! admission decisions, declared in `libobs/obs-source-media-admission.h`
//! without `EXPORT`.
//!
//! The `obs_source_media_*()` functions and `process_media_actions()` in
//! `obs-source.c.h` (still C) call `data_valid()` or `obs_source_valid()`,
//! read the source's output flags and callbacks when a step asks for them,
//! keep the action queue and its mutex, call the callbacks and emit the
//! signals. So `struct obs_source` is never mirrored, and these shims take
//! plain values, touch no memory, take no lock and call nothing in libobs.
//! All eight stay hidden (`rust-hidden.txt`).
//!
//! The enums cross as the C's `int` (`obs-source.c.h` checks
//! `enum media_action_type` and `enum obs_media_state`; the header's own
//! enums have no negative value, so an `unsigned int` one passes the same
//! bits), and a value outside an enum takes the path the C takes. The
//! signal names are static C strings, as the C's literals are.
//!
//! No intentional differences from C: every input is a value, so there is
//! no pointer to check.

use std::os::raw::{c_char, c_int};
use std::ptr;

use crate::media_admission::{self as core, Call, Dispatch};

/// `enum obs_source_media_call`, `enum obs_source_media_admit` and
/// `enum obs_source_media_dispatch`, as the C passes and returns them.
#[allow(non_camel_case_types)]
pub type obs_source_media_enum = c_int;

/// `enum obs_source_media_admit obs_source_media_admit(bool valid)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_media_admit(valid: bool) -> obs_source_media_enum {
    core::admit(valid).value()
}

/// `enum obs_source_media_admit obs_source_media_admit_flags(enum
/// obs_source_media_call call, uint32_t output_flags)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_media_admit_flags(
    call: obs_source_media_enum,
    output_flags: u32,
) -> obs_source_media_enum {
    core::admit_flags(Call::from_value(call), output_flags).value()
}

/// `enum obs_source_media_admit obs_source_media_admit_callback(bool
/// has_callback)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_media_admit_callback(has_callback: bool) -> obs_source_media_enum {
    core::admit_callback(has_callback).value()
}

/// `int64_t obs_source_media_time_fallback(void)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_media_time_fallback() -> i64 {
    core::time_fallback()
}

/// `int obs_source_media_state_fallback(void)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_media_state_fallback() -> c_int {
    core::state_fallback()
}

/// `enum obs_source_media_dispatch obs_source_media_dispatch(int type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_media_dispatch(action_type: c_int) -> obs_source_media_enum {
    core::dispatch(action_type).value()
}

/// `bool obs_source_media_signal_reads_pause(enum obs_source_media_dispatch
/// dispatch)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_media_signal_reads_pause(dispatch: obs_source_media_enum) -> bool {
    core::signal_reads_pause(Dispatch::from_value(dispatch))
}

/// `const char *obs_source_media_signal(enum obs_source_media_dispatch
/// dispatch, bool pause)`: a static name, or NULL.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_media_signal(
    dispatch: obs_source_media_enum,
    pause: bool,
) -> *const c_char {
    core::signal(Dispatch::from_value(dispatch), pause).map_or(ptr::null(), |name| name.as_ptr())
}
