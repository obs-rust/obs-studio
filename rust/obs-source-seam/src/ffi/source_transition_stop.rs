//! C ABI for `libobs/obs-source-transition-stop.c.h`, the clock read and
//! stop of a transition: `obs_transition_get_time()` and
//! `obs_transition_force_stop()`, both `EXPORT` through `obs.h`
//! (`rust-exports.d`), and the four helpers the video and audio renders of
//! `obs-source-transition.c.h` (still C) call, declared in
//! `libobs/obs-source-transition-stop.h` without `EXPORT` (`rust-hidden.d`):
//! `transition_calc_time()`, `transition_get_video_time()`,
//! `transition_stop()` and `transition_handle_stop()`.
//!
//! The functions are the safe core's (`obs_source::transition_stop`), run on
//! [`Seam`], which implements its `Transition` over the transition seam
//! (`libobs/obs-source-transition-seam.h`, still C in `libobs.core-source`):
//! `transition_valid()`, one accessor per field, the lock and unlock of
//! `transition_mutex` and the plugin's `transition_stop`
//! (`obs_source_transition_seam_call_stop()`), one seam call per method.
//! `struct obs_source` is never mirrored. The calls into the rest of libobs
//! (the video frame time through `obs_source_transition_seam_get_video_time()`,
//! the signals through `obs_source_transition_seam_dosignal()`,
//! `obs_source_remove_active_child()` and `obs_source_release()`) go
//! through [`Calls`]: [`Libobs`] calls them, and the parity test hands in
//! the oracle's recording stand-ins, as the C oracle calls them.
//!
//! The locks are the callers', as in the C: the renders call
//! [`transition_calc_time`], [`transition_get_video_time`] and
//! [`transition_stop`] with `transition_mutex` held, so those take no lock;
//! [`transition_get_video_time`] reads the video frame time, which takes
//! the video stats mutex, and takes nothing else between the two.
//! [`transition_handle_stop`] runs after the renders let the lock go.
//!
//! Intentional difference from C: [`obs_transition_get_time`] and
//! [`obs_transition_force_stop`] ask `transition_valid()` first, with their
//! own names: no transition or a source that is not one gets 0 and nothing,
//! where the C crashes or reads that source's unused fields (and calls back
//! and signals a stop on it). The four hidden helpers check nothing, as in
//! the C: their callers already did.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::marker::PhantomData;
use std::ptr;

use obs_source::transition_stop::{self as core, Transition};

unsafe extern "C" {
    fn transition_valid(transition: *const c_void, func: *const c_char) -> bool;

    fn obs_source_transition_seam_get_sources(transition: *const c_void, idx: usize)
    -> *mut c_void;
    fn obs_source_transition_seam_set_sources(
        transition: *mut c_void,
        idx: usize,
        value: *mut c_void,
    );
    fn obs_source_transition_seam_get_source_active(transition: *const c_void, idx: usize) -> bool;
    fn obs_source_transition_seam_set_source_active(
        transition: *mut c_void,
        idx: usize,
        value: bool,
    );
    fn obs_source_transition_seam_get_mode(transition: *const c_void) -> c_int;
    fn obs_source_transition_seam_get_manual_val(transition: *const c_void) -> f32;
    fn obs_source_transition_seam_get_start_time(transition: *const c_void) -> u64;
    fn obs_source_transition_seam_get_duration(transition: *const c_void) -> u64;
    fn obs_source_transition_seam_lock(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_unlock(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_call_stop(transition: *mut c_void) -> bool;
}

/// The calls the clock read and stop make into the rest of libobs, one C
/// function each.
///
/// # Safety
/// Each is called with the pointers the C passes: live sources (a child or
/// NULL where the C allows NULL).
pub unsafe trait Calls {
    /// `obs_source_transition_seam_get_video_time()`:
    /// `obs_video_stats_frame_time(&obs->video.stats)`.
    ///
    /// # Safety
    /// libobs is started (the core's video stats exist).
    unsafe fn video_time() -> u64;
    /// `obs_source_transition_seam_dosignal(transition, signal_obs,
    /// signal_source)`.
    ///
    /// # Safety
    /// `transition` is a live `obs_source_t`, the names NUL-terminated.
    unsafe fn dosignal(
        transition: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    );
    /// `obs_source_remove_active_child(parent, child)`.
    ///
    /// # Safety
    /// Both are live `obs_source_t`.
    unsafe fn remove_active_child(parent: *mut c_void, child: *mut c_void);
    /// `obs_source_release(source)`.
    ///
    /// # Safety
    /// `source` is NULL or a live `obs_source_t` the caller holds a
    /// reference on.
    unsafe fn release(source: *mut c_void);
}

/// [`Calls`] through libobs's own functions.
pub enum Libobs {}

unsafe extern "C" {
    fn obs_source_transition_seam_get_video_time() -> u64;
    fn obs_source_transition_seam_dosignal(
        transition: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    );
    fn obs_source_remove_active_child(parent: *mut c_void, child: *mut c_void);
    fn obs_source_release(source: *mut c_void);
}

// SAFETY: each method is the one libobs function of its name, called with
// the caller's pointers.
unsafe impl Calls for Libobs {
    unsafe fn video_time() -> u64 {
        // SAFETY: see the trait.
        unsafe { obs_source_transition_seam_get_video_time() }
    }
    unsafe fn dosignal(
        transition: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    ) {
        // SAFETY: see the trait.
        unsafe { obs_source_transition_seam_dosignal(transition, signal_obs, signal_source) }
    }
    unsafe fn remove_active_child(parent: *mut c_void, child: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { obs_source_remove_active_child(parent, child) }
    }
    unsafe fn release(source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { obs_source_release(source) }
    }
}

/// A source a slot holds, as its pointer: never dereferenced here, only
/// passed to libobs and compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChildSource(*mut c_void);

impl ChildSource {
    /// The source's pointer, as libobs takes it.
    pub(crate) fn as_ptr(self) -> *mut c_void {
        self.0
    }

    /// `None` for NULL.
    pub(crate) fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        child(ptr)
    }
}

/// A live `obs_source_t` a shim was handed, with the libobs calls `L` makes
/// for it.
pub struct Seam<L>(*mut c_void, PhantomData<L>);

impl<L> Seam<L> {
    pub(crate) fn new(source: *mut c_void) -> Self {
        Self(source, PhantomData)
    }

    /// The transition's pointer, as the seam takes it.
    pub(crate) fn as_ptr(&self) -> *mut c_void {
        self.0
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
// libobs function that names it. `idx` is a slot, 0 or 1.
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
    fn set_source(&self, idx: usize, value: Option<ChildSource>) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_sources(self.0, idx, child_ptr(value)) }
    }
    fn source_active(&self, idx: usize) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_source_active(self.0, idx) }
    }
    fn set_source_active(&self, idx: usize, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_source_active(self.0, idx, value) }
    }
    fn mode(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_mode(self.0) }
    }
    fn manual_val(&self) -> f32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_manual_val(self.0) }
    }
    fn start_time(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_start_time(self.0) }
    }
    fn duration(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_duration(self.0) }
    }
    fn lock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_lock(self.0) };
    }
    fn unlock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_unlock(self.0) };
    }
    fn video_time(&self) -> u64 {
        // SAFETY: see the impl; the caller's libobs is up.
        unsafe { L::video_time() }
    }
    fn call_stop(&self) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_call_stop(self.0) };
    }
    fn dosignal(&self, signal_obs: &'static CStr, signal_source: &'static CStr) {
        // SAFETY: see the impl; static names.
        unsafe { L::dosignal(self.0, signal_obs.as_ptr(), signal_source.as_ptr()) }
    }
    fn remove_active_child(&self, child_source: ChildSource) {
        // SAFETY: see the impl.
        unsafe { L::remove_active_child(self.0, child_source.0) }
    }
    fn release(&self, child_source: Option<ChildSource>) {
        // SAFETY: see the impl; NULL as the C passes it.
        unsafe { L::release(child_ptr(child_source)) }
    }
}

