//! Safe core of `libobs/obs-source-transition-start.c.h`: the start checks
//! and child handover of the transitions in
//! `libobs/obs-source-transition.c.h`.
//!
//! - [`refused`], [`walks`], [`admit`], [`duration_ns`] and [`null_dest`]:
//!   the steps of `obs_transition_start()` between its three lock sections
//!   (the first refusal, the recursion walk, the second admission, the
//!   clock's duration and the start to no destination);
//! - [`restart`], [`adds`], [`removes`], [`calls_back`] and [`is_unsized`]:
//!   `set_source()` (the clock rule, the handover of an active slot, the
//!   admission callback and the size recalculation);
//! - [`activates`], in `activate_transition()`;
//! - [`active_index`], in `obs_transition_get_active_source()` and
//!   `obs_transition_save()`;
//! - [`manual_reset`], in `obs_transition_set_manual_time()`;
//! - [`child_active`], in `obs_transition_set()` and `copy_source_state()`.
//!
//! Each decision is its own function, asked at its own point, so
//! `obs-source-transition.c` keeps every read where it was.
//! `obs_transition_start()` reads the sources in three separate lock
//! sections on purpose (the plugin's `transition_start` callback can
//! re-enter, and `obs_transition_set()` runs in between), and each function
//! here gets only what one section read. The C keeps the locks, the pointer
//! comparisons, `transition_active()` (the video flag is read only when the
//! audio flag is clear), the fixed duration read (only when it is enabled),
//! the size reads (`cy` only when `cx` is not 0), every
//! `obs_source_get_ref()` and `obs_source_release()`, the active child
//! calls in their order (the new child added before the old one is
//! removed), the recursion walk, the plugin callbacks, the signals, the
//! size and matrix recalculation, `os_gettime_ns()` and the logs. Nothing
//! here keeps state, takes a lock or calls libobs.
//!
//! Characterized, not endorsed (`test_transition_start_steps`), and kept: a
//! mode written by a re-entering start stays; since upstream's finish step
//! runs before the second read, a running manual transition started again
//! to the same destination finishes on it and the start is refused, so the
//! [`Admission::Done`] success needs another thread's start in between.

/// `OBS_TRANSITION_MODE_MANUAL` (`enum obs_transition_mode`), the same
/// constant [`crate::transition_clock`] uses.
pub const MODE_MANUAL: i32 = crate::transition_clock::MODE_MANUAL;

/// `enum obs_source_transition_start_admission`: what
/// `obs_transition_start()` does after its second read of the sources. The
/// discriminants are the C's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Admission {
    /// Go on: write the mode, call the plugin and set source B.
    Go = 0,
    /// Return false: the destination is source A and nothing is running.
    Refuse = 1,
    /// Return true: a manual transition is already running to this
    /// destination in manual mode.
    Done = 2,
}

/// Whether a start is refused: the destination is source A and the
/// transition is not active. The first read's gate, and the first rule of
/// [`admit`].
#[must_use]
pub fn refused(same_as_source: bool, active: bool) -> bool {
    let _ = (same_as_source, active);
    todo!("transition_start::refused")
}

/// Whether the recursion walk of the destination's tree runs: there is a
/// destination and it is not the transition itself (`recursive`, the
/// walk's start value).
#[must_use]
pub fn walks(has_dest: bool, recursive: bool) -> bool {
    let _ = (has_dest, recursive);
    todo!("transition_start::walks")
}

/// The second admission of `obs_transition_start()`: [`Admission::Refuse`]
/// as [`refused`]; [`Admission::Done`] when active, the requested `mode`
/// is [`MODE_MANUAL`], the mode is the same and the destination is already
/// source B; else [`Admission::Go`].
#[must_use]
pub fn admit(
    same_as_source: bool,
    same_as_dest: bool,
    same_mode: bool,
    mode: i32,
    active: bool,
) -> Admission {
    let _ = (same_as_source, same_as_dest, same_mode, mode, active);
    todo!("transition_start::admit")
}

/// A duration in ms as the start clock's ns: `(uint64_t)ms * 1000000`,
/// which cannot overflow.
#[must_use]
pub fn duration_ns(ms: u32) -> u64 {
    let _ = ms;
    todo!("transition_start::duration_ns")
}

/// Whether `set_source()` restarts the clock: the transition is not
/// active, or the new child is neither the old one nor source A.
#[must_use]
pub fn restart(active: bool, same_as_dest: bool, same_as_source: bool) -> bool {
    let _ = (active, same_as_dest, same_as_source);
    todo!("transition_start::restart")
}

/// Whether `set_source()` adds the new child as an active child: its slot
/// is already active and there is a new child.
#[must_use]
pub fn adds(already_active: bool, has_new: bool) -> bool {
    let _ = (already_active, has_new);
    todo!("transition_start::adds")
}

/// Whether `set_source()` removes the old child as an active child: its
/// slot is already active, there is an old child and the add (if any)
/// worked.
#[must_use]
pub fn removes(already_active: bool, has_old: bool, add_ok: bool) -> bool {
    let _ = (already_active, has_old, add_ok);
    todo!("transition_start::removes")
}

/// Whether `set_source()` calls its admission callback: there is one and
/// the add (if any) worked.
#[must_use]
pub fn calls_back(has_callback: bool, add_ok: bool) -> bool {
    let _ = (has_callback, add_ok);
    todo!("transition_start::calls_back")
}

/// `obs_source_transition_start_unsized()`: whether a transition size
/// leaves the size to the children (`size == 0`). The C asks it for `cx`,
/// and for `cy` only when `cx` is not 0.
#[must_use]
pub fn is_unsized(size: u32) -> bool {
    let _ = size;
    todo!("transition_start::is_unsized")
}

/// Whether a start to no destination sets both transitioning flags itself:
/// no destination, source B was already none and source A is not none.
#[must_use]
pub fn null_dest(has_dest: bool, same_as_dest: bool, same_as_source: bool) -> bool {
    let _ = (has_dest, same_as_dest, same_as_source);
    todo!("transition_start::null_dest")
}

/// Whether `activate_transition()` adds the child: its slot is not active
/// yet.
#[must_use]
pub fn activates(slot_active: bool) -> bool {
    let _ = slot_active;
    todo!("transition_start::activates")
}

/// The index of the active child: 1 (source B) while the transition is
/// active, else 0 (source A).
#[must_use]
pub fn active_index(active: bool) -> usize {
    let _ = active;
    todo!("transition_start::active_index")
}

/// Whether `obs_transition_set_manual_time()` sets source A: the mode is
/// [`MODE_MANUAL`] and `t == 0.0` (an IEEE compare: true for -0.0, false
/// for NaN, as the C's `t == 0.0f`).
#[must_use]
pub fn manual_reset(mode: i32, t: f32) -> bool {
    let _ = (mode, t);
    todo!("transition_start::manual_reset")
}

/// Whether a child goes out (`obs_transition_set()`) or comes in
/// (`copy_source_state()`) as an active child: there is one and its slot is
/// active.
#[must_use]
pub fn child_active(has_child: bool, active: bool) -> bool {
    let _ = (has_child, active);
    todo!("transition_start::child_active")
}
