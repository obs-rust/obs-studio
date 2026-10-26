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

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

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
pub const fn refused(same_as_source: bool, active: bool) -> bool {
    same_as_source && !active
}

/// Whether the recursion walk of the destination's tree runs: there is a
/// destination and it is not the transition itself (`recursive`, the
/// walk's start value).
#[must_use]
pub const fn walks(has_dest: bool, recursive: bool) -> bool {
    has_dest && !recursive
}

/// The second admission of `obs_transition_start()`: [`Admission::Refuse`]
/// as [`refused`]; [`Admission::Done`] when active, the requested `mode`
/// is [`MODE_MANUAL`], the mode is the same and the destination is already
/// source B; else [`Admission::Go`].
#[must_use]
pub const fn admit(
    same_as_source: bool,
    same_as_dest: bool,
    same_mode: bool,
    mode: i32,
    active: bool,
) -> Admission {
    if refused(same_as_source, active) {
        Admission::Refuse
    } else if active && mode == MODE_MANUAL && same_mode && same_as_dest {
        Admission::Done
    } else {
        Admission::Go
    }
}

/// A duration in ms as the start clock's ns: `(uint64_t)ms * 1000000`,
/// which cannot overflow.
#[must_use]
pub fn duration_ns(ms: u32) -> u64 {
    // u32::MAX * 1e6 is below 2^52: the widened product never wraps.
    u64::from(ms).wrapping_mul(1_000_000)
}

/// Whether `set_source()` restarts the clock: the transition is not
/// active, or the new child is neither the old one nor source A.
#[must_use]
pub const fn restart(active: bool, same_as_dest: bool, same_as_source: bool) -> bool {
    !active || (!same_as_dest && !same_as_source)
}

/// Whether `set_source()` adds the new child as an active child: its slot
/// is already active and there is a new child.
#[must_use]
pub const fn adds(already_active: bool, has_new: bool) -> bool {
    already_active && has_new
}

/// Whether `set_source()` removes the old child as an active child: its
/// slot is already active, there is an old child and the add (if any)
/// worked.
#[must_use]
pub const fn removes(already_active: bool, has_old: bool, add_ok: bool) -> bool {
    already_active && has_old && add_ok
}

/// Whether `set_source()` calls its admission callback: there is one and
/// the add (if any) worked.
#[must_use]
pub const fn calls_back(has_callback: bool, add_ok: bool) -> bool {
    has_callback && add_ok
}

/// `obs_source_transition_start_unsized()`: whether a transition size
/// leaves the size to the children (`size == 0`). The C asks it for `cx`,
/// and for `cy` only when `cx` is not 0.
#[must_use]
pub const fn is_unsized(size: u32) -> bool {
    size == 0
}

/// Whether a start to no destination sets both transitioning flags itself:
/// no destination, source B was already none and source A is not none.
#[must_use]
pub const fn null_dest(has_dest: bool, same_as_dest: bool, same_as_source: bool) -> bool {
    !has_dest && same_as_dest && !same_as_source
}

/// Whether `activate_transition()` adds the child: its slot is not active
/// yet.
#[must_use]
pub const fn activates(slot_active: bool) -> bool {
    !slot_active
}

/// The index of the active child: 1 (source B) while the transition is
/// active, else 0 (source A).
#[must_use]
pub const fn active_index(active: bool) -> usize {
    if active { 1 } else { 0 }
}

/// Whether `obs_transition_set_manual_time()` sets source A: the mode is
/// [`MODE_MANUAL`] and `t == 0.0` (an IEEE compare: true for -0.0, false
/// for NaN, as the C's `t == 0.0f`).
#[must_use]
pub fn manual_reset(mode: i32, t: f32) -> bool {
    mode == MODE_MANUAL && t == 0.0
}

/// Whether a child goes out (`obs_transition_set()`) or comes in
/// (`copy_source_state()`) as an active child: there is one and its slot is
/// active.
#[must_use]
pub const fn child_active(has_child: bool, active: bool) -> bool {
    has_child && active
}
