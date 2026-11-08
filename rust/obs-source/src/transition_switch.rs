//! Safe core of `libobs/obs-source-transition-switch.c.h`: the start and
//! set of a transition, moved out of `libobs/obs-source-transition.c.h`.
//!
//! - [`start`]: `obs_transition_start()`, which refuses a start to source A
//!   of an idle transition and a destination that contains the transition,
//!   finishes a running transition ([`set`] to its source B), admits the
//!   start, writes the mode, calls the plugin's `transition_start`, hands
//!   the destination to source B ([`set_source`] with
//!   [`activate_transition`]), signals the start and recalculates the size;
//! - [`set`]: `obs_transition_set()`, which refuses a source that contains
//!   the transition, signals the stop of a running transition, empties both
//!   slots, puts the new source in slot A and makes it an active child;
//! - [`set_source`], [`activate_transition`], [`write_start_clock`] and
//!   [`recursive`] (`check_transition_recursion()` and the walk around it):
//!   the C's file-local helpers.
//!
//! Each is generic over a [`Transition`]: one method per field read or
//! write, per lock and unlock call and per call into the rest of libobs
//! (`obs_source_get_ref()`, `obs_source_release()`, the active-child add and
//! remove, `obs_source_enum_full_tree()`, the plugin's `transition_start`,
//! `obs_source_dosignal()`, `os_gettime_ns()`, the size and matrix
//! recalculation and the two warnings), so a function makes the C's calls in
//! the C's order and under the C's locks. The decisions are the rules ported
//! with the transition start checks ([`crate::transition_start`]), asked
//! where the C asks them.
//!
//! The C shims live in the `obs-source-seam` crate, which implements
//! [`Transition`] over `libobs/obs-source-transition-seam.h` and the libobs
//! calls; only libobs-rust links it, so this crate calls nothing in libobs.
//!
//! Nothing is held across a call that can re-enter: the plugin's
//! `transition_start` can call [`start`] or [`set`] on the same transition,
//! so every field is read again, under the lock, after it returns, as the C
//! reads it. [`set_source`] adds the new child as an active child with
//! `transition_mutex` held, as the C does (the recursion check before it is
//! what keeps that add from walking back into this transition).

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

use crate::transition_start::{self as start_rules, Admission};

/// The name `obs_transition_start()` hands `transition_valid()`, which logs
/// it for a NULL source.
pub const START_LOG_NAME: &CStr = c"obs_transition_start";

/// The name `obs_transition_set()` hands `transition_valid()`.
pub const SET_LOG_NAME: &CStr = c"obs_transition_set";

/// The warning `obs_transition_start()` logs (`LOG_WARNING`) when its
/// destination contains the transition, before anything changes, and when
/// source B refuses it: the format, with the transition's name, then the
/// destination's.
pub const START_RECURSION_WARNING: &CStr =
    c"obs_transition_start: transition '%s' has recursive destination '%s'";

/// The warning `obs_transition_set()` logs (`LOG_WARNING`) when its source
/// contains the transition: the format, with the transition's name, then the
/// source's.
pub const SET_RECURSION_WARNING: &CStr =
    c"obs_transition_set: cannot set transition '%s' to source '%s' due to infinite recursion";

/// The signals of a start: the core handler's, then the transition's.
pub const START_SIGNALS: (&CStr, &CStr) = (c"source_transition_start", c"transition_start");

/// The signals [`set`] sends when the transition was running.
pub const STOP_SIGNALS: (&CStr, &CStr) = (c"source_transition_stop", c"transition_stop");

/// `OBS_TRANSITION_SOURCE_A`, the slot a [`set`] fills.
pub const SOURCE_A: usize = 0;

/// `OBS_TRANSITION_SOURCE_B`, the slot a [`start`] fills.
pub const SOURCE_B: usize = 1;

/// The fields, locks and libobs calls of a transition that its start and set
/// use: in libobs, the transition seam (`obs-source-transition-seam.h`) and
/// the libobs functions, one call per method. Every method takes `&self`, as
/// the C writes through its `obs_source_t *`, and none holds anything across
/// a call: the plugin's start can re-enter.
pub trait Transition {
    /// A source other than the transition (or the transition itself, when a
    /// caller hands it in), as a slot holds it.
    type Child: Copy + PartialEq;

