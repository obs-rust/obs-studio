//! C ABI for `libobs/obs-source-transition-load.c.h`, the load, scale type
//! and swap checks of the transitions, declared in
//! `libobs/obs-source-transition-load.h` without `EXPORT`.
//!
//! `obs-source-transition.c.h` (still C) checks the pointer with
//! `obs_ptr_valid()`, reads the transition's fields in its lock sections,
//! compares the pointers itself, passes the results as plain values to these
//! shims and acts on what comes back: the locks, the lookup, the active
//! child calls (each add only under the C's own `&&`),
//! `obs_source_get_ref()` and `obs_source_release()`, the truncating casts,
//! the swap's field handover, the texrender swap, the size and matrix
//! recalculation and the logs stay there. So `struct obs_source` is never
//! mirrored, no pointer crosses, and the shims take no lock, call nothing in
//! libobs and touch no memory. All six stay hidden (`rust-hidden.txt`).
//!
//! The source type crosses as `int`, as the C passes `enum obs_source_type`
//! (int-sized, checked in `obs-source-transition.c.h`); the scale type comes
//! in as the `int64_t` `obs_data` holds and goes out as `int`; the source
//! outcome is the C's enum.

use std::os::raw::c_int;

use crate::transition_load::{self as core, Source};

/// `enum obs_source_transition_load_outcome`, as the C's enum is returned
/// (an `int` or `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_source_transition_load_outcome = u32;
/// `OBS_SOURCE_TRANSITION_LOAD_KEEP`.
pub const OBS_SOURCE_TRANSITION_LOAD_KEEP: obs_source_transition_load_outcome = 0;
/// `OBS_SOURCE_TRANSITION_LOAD_NOT_FOUND`.
pub const OBS_SOURCE_TRANSITION_LOAD_NOT_FOUND: obs_source_transition_load_outcome = 1;
/// `OBS_SOURCE_TRANSITION_LOAD_REFUSE`.
pub const OBS_SOURCE_TRANSITION_LOAD_REFUSE: obs_source_transition_load_outcome = 2;

const fn source_value(source: Source) -> obs_source_transition_load_outcome {
    match source {
        Source::Keep => OBS_SOURCE_TRANSITION_LOAD_KEEP,
        Source::NotFound => OBS_SOURCE_TRANSITION_LOAD_NOT_FOUND,
        Source::Refuse => OBS_SOURCE_TRANSITION_LOAD_REFUSE,
    }
}

/// `bool obs_source_transition_load_is_transition(int type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_load_is_transition(source_type: c_int) -> bool {
    core::is_transition(source_type)
}

/// `int obs_source_transition_load_scale_type(int64_t type)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_load_scale_type(value: i64) -> c_int {
    core::scale_type(value)
}

/// `bool obs_source_transition_load_looks_up(bool has_name)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_load_looks_up(has_name: bool) -> bool {
    core::looks_up(has_name)
}

/// `enum obs_source_transition_load_outcome
/// obs_source_transition_load_source(bool found, bool add_ok)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_load_source(
    found: bool,
    add_ok: bool,
) -> obs_source_transition_load_outcome {
    source_value(core::source(found, add_ok))
}

/// `bool obs_source_transition_load_swaps(bool same)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_load_swaps(same: bool) -> bool {
    core::swaps(same)
}

/// `bool obs_source_transition_load_copy_stores(bool adds, bool add_ok)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_load_copy_stores(adds: bool, add_ok: bool) -> bool {
    core::copy_stores(adds, add_ok)
}
