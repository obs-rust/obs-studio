//! C ABI for `libobs/obs-source-transition-switch.c.h`, the start and set of
//! a transition: `obs_transition_start()` and `obs_transition_set()`, both
//! `EXPORT` through `obs.h` (`rust-exports.d`).
//!
//! The functions are the safe core's (`obs_source::transition_switch`), run
//! on [`Seam`], which implements its `Transition` over the transition seam
//! (`libobs/obs-source-transition-seam.h`, still C in `libobs.core-source`):
//! `transition_valid()`, `transition_active()`, one accessor per field, the
//! lock and unlock of `transition_mutex` and the plugin's
//! `transition_start` (`obs_source_transition_seam_call_start()`), one seam
//! call per method. `struct obs_source` is never mirrored. The calls into
//! the rest of libobs (`obs_source_get_ref()`, `obs_source_release()`, the
//! active-child add and remove, `obs_source_get_name()` of the warnings,
//! `obs_source_enum_full_tree()`, `os_gettime_ns()`, the signals through
//! `obs_source_transition_seam_dosignal()`, and the size and matrix
//! recalculation, which is the size port's Rust,
//! [`super::source_transition_size`]) go through [`Calls`]: [`Libobs`] calls
//! them, and the parity test hands in the oracle's recording stand-ins, as
//! the C oracle calls them. The warnings are `blog(LOG_WARNING, ...)`, as in
//! the C.
//!
//! [`Seam`] holds only the `obs_source_t *` it was handed, so the plugin's
//! `transition_start`, which can call `obs_transition_start()` or
//! `obs_transition_set()` on the same transition, finds nothing of this call
//! borrowed or cached: the core reads every field again, under the lock,
//! after it returns. The recursion walk's callback ([`visit`]) runs inside
//! `obs_source_enum_full_tree()`, which holds the tree's locks: it hands the
//! parent and child to the core, which only compares them with the
//! transition.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::marker::PhantomData;
use std::ptr;

use obs_source::transition_switch::{self as core, Transition};

use super::source_transition_size;

/// `LOG_WARNING`.
const LOG_WARNING: c_int = 200;

/// `obs_source_enum_proc_t`.
pub type EnumProc = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void);

