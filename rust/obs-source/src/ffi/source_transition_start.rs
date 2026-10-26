//! C ABI for `libobs/obs-source-transition-start.c.h`, the start checks and
//! child handover of the transitions, declared in
//! `libobs/obs-source-transition-start.h` without `EXPORT`.
//!
//! `obs-source-transition.c.h` (still C) reads the transition's fields in
//! its lock sections, compares the pointers itself, passes the results as
//! plain values to these shims and acts on what comes back: the locks, the
//! active child calls in their order, `obs_source_get_ref()` and
//! `obs_source_release()`, the recursion walk, the plugin callbacks, the
//! signals, the size and matrix recalculation and the logs stay there. So
//! `struct obs_source` is never mirrored, no pointer crosses, and the shims
//! take no lock, call nothing in libobs and touch no memory. All fourteen
//! stay hidden (`rust-hidden.txt`).
//!
//! The mode crosses as `int`, as the C passes `enum obs_transition_mode`
//! (int-sized, checked in `obs-source-transition.c.h`), and the admission
//! as the C's enum.

use std::os::raw::c_int;

use crate::transition_start::{self as core, Admission};

/// `enum obs_source_transition_start_admission`, as the C's enum is
/// returned (an `int` or `unsigned int`; every value fits both).
#[allow(non_camel_case_types)]
pub type obs_source_transition_start_admission = u32;
/// `OBS_SOURCE_TRANSITION_START_GO`.
pub const OBS_SOURCE_TRANSITION_START_GO: obs_source_transition_start_admission = 0;
/// `OBS_SOURCE_TRANSITION_START_REFUSE`.
pub const OBS_SOURCE_TRANSITION_START_REFUSE: obs_source_transition_start_admission = 1;
/// `OBS_SOURCE_TRANSITION_START_DONE`.
pub const OBS_SOURCE_TRANSITION_START_DONE: obs_source_transition_start_admission = 2;

const fn admission_value(admission: Admission) -> obs_source_transition_start_admission {
    match admission {
        Admission::Go => OBS_SOURCE_TRANSITION_START_GO,
        Admission::Refuse => OBS_SOURCE_TRANSITION_START_REFUSE,
        Admission::Done => OBS_SOURCE_TRANSITION_START_DONE,
    }
}

/// `bool obs_source_transition_start_refused(bool same_as_source, bool
/// active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_start_refused(same_as_source: bool, active: bool) -> bool {
    core::refused(same_as_source, active)
}

/// `bool obs_source_transition_start_walks(bool has_dest, bool
/// recursive)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_start_walks(has_dest: bool, recursive: bool) -> bool {
    core::walks(has_dest, recursive)
}

/// `enum obs_source_transition_start_admission
/// obs_source_transition_start_admit(bool same_as_source, bool
/// same_as_dest, bool same_mode, int mode, bool active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_start_admit(
    same_as_source: bool,
    same_as_dest: bool,
    same_mode: bool,
    mode: c_int,
    active: bool,
) -> obs_source_transition_start_admission {
    admission_value(core::admit(
        same_as_source,
        same_as_dest,
        same_mode,
        mode,
        active,
    ))
}

/// `uint64_t obs_source_transition_start_duration_ns(uint32_t ms)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_start_duration_ns(ms: u32) -> u64 {
    core::duration_ns(ms)
}

/// `bool obs_source_transition_start_restart(bool active, bool
/// same_as_dest, bool same_as_source)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_start_restart(
    active: bool,
    same_as_dest: bool,
    same_as_source: bool,
) -> bool {
    core::restart(active, same_as_dest, same_as_source)
}

/// `bool obs_source_transition_start_adds(bool already_active, bool
/// has_new)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_start_adds(already_active: bool, has_new: bool) -> bool {
    core::adds(already_active, has_new)
}

/// `bool obs_source_transition_start_removes(bool already_active, bool
/// has_old, bool add_ok)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_start_removes(
    already_active: bool,
    has_old: bool,
    add_ok: bool,
) -> bool {
    core::removes(already_active, has_old, add_ok)
}

/// `bool obs_source_transition_start_calls_back(bool has_callback, bool
/// add_ok)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_start_calls_back(has_callback: bool, add_ok: bool) -> bool {
    core::calls_back(has_callback, add_ok)
}

/// `bool obs_source_transition_start_unsized(uint32_t size)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_start_unsized(size: u32) -> bool {
    core::is_unsized(size)
}

/// `bool obs_source_transition_start_null_dest(bool has_dest, bool
/// same_as_dest, bool same_as_source)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_start_null_dest(
    has_dest: bool,
    same_as_dest: bool,
    same_as_source: bool,
) -> bool {
    core::null_dest(has_dest, same_as_dest, same_as_source)
}

/// `bool obs_source_transition_start_activates(bool slot_active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_start_activates(slot_active: bool) -> bool {
    core::activates(slot_active)
}

/// `size_t obs_source_transition_start_active_index(bool active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_start_active_index(active: bool) -> usize {
    core::active_index(active)
}

/// `bool obs_source_transition_start_manual_reset(int mode, float t)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_start_manual_reset(mode: c_int, t: f32) -> bool {
    core::manual_reset(mode, t)
}

/// `bool obs_source_transition_start_child_active(bool has_child, bool
/// active)`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_source_transition_start_child_active(has_child: bool, active: bool) -> bool {
    core::child_active(has_child, active)
}