    /// `transition_valid(source, func)`: false for no source (the C logs
    /// `func` at debug level) and for one that is not a transition.
    fn valid(source: Option<&Self>, func: &'static CStr) -> bool;
    /// Whether `child` is this transition (`child == transition`).
    fn is(&self, child: Self::Child) -> bool;
    /// `transition_active()`: the audio flag, or (when it is clear) the
    /// video flag.
    fn active(&self) -> bool;

    /// `transition_sources[idx]`, `None` for an empty slot.
    fn source(&self, idx: usize) -> Option<Self::Child>;
    /// Writes `transition_sources[idx]`.
    fn set_source(&self, idx: usize, value: Option<Self::Child>);
    /// `transition_source_active[idx]`.
    fn source_active(&self, idx: usize) -> bool;
    /// Writes `transition_source_active[idx]`.
    fn set_source_active(&self, idx: usize, value: bool);
    /// `transitioning_video` (an atomic load).
    fn transitioning_video(&self) -> bool;
    /// Writes `transitioning_video` (an atomic store).
    fn set_transitioning_video(&self, value: bool);
    /// `transitioning_audio` (an atomic load).
    fn transitioning_audio(&self) -> bool;
    /// Writes `transitioning_audio` (an atomic store).
    fn set_transitioning_audio(&self, value: bool);
    /// `transition_mode`, as its `int`.
    fn mode(&self) -> i32;
    /// Writes `transition_mode`.
    fn set_mode(&self, value: i32);
    /// Writes `transition_manual_val`.
    fn set_manual_val(&self, value: f32);
    /// Writes `transition_manual_target`.
    fn set_manual_target(&self, value: f32);
    /// Writes `transition_start_time`.
    fn set_start_time(&self, value: u64);
    /// Writes `transition_duration`.
    fn set_duration(&self, value: u64);
    /// `transition_use_fixed_duration`.
    fn use_fixed_duration(&self) -> bool;
    /// `transition_fixed_duration`, in ms.
    fn fixed_duration(&self) -> u32;
    /// `transition_cx` (an atomic load).
    fn cx(&self) -> u32;
    /// `transition_cy` (an atomic load).
    fn cy(&self) -> u32;

    /// Locks `transition_mutex`.
    fn lock(&self);
    /// Unlocks `transition_mutex`.
    fn unlock(&self);

    /// The type's `transition_start` callback on the context data, when it
    /// has one. It can call [`start`] or [`set`] on this transition.
    fn call_start(&self);
    /// `obs_source_get_ref(child)`: `None` for no child and for one whose
    /// last reference is gone.
    fn get_ref(&self, child: Option<Self::Child>) -> Option<Self::Child>;
    /// `obs_source_release(child)`, which does nothing for `None`.
    fn release(&self, child: Option<Self::Child>);
    /// `obs_source_add_active_child(self, child)`: false for no child, and
    /// when `child` is `self` or contains it.
    fn add_active_child(&self, child: Option<Self::Child>) -> bool;
    /// `obs_source_remove_active_child(self, child)`.
    fn remove_active_child(&self, child: Self::Child);
    /// `obs_source_enum_full_tree(root, ...)`: calls `visit` with each
    /// parent and child of `root`'s tree. `visit` runs with the tree's
    /// locks held, so it only compares.
    fn enum_full_tree(&self, root: Self::Child, visit: &mut dyn FnMut(Self::Child, Self::Child));
    /// `os_gettime_ns()`.
    fn now(&self) -> u64;
    /// `obs_source_dosignal(self, signal_obs, signal_source)`.
    fn dosignal(&self, signal_obs: &'static CStr, signal_source: &'static CStr);
    /// `transition_recalculate_size(self)`.
    fn recalculate_size(&self);
    /// `transition_recalculate_matrices(self)`.
    fn recalculate_matrices(&self);
    /// Logs `format` (`LOG_WARNING`) with `obs_source_get_name()` of `self`
    /// and of `other` (NULL's name for `None`).
    fn warn(&self, format: &'static CStr, other: Option<Self::Child>);
}

/// `struct transition_start_clock`: a start's clock, decided by
/// [`set_source`] from the state its destination admission replaces (after
/// any plugin re-entry) and written only once that admission succeeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartClock {
    /// The duration the start asks for, in ns.
    pub duration: u64,
    /// The new child is source A.
    pub same_as_source: bool,
    /// The new child is the one already in the slot.
    pub same_as_dest: bool,
    /// The clock restarts ([`crate::transition_start::restart`]).
    pub restart: bool,
}

impl StartClock {
    /// A clock for a start of `duration` ns, its decisions not made yet.
    #[must_use]
    pub const fn new(duration: u64) -> Self {
        Self {
            duration,
            same_as_source: false,
            same_as_dest: false,
            restart: false,
        }
    }
}

/// The admission callback [`set_source`] calls once the active-child
/// handover worked, with the slot and the new child; false refuses it.
pub type Admit<T> = fn(&T, usize, Option<<T as Transition>::Child>) -> bool;

/// `write_start_clock()`: when `clock` restarts, the start time (now) and
/// then the duration.
pub fn write_start_clock<T: Transition>(transition: &T, clock: &StartClock) {
    if clock.restart {
        transition.set_start_time(transition.now());
        transition.set_duration(clock.duration);
    }
}

/// `set_source()`: hands slot `idx` to `new_child`.
///
/// Takes a reference on the new child, then, under the lock, decides the
/// clock (`same_as_source`, `same_as_dest`, `restart`). The same child
/// again writes the clock and keeps everything else. Otherwise an active
/// slot adds the new child as an active child (lock held) and then removes
/// the old one when the add worked, and `callback` admits the new child;
/// once admitted, the clock is written and the slot stored. After the
/// unlock, a refused child's reference goes; an admitted one recalculates
/// the size and matrices when the transition has no size of its own and
/// releases the old child. Returns whether it was admitted.
pub fn set_source<T: Transition>(
    transition: &T,
    idx: usize,
    new_child: Option<T::Child>,
    callback: Option<Admit<T>>,
    clock: &mut StartClock,
) -> bool {
    let mut new_child = new_child;
    let mut add_success = true;

    if new_child.is_some() {
        new_child = transition.get_ref(new_child);
    }

    transition.lock();

    let old_child = transition.source(idx);

    clock.same_as_source = new_child == transition.source(SOURCE_A);
    clock.same_as_dest = new_child == old_child;
    clock.restart = start_rules::restart(
        transition.transitioning_audio() || transition.transitioning_video(),
        clock.same_as_dest,
        clock.same_as_source,
    );

    if new_child == old_child {
        write_start_clock(transition, clock);
        transition.unlock();
        transition.release(new_child);
        return true;
    }

    let already_active = transition.source_active(idx);

    if start_rules::adds(already_active, new_child.is_some()) {
        add_success = transition.add_active_child(new_child);
    }
    if start_rules::removes(already_active, old_child.is_some(), add_success)
        && let Some(old) = old_child
    {
        transition.remove_active_child(old);
    }

    if start_rules::calls_back(callback.is_some(), add_success)
        && let Some(admit) = callback
    {
        add_success = admit(transition, idx, new_child);
    }

    if add_success {
        write_start_clock(transition, clock);
        transition.set_source(idx, new_child);
    }

    transition.unlock();

    if !add_success {
        transition.release(new_child);
        return false;
    }

    if start_rules::is_unsized(transition.cx()) || start_rules::is_unsized(transition.cy()) {
        transition.recalculate_size();
        transition.recalculate_matrices();
    }

    transition.release(old_child);
    true
}

/// `activate_transition()`, [`set_source`]'s callback for a start: adds
/// `child` as an active child when slot `idx` is not active yet (false when
/// that fails) and marks the slot active, then sets both running flags.
pub fn activate_transition<T: Transition>(
    transition: &T,
    idx: usize,
    child: Option<T::Child>,
) -> bool {
    if start_rules::activates(transition.source_active(idx)) {
        if !transition.add_active_child(child) {
            return false;
        }

        transition.set_source_active(idx, true);
    }

    transition.set_transitioning_video(true);
    transition.set_transitioning_audio(true);
    true
}

/// The recursion check of the start and the set: `dest` is the transition,
/// or (when [`crate::transition_start::walks`]) a parent or child of
/// `dest`'s full tree is (`check_transition_recursion()`).
pub fn recursive<T: Transition>(transition: &T, dest: Option<T::Child>) -> bool {
    let mut recursive = dest.is_some_and(|d| transition.is(d));
    if start_rules::walks(dest.is_some(), recursive)
        && let Some(root) = dest
    {
        transition.enum_full_tree(root, &mut |parent, child| {
            if transition.is(parent) || transition.is(child) {
                recursive = true;
            }
        });
    }
    recursive
}

/// `obs_transition_start()`: starts `transition` to `dest` in `mode` for
/// `duration_ms` (the fixed duration instead, when it is enabled). Returns
/// whether the transition runs to `dest`.
pub fn start<T: Transition>(
    transition: Option<&T>,
    mode: i32,
    duration_ms: u32,
    dest: Option<T::Child>,
) -> bool {
    if !T::valid(transition, START_LOG_NAME) {
        return false;
    }
    let Some(transition) = transition else {
        return false;
    };
    let mut duration_ms = duration_ms;

    transition.lock();
    let same_as_source = dest == transition.source(SOURCE_A);
    let active = transition.active();
    transition.unlock();
    if start_rules::refused(same_as_source, active) {
        return false;
    }

    // The destination walk can visit this transition. Check before child
    // activation holds its mutex, and before changing state or calling
    // plugins.
    if recursive(transition, dest) {
        transition.warn(START_RECURSION_WARNING, dest);
        return false;
    }

    if transition.active() {
        set(Some(transition), transition.source(SOURCE_B));
    }

    transition.lock();
    let same_as_source = dest == transition.source(SOURCE_A);
    let same_as_dest = dest == transition.source(SOURCE_B);
    let same_mode = mode == transition.mode();
    let active = transition.active();
    transition.unlock();

    match start_rules::admit(same_as_source, same_as_dest, same_mode, mode, active) {
        Admission::Refuse => return false,
        Admission::Done => return true,
        Admission::Go => {}
    }

    // Written before the plugin callback: a start the callback makes on
    // this transition replaces the mode, and it stays even if this start
    // then completes (unchanged upstream behaviour).
    transition.lock();
    transition.set_mode(mode);
    transition.set_manual_val(0.0);
    transition.set_manual_target(0.0);
    transition.unlock();

    transition.call_start();

    if transition.use_fixed_duration() {
        duration_ms = transition.fixed_duration();
    }

    // Plugin re-entry may have changed the destination since the
    // preflight, or started this transition itself. Child admission checks
    // again without descending into our held mutex, and the clock is
    // decided from the state being replaced, so a rejected start leaves a
    // nested start's clock intact.
    let mut clock = StartClock::new(start_rules::duration_ns(duration_ms));
    if !set_source(
        transition,
        SOURCE_B,
        dest,
        Some(activate_transition::<T>),
        &mut clock,
    ) {
        transition.warn(START_RECURSION_WARNING, dest);
        return false;
    }
    if start_rules::null_dest(dest.is_some(), clock.same_as_dest, clock.same_as_source) {
        transition.set_transitioning_video(true);
        transition.set_transitioning_audio(true);
    }

    transition.dosignal(START_SIGNALS.0, START_SIGNALS.1);

    transition.recalculate_size();
    transition.recalculate_matrices();

    true
}

/// `obs_transition_set()`: puts `source` in slot A of an idle transition.
///
/// Refuses (with [`SET_RECURSION_WARNING`]) a source that is the
/// transition or contains it, before changing anything: a walk of its
/// active tree would enter the transition again and lock its mutex twice.
/// Otherwise takes a reference on the source, signals the stop of a running
/// transition, and under the lock empties both slots, puts the source in
/// slot A (active), stops the transition and resets the manual value and
/// target. After the unlock, each old child is removed from the active
/// children (when its slot was active) and released, slot A first; then the
/// source is added as an active child, and when that fails (a source that
/// contains the transition) slot A is made inactive again, if it still
/// holds the source, so the next set does not remove a reference another
/// parent holds.
pub fn set<T: Transition>(transition: Option<&T>, source: Option<T::Child>) {
    if !T::valid(transition, SET_LOG_NAME) {
        return;
    }
    let Some(transition) = transition else {
        return;
    };
    let mut s = [None; 2];
    let mut active = [false; 2];

    if recursive(transition, source) {
        transition.warn(SET_RECURSION_WARNING, source);
        return;
    }

    let source = transition.get_ref(source);

    if transition.active() {
        transition.dosignal(STOP_SIGNALS.0, STOP_SIGNALS.1);
    }

    transition.lock();
    for (i, (child, was_active)) in s.iter_mut().zip(active.iter_mut()).enumerate() {
        *child = transition.source(i);
        *was_active = transition.source_active(i);
        transition.set_source(i, None);
        transition.set_source_active(i, false);
    }
    transition.set_source_active(SOURCE_A, true);
    transition.set_source(SOURCE_A, source);
    transition.set_transitioning_video(false);
    transition.set_transitioning_audio(false);
    transition.set_manual_val(0.0);
    transition.set_manual_target(0.0);
    transition.unlock();

    for (child, was_active) in s.into_iter().zip(active) {
        if start_rules::child_active(child.is_some(), was_active)
            && let Some(c) = child
        {
            transition.remove_active_child(c);
        }
        transition.release(child);
    }

    if let Some(src) = source
        && !transition.add_active_child(source)
    {
        transition.lock();
        if transition.source(SOURCE_A) == Some(src) {
            transition.set_source_active(SOURCE_A, false);
        }
        transition.unlock();
    }
}