unsafe extern "C" {
    fn transition_valid(transition: *const c_void, func: *const c_char) -> bool;
    fn transition_active(transition: *mut c_void) -> bool;

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
    fn obs_source_transition_seam_get_transitioning_video(transition: *const c_void) -> bool;
    fn obs_source_transition_seam_set_transitioning_video(transition: *mut c_void, value: bool);
    fn obs_source_transition_seam_get_transitioning_audio(transition: *const c_void) -> bool;
    fn obs_source_transition_seam_set_transitioning_audio(transition: *mut c_void, value: bool);
    fn obs_source_transition_seam_get_mode(transition: *const c_void) -> c_int;
    fn obs_source_transition_seam_set_mode(transition: *mut c_void, value: c_int);
    fn obs_source_transition_seam_set_manual_val(transition: *mut c_void, value: f32);
    fn obs_source_transition_seam_set_manual_target(transition: *mut c_void, value: f32);
    fn obs_source_transition_seam_set_start_time(transition: *mut c_void, value: u64);
    fn obs_source_transition_seam_set_duration(transition: *mut c_void, value: u64);
    fn obs_source_transition_seam_get_use_fixed_duration(transition: *const c_void) -> bool;
    fn obs_source_transition_seam_get_fixed_duration(transition: *const c_void) -> u32;
    fn obs_source_transition_seam_get_cx(transition: *const c_void) -> u32;
    fn obs_source_transition_seam_get_cy(transition: *const c_void) -> u32;
    fn obs_source_transition_seam_lock(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_unlock(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_call_start(transition: *mut c_void) -> bool;

    fn blog(level: c_int, format: *const c_char, ...);
}

/// The calls the start and set make into the rest of libobs, one C function
/// each.
///
/// # Safety
/// Each is called with the pointers the C passes: live sources (a child or
/// NULL where the C allows NULL).
pub unsafe trait Calls {
    /// `obs_source_get_ref(source)`.
    ///
    /// # Safety
    /// `source` is NULL or a live `obs_source_t`.
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void;
    /// `obs_source_release(source)`.
    ///
    /// # Safety
    /// `source` is NULL or a live `obs_source_t` the caller holds a
    /// reference on.
    unsafe fn release(source: *mut c_void);
    /// `obs_source_add_active_child(parent, child)`.
    ///
    /// # Safety
    /// `parent` is a live `obs_source_t`, `child` NULL or one.
    unsafe fn add_active_child(parent: *mut c_void, child: *mut c_void) -> bool;
    /// `obs_source_remove_active_child(parent, child)`.
    ///
    /// # Safety
    /// Both are live `obs_source_t`.
    unsafe fn remove_active_child(parent: *mut c_void, child: *mut c_void);
    /// `obs_source_get_name(source)`.
    ///
    /// # Safety
    /// `source` is NULL or a live `obs_source_t`.
    unsafe fn get_name(source: *const c_void) -> *const c_char;
    /// `obs_source_enum_full_tree(source, callback, param)`.
    ///
    /// # Safety
    /// `source` is a live `obs_source_t` and `callback` takes `param`.
    unsafe fn enum_full_tree(source: *mut c_void, callback: EnumProc, param: *mut c_void);
    /// `os_gettime_ns()`.
    ///
    /// # Safety
    /// None beyond the C's.
    unsafe fn now() -> u64;
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
    /// `transition_recalculate_size(transition)`.
    ///
    /// # Safety
    /// `transition` is a live transition `obs_source_t`.
    unsafe fn recalculate_size(transition: *mut c_void);
    /// `transition_recalculate_matrices(transition)`.
    ///
    /// # Safety
    /// `transition` is a live transition `obs_source_t`.
    unsafe fn recalculate_matrices(transition: *mut c_void);
}

/// [`Calls`] through libobs's own functions, and the size port's Rust.
pub enum Libobs {}

unsafe extern "C" {
    fn obs_source_get_ref(source: *mut c_void) -> *mut c_void;
    fn obs_source_release(source: *mut c_void);
    fn obs_source_add_active_child(parent: *mut c_void, child: *mut c_void) -> bool;
    fn obs_source_remove_active_child(parent: *mut c_void, child: *mut c_void);
    fn obs_source_get_name(source: *const c_void) -> *const c_char;
    fn obs_source_enum_full_tree(source: *mut c_void, callback: EnumProc, param: *mut c_void);
    fn os_gettime_ns() -> u64;
    fn obs_source_transition_seam_dosignal(
        transition: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    );
}

// SAFETY: each method is the one libobs function of its name (or the size
// port's function the C calls), called with the caller's pointers.
unsafe impl Calls for Libobs {
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { obs_source_get_ref(source) }
    }
    unsafe fn release(source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { obs_source_release(source) }
    }
    unsafe fn add_active_child(parent: *mut c_void, child: *mut c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { obs_source_add_active_child(parent, child) }
    }
    unsafe fn remove_active_child(parent: *mut c_void, child: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { obs_source_remove_active_child(parent, child) }
    }
    unsafe fn get_name(source: *const c_void) -> *const c_char {
        // SAFETY: see the trait.
        unsafe { obs_source_get_name(source) }
    }
    unsafe fn enum_full_tree(source: *mut c_void, callback: EnumProc, param: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { obs_source_enum_full_tree(source, callback, param) }
    }
    unsafe fn now() -> u64 {
        // SAFETY: see the trait.
        unsafe { os_gettime_ns() }
    }
    unsafe fn dosignal(
        transition: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    ) {
        // SAFETY: see the trait.
        unsafe { obs_source_transition_seam_dosignal(transition, signal_obs, signal_source) }
    }
    unsafe fn recalculate_size(transition: *mut c_void) {
        // SAFETY: see the trait.
        unsafe {
            source_transition_size::recalculate_size::<source_transition_size::Libobs>(transition)
        }
    }
    unsafe fn recalculate_matrices(transition: *mut c_void) {
        // SAFETY: see the trait.
        unsafe {
            source_transition_size::recalculate_matrices::<source_transition_size::Libobs>(
                transition,
            );
        }
    }
}

/// A source a slot holds, or a caller handed in, as its pointer: never
/// dereferenced here, only passed to libobs and compared.
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

/// The walk's `obs_source_enum_proc_t`, `check_transition_recursion()`'s
/// place: hands the parent and child to the core's comparison. It runs with
/// the tree's locks held and calls nothing in libobs.
///
/// # Safety
/// `param` is the `&mut dyn FnMut` [`Seam::enum_full_tree`] passed.
unsafe extern "C" fn visit(parent: *mut c_void, child_source: *mut c_void, param: *mut c_void) {
    // SAFETY: the walk hands back the param enum_full_tree passed, a live
    // `&mut &mut dyn FnMut` on its stack for the whole walk.
    let visit = unsafe { &mut *param.cast::<&mut dyn FnMut(ChildSource, ChildSource)>() };
    visit(ChildSource(parent), ChildSource(child_source));
}

// SAFETY (every method): `self.0` is the live source the C caller passed,
// each child one the seam read from its slot or the caller passed (live
// while a slot or the caller holds a reference on it), and each call is the
// one seam accessor, lock call or libobs function that names it. `idx` is a
// slot, 0 or 1.
impl<L: Calls> Transition for Seam<L> {
    type Child = ChildSource;

    fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
        let ptr = source.map_or(ptr::null(), |s| s.0.cast_const());
        // SAFETY: NULL or the caller's live source, and a static name.
        unsafe { transition_valid(ptr, func.as_ptr()) }
    }
    fn is(&self, child_source: ChildSource) -> bool {
        child_source.0 == self.0
    }
    fn active(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { transition_active(self.0) }
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
    fn transitioning_video(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_transitioning_video(self.0) }
    }
    fn set_transitioning_video(&self, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_transitioning_video(self.0, value) }
    }
    fn transitioning_audio(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_transitioning_audio(self.0) }
    }
    fn set_transitioning_audio(&self, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_transitioning_audio(self.0, value) }
    }
    fn mode(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_mode(self.0) }
    }
    fn set_mode(&self, value: i32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_mode(self.0, value) }
    }
    fn set_manual_val(&self, value: f32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_manual_val(self.0, value) }
    }
    fn set_manual_target(&self, value: f32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_manual_target(self.0, value) }
    }
    fn set_start_time(&self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_start_time(self.0, value) }
    }
    fn set_duration(&self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_duration(self.0, value) }
    }
    fn use_fixed_duration(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_use_fixed_duration(self.0) }
    }
    fn fixed_duration(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_fixed_duration(self.0) }
    }
    fn cx(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_cx(self.0) }
    }
    fn cy(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_cy(self.0) }
    }
    fn lock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_lock(self.0) };
    }
    fn unlock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_unlock(self.0) };
    }
    fn call_start(&self) {
        // SAFETY: see the impl; the callback may re-enter through the
        // exported functions, which make their own Seam on the pointer.
        unsafe { obs_source_transition_seam_call_start(self.0) };
    }
    fn get_ref(&self, child_source: Option<ChildSource>) -> Option<ChildSource> {
        // SAFETY: see the impl; NULL as the C passes it.
        child(unsafe { L::get_ref(child_ptr(child_source)) })
    }
    fn release(&self, child_source: Option<ChildSource>) {
        // SAFETY: see the impl; NULL as the C passes it.
        unsafe { L::release(child_ptr(child_source)) }
    }
    fn add_active_child(&self, child_source: Option<ChildSource>) -> bool {
        // SAFETY: see the impl; NULL as the C passes it.
        unsafe { L::add_active_child(self.0, child_ptr(child_source)) }
    }
    fn remove_active_child(&self, child_source: ChildSource) {
        // SAFETY: see the impl.
        unsafe { L::remove_active_child(self.0, child_source.0) }
    }
    fn enum_full_tree(&self, root: ChildSource, visitor: &mut dyn FnMut(ChildSource, ChildSource)) {
        let mut visitor = visitor;
        // SAFETY: see the impl; `visit` gets back this frame's `visitor`,
        // live for the whole walk.
        unsafe { L::enum_full_tree(root.0, visit, (&raw mut visitor).cast()) }
    }
    fn now(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { L::now() }
    }
    fn dosignal(&self, signal_obs: &'static CStr, signal_source: &'static CStr) {
        // SAFETY: see the impl; static names.
        unsafe { L::dosignal(self.0, signal_obs.as_ptr(), signal_source.as_ptr()) }
    }
    fn recalculate_size(&self) {
        // SAFETY: see the impl.
        unsafe { L::recalculate_size(self.0) }
    }
    fn recalculate_matrices(&self) {
        // SAFETY: see the impl.
        unsafe { L::recalculate_matrices(self.0) }
    }
    fn warn(&self, format: &'static CStr, other: Option<ChildSource>) {
        // SAFETY: see the impl; the names live while the call runs (the
        // caller holds the transition, and `other` is the caller's source),
        // and blog() formats them (NULL's name as the C's does).
        unsafe {
            let name = L::get_name(self.0);
            let other_name = L::get_name(child_ptr(other));
            blog(LOG_WARNING, format.as_ptr(), name, other_name);
        }
    }
}