/// `transition_calc_time()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is a live transition `obs_source_t`, its `transition_mutex`
/// held by the caller.
pub unsafe fn calc_time<L: Calls>(transition: *mut c_void, ts: u64) -> f32 {
    core::calc_time(&Seam::<L>::new(transition), ts)
}

/// `transition_get_video_time()` with the libobs calls of `L`.
///
/// # Safety
/// As for [`calc_time`].
pub unsafe fn get_video_time<L: Calls>(transition: *mut c_void) -> f32 {
    core::get_video_time(&Seam::<L>::new(transition))
}

/// `obs_transition_get_time()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`, whose `transition_mutex`
/// the caller does not hold.
pub unsafe fn get_time<L: Calls>(transition: *mut c_void) -> f32 {
    let seam = (!transition.is_null()).then(|| Seam::<L>::new(transition));
    core::get_time(seam.as_ref())
}

/// `transition_stop()` with the libobs calls of `L`.
///
/// # Safety
/// As for [`calc_time`].
pub unsafe fn stop<L: Calls>(transition: *mut c_void) {
    core::stop(&Seam::<L>::new(transition));
}

/// `transition_handle_stop()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is a live transition `obs_source_t`.
pub unsafe fn handle_stop<L: Calls>(transition: *mut c_void) {
    core::handle_stop(&Seam::<L>::new(transition));
}

/// `obs_transition_force_stop()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
pub unsafe fn force_stop<L: Calls>(transition: *mut c_void) {
    let seam = (!transition.is_null()).then(|| Seam::<L>::new(transition));
    core::force_stop(seam.as_ref());
}

/// `float transition_calc_time(obs_source_t *transition, uint64_t ts)`.
///
/// # Safety
/// As for [`calc_time`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn transition_calc_time(transition: *mut c_void, ts: u64) -> f32 {
    // SAFETY: the caller's contract.
    unsafe { calc_time::<Libobs>(transition, ts) }
}

/// `float transition_get_video_time(obs_source_t *transition)`.
///
/// # Safety
/// As for [`calc_time`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn transition_get_video_time(transition: *mut c_void) -> f32 {
    // SAFETY: the caller's contract.
    unsafe { get_video_time::<Libobs>(transition) }
}

/// `float obs_transition_get_time(obs_source_t *transition)`.
///
/// # Safety
/// As for [`get_time`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_get_time(transition: *mut c_void) -> f32 {
    // SAFETY: the caller's contract.
    unsafe { get_time::<Libobs>(transition) }
}

/// `void transition_stop(obs_source_t *transition)`.
///
/// # Safety
/// As for [`calc_time`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn transition_stop(transition: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { stop::<Libobs>(transition) }
}

/// `void transition_handle_stop(obs_source_t *transition)`.
///
/// # Safety
/// As for [`handle_stop`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn transition_handle_stop(transition: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { handle_stop::<Libobs>(transition) }
}

/// `void obs_transition_force_stop(obs_source_t *transition)`.
///
/// # Safety
/// As for [`force_stop`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_force_stop(transition: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { force_stop::<Libobs>(transition) }
}
