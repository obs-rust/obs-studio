//! C ABI for `libobs/obs-source-transition-access.c.h`, the getters and
//! manual controls of a transition: `obs_transition_get_source()`,
//! `obs_transition_get_active_source()`,
//! `obs_transition_set_manual_torque()` and
//! `obs_transition_set_manual_time()`. All four are `EXPORT` through
//! `obs.h` (`rust-exports.d`).
//!
//! The functions are the safe core's (`obs_source::transition_access`), run
//! on [`Seam`], which implements its `Transition` over the transition seam
//! (`libobs/obs-source-transition-seam.h`, still C in `libobs.core-source`):
//! `transition_valid()`, one accessor per field, and the lock and unlock of
//! `transition_mutex`, one seam call per method. `struct obs_source` is
//! never mirrored. The calls into the rest of libobs
//! (`obs_source_get_ref()` and `obs_transition_set()`) go through
//! [`Calls`]: [`Libobs`] calls the C symbols, and the parity test hands in
//! the oracle's recording stand-ins, as the C oracle calls them.
//!
//! The lock is the C's, taken where it takes it: the getters read the slot
//! and take the reference under `transition_mutex`, the setters write under
//! it, and `obs_transition_set_manual_time()` reads slot A for
//! `obs_transition_set()` after the unlock. The seam takes no lock of its
//! own. `obs_transition_get_source()` hands the caller's
//! `enum obs_transition_target` to the seam as a `size_t`, unchecked, as
//! the C indexes with it.
//!
//! One intentional difference: both setters return first unless
//! `transition_valid()` holds, where the C checks nothing (a NULL transition
//! crashes on the lock, and a source that is not a transition has its
//! fields written).

use std::ffi::{CStr, c_char, c_int, c_void};
use std::marker::PhantomData;
use std::ptr;

use obs_source::transition_access::{self as core, Transition};

unsafe extern "C" {
    fn transition_valid(transition: *const c_void, func: *const c_char) -> bool;

    fn obs_source_transition_seam_get_sources(transition: *const c_void, idx: usize)
    -> *mut c_void;
    fn obs_source_transition_seam_get_transitioning_video(transition: *const c_void) -> bool;
    fn obs_source_transition_seam_get_transitioning_audio(transition: *const c_void) -> bool;
    fn obs_source_transition_seam_get_mode(transition: *const c_void) -> c_int;
    fn obs_source_transition_seam_set_manual_torque(transition: *mut c_void, value: f32);
    fn obs_source_transition_seam_set_manual_clamp(transition: *mut c_void, value: f32);
    fn obs_source_transition_seam_set_manual_target(transition: *mut c_void, value: f32);
    fn obs_source_transition_seam_lock(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_unlock(transition: *mut c_void) -> c_int;
}

/// The calls the getters and manual controls make into the rest of libobs,
/// one C function each.
///
/// # Safety
/// Each is called with the pointers the C passes: a live transition, and a
/// child or NULL where the C allows NULL.
pub unsafe trait Calls {
    /// `obs_source_get_ref(source)`.
    ///
    /// # Safety
    /// `source` is NULL or a live `obs_source_t`.
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void;
    /// `obs_transition_set(transition, source)`.
    ///
    /// # Safety
    /// `transition` is a live `obs_source_t`, `source` NULL or one.
    unsafe fn transition_set(transition: *mut c_void, source: *mut c_void);
}

/// [`Calls`] through libobs's own functions.
pub enum Libobs {}

unsafe extern "C" {
    fn obs_source_get_ref(source: *mut c_void) -> *mut c_void;
    fn obs_transition_set(transition: *mut c_void, source: *mut c_void);
}

// SAFETY: each method is the one libobs function of its name, called with
// the caller's pointers.
unsafe impl Calls for Libobs {
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { obs_source_get_ref(source) }
    }
    unsafe fn transition_set(transition: *mut c_void, source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { obs_transition_set(transition, source) }
    }
}

/// A non-NULL child of a transition, read from one of its slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChildSource(*mut c_void);

/// A live, non-NULL `obs_source_t` a shim was handed, with the libobs calls
/// `L` makes for it.
pub struct Seam<L>(*mut c_void, PhantomData<L>);

impl<L> Seam<L> {
    fn new(source: *mut c_void) -> Self {
        Self(source, PhantomData)
    }
}

fn child(ptr: *mut c_void) -> Option<ChildSource> {
    (!ptr.is_null()).then_some(ChildSource(ptr))
}

fn child_ptr(child: Option<ChildSource>) -> *mut c_void {
    child.map_or(ptr::null_mut(), |c| c.0)
}