/// `obs_transition_start()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`, `dest` NULL or one.
pub unsafe fn start<L: Calls>(
    transition: *mut c_void,
    mode: c_int,
    duration_ms: u32,
    dest: *mut c_void,
) -> bool {
    let seam = (!transition.is_null()).then(|| Seam::<L>::new(transition));
    core::start(seam.as_ref(), mode, duration_ms, child(dest))
}

/// `obs_transition_set()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`, `source` NULL or one.
pub unsafe fn set<L: Calls>(transition: *mut c_void, source: *mut c_void) {
    let seam = (!transition.is_null()).then(|| Seam::<L>::new(transition));
    core::set(seam.as_ref(), child(source));
}

/// `bool obs_transition_start(obs_source_t *transition, enum
/// obs_transition_mode mode, uint32_t duration_ms, obs_source_t *dest)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`, `dest` NULL or one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_start(
    transition: *mut c_void,
    mode: c_int,
    duration_ms: u32,
    dest: *mut c_void,
) -> bool {
    // SAFETY: the caller's contract.
    unsafe { start::<Libobs>(transition, mode, duration_ms, dest) }
}

/// `void obs_transition_set(obs_source_t *transition, obs_source_t
/// *source)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`, `source` NULL or one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_set(transition: *mut c_void, source: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { set::<Libobs>(transition, source) }
}