// SAFETY (every method): `self.0` is the live source the C caller passed,
// each child one the seam read from its slot (live while the slot holds a
// reference on it), and each call is the one seam accessor, lock call or
// libobs function that names it. `idx` is the caller's, passed on as the C
// indexes with it.
impl<L: Calls> Transition for Seam<L> {
    type Child = ChildSource;

    fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
        let ptr = source.map_or(ptr::null(), |s| s.0.cast_const());
        // SAFETY: NULL or the caller's live source, and a static name.
        unsafe { transition_valid(ptr, func.as_ptr()) }
    }
    fn source(&self, idx: usize) -> Option<ChildSource> {
        // SAFETY: see the impl.
        child(unsafe { obs_source_transition_seam_get_sources(self.0, idx) })
    }
    fn transitioning_video(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_transitioning_video(self.0) }
    }
    fn transitioning_audio(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_transitioning_audio(self.0) }
    }
    fn mode(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_mode(self.0) }
    }
    fn set_manual_torque(&self, value: f32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_manual_torque(self.0, value) }
    }
    fn set_manual_clamp(&self, value: f32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_manual_clamp(self.0, value) }
    }
    fn set_manual_target(&self, value: f32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_manual_target(self.0, value) }
    }
    fn lock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_lock(self.0) };
    }
    fn unlock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_unlock(self.0) };
    }
    fn get_ref(&self, child_source: Option<ChildSource>) -> Option<ChildSource> {
        // SAFETY: see the impl; NULL for an empty slot, as the C passes it.
        child(unsafe { L::get_ref(child_ptr(child_source)) })
    }
    fn set(&self, source: Option<ChildSource>) {
        // SAFETY: see the impl; NULL for an empty slot A, as the C passes
        // it.
        unsafe { L::transition_set(self.0, child_ptr(source)) }
    }
}

fn seam<L>(transition: *mut c_void) -> Option<Seam<L>> {
    (!transition.is_null()).then(|| Seam::new(transition))
}

/// The slot index the C makes of an `enum obs_transition_target`:
/// `(size_t)target`, sign-extended as C converts a negative `int`.
fn slot_index(target: c_int) -> usize {
    // A negative int becomes size_t modulo 2^N in C, as in a sign-extending
    // cast here; isize holds every c_int on the targets libobs builds for.
    isize::try_from(target).map_or(usize::MAX, isize::cast_unsigned)
}

/// `obs_transition_get_source()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`; for a transition,
/// `target` is a slot (the C does not check it).
pub unsafe fn get_source<L: Calls>(transition: *mut c_void, target: c_int) -> *mut c_void {
    child_ptr(core::get_source(
        seam::<L>(transition).as_ref(),
        slot_index(target),
    ))
}

/// `obs_transition_get_active_source()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
pub unsafe fn get_active_source<L: Calls>(transition: *mut c_void) -> *mut c_void {
    child_ptr(core::get_active_source(seam::<L>(transition).as_ref()))
}

/// `obs_transition_set_manual_torque()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
pub unsafe fn set_manual_torque<L: Calls>(transition: *mut c_void, torque: f32, clamp: f32) {
    core::set_manual_torque(seam::<L>(transition).as_ref(), torque, clamp);
}

/// `obs_transition_set_manual_time()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
pub unsafe fn set_manual_time<L: Calls>(transition: *mut c_void, t: f32) {
    core::set_manual_time(seam::<L>(transition).as_ref(), t);
}

/// `obs_source_t *obs_transition_get_source(obs_source_t *transition,
/// enum obs_transition_target target)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`; for a transition,
/// `target` is `OBS_TRANSITION_SOURCE_A` or `_B`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_get_source(
    transition: *mut c_void,
    target: c_int,
) -> *mut c_void {
    // SAFETY: the caller's contract.
    unsafe { get_source::<Libobs>(transition, target) }
}

/// `obs_source_t *obs_transition_get_active_source(obs_source_t
/// *transition)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_get_active_source(transition: *mut c_void) -> *mut c_void {
    // SAFETY: the caller's contract.
    unsafe { get_active_source::<Libobs>(transition) }
}

/// `void obs_transition_set_manual_torque(obs_source_t *transition, float
/// torque, float clamp)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_set_manual_torque(
    transition: *mut c_void,
    torque: f32,
    clamp: f32,
) {
    // SAFETY: the caller's contract.
    unsafe { set_manual_torque::<Libobs>(transition, torque, clamp) }
}

/// `void obs_transition_set_manual_time(obs_source_t *transition, float
/// t)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_set_manual_time(transition: *mut c_void, t: f32) {
    // SAFETY: the caller's contract.
    unsafe { set_manual_time::<Libobs>(transition, t) }
}
