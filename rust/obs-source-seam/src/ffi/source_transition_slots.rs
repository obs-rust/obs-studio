//! C ABI for `libobs/obs-source-transition-slots.c.h`, the two child slots
//! of a transition: `obs_transition_clear()`,
//! `obs_transition_enum_sources()`, `obs_transition_swap_begin()` and
//! `obs_transition_swap_end()`. Three are `EXPORT` through `obs.h`
//! (`rust-exports.d`); `obs_transition_enum_sources()` is declared in
//! `obs-internal.h` without it and stays hidden (`rust-hidden.d`).
//!
//! The functions are the safe core's (`obs_source::transition_slots`), run
//! on [`Seam`], which implements its `Transition` over the transition seam
//! (`libobs/obs-source-transition-seam.h`, still C in `libobs.core-source`):
//! `transition_valid()`, one accessor per field, and the lock and unlock of
//! `transition_mutex` and `transition_tex_mutex`, one seam call per method.
//! `struct obs_source` is never mirrored. The calls into the rest of libobs
//! (`obs_source_get_ref()`, `obs_source_release()`,
//! `obs_source_add_active_child()`, `obs_source_remove_active_child()` and
//! the two `obs_source_get_name()` of the recursion warning) go through
//! [`Calls`]: [`Libobs`] calls the C symbols, and the parity test hands in
//! the oracle's recording stand-ins, as the C oracle calls them. The warning
//! itself is `blog(LOG_WARNING, ...)`, as in the C.
//!
//! The locks are the C's, taken where it takes them: the clear and the enum
//! under `transition_mutex` (the enum's callback runs with it held), the
//! swap's begin taking both texture locks, then both transition locks, and
//! its end releasing the texture locks after the render targets move. The
//! seam takes no lock of its own.
//!
//! As in the C, `obs_transition_enum_sources()` and the swap do not check
//! their sources; `obs_transition_clear()` asks `transition_valid()`.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::marker::PhantomData;
use std::ptr;

use obs_source::transition_slots::{self as core, RECURSION_WARNING, Transition};

/// `LOG_WARNING`.
const LOG_WARNING: c_int = 200;

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
    fn obs_source_transition_seam_get_transitioning_video(transition: *const c_void) -> bool;
    fn obs_source_transition_seam_set_transitioning_video(transition: *mut c_void, value: bool);
    fn obs_source_transition_seam_get_transitioning_audio(transition: *const c_void) -> bool;
    fn obs_source_transition_seam_set_transitioning_audio(transition: *mut c_void, value: bool);
    fn obs_source_transition_seam_get_start_time(transition: *const c_void) -> u64;
    fn obs_source_transition_seam_set_start_time(transition: *mut c_void, value: u64);
    fn obs_source_transition_seam_get_duration(transition: *const c_void) -> u64;
    fn obs_source_transition_seam_set_duration(transition: *mut c_void, value: u64);
    fn obs_source_transition_seam_get_mode(transition: *const c_void) -> c_int;
    fn obs_source_transition_seam_set_mode(transition: *mut c_void, value: c_int);
    fn obs_source_transition_seam_get_manual_val(transition: *const c_void) -> f32;
    fn obs_source_transition_seam_set_manual_val(transition: *mut c_void, value: f32);
    fn obs_source_transition_seam_get_manual_target(transition: *const c_void) -> f32;
    fn obs_source_transition_seam_set_manual_target(transition: *mut c_void, value: f32);
    fn obs_source_transition_seam_get_texrender(
        transition: *const c_void,
        idx: usize,
    ) -> *mut c_void;
    fn obs_source_transition_seam_set_texrender(
        transition: *mut c_void,
        idx: usize,
        value: *mut c_void,
    );
    fn obs_source_transition_seam_lock(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_unlock(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_lock_textures(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_unlock_textures(transition: *mut c_void) -> c_int;

    fn blog(level: c_int, format: *const c_char, ...);
}

/// The calls the slots make into the rest of libobs, one C function each.
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
    /// Both are live `obs_source_t`.
    unsafe fn add_active_child(parent: *mut c_void, child: *mut c_void) -> bool;
    /// `obs_source_remove_active_child(parent, child)`.
    ///
    /// # Safety
    /// Both are live `obs_source_t`.
    unsafe fn remove_active_child(parent: *mut c_void, child: *mut c_void);
    /// `obs_source_get_name(source)`.
    ///
    /// # Safety
    /// `source` is a live `obs_source_t`.
    unsafe fn get_name(source: *const c_void) -> *const c_char;
}

/// [`Calls`] through libobs's own functions.
pub enum Libobs {}

unsafe extern "C" {
    fn obs_source_get_ref(source: *mut c_void) -> *mut c_void;
    fn obs_source_release(source: *mut c_void);
    fn obs_source_add_active_child(parent: *mut c_void, child: *mut c_void) -> bool;
    fn obs_source_remove_active_child(parent: *mut c_void, child: *mut c_void);
    fn obs_source_get_name(source: *const c_void) -> *const c_char;
}

// SAFETY: each method is the one libobs function of its name, called with
// the caller's pointers.
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
}

/// A non-NULL child of a transition, read from one of its slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChildSource(*mut c_void);

/// A `gs_texrender_t *` of a transition, moved but never dereferenced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Texrender(*mut c_void);

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
// each child one the seam read from its slot (live while a slot or the
// caller holds a reference on it), and each call is the one seam accessor,
// lock call or libobs function that names it. `idx` is a slot, 0 or 1.
impl<L: Calls> Transition for Seam<L> {
    type Child = ChildSource;
    type Texrender = Texrender;

    fn same(&self, other: &Self) -> bool {
        self.0 == other.0
    }
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
    fn start_time(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_start_time(self.0) }
    }
    fn set_start_time(&self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_start_time(self.0, value) }
    }
    fn duration(&self) -> u64 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_duration(self.0) }
    }
    fn set_duration(&self, value: u64) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_duration(self.0, value) }
    }
    fn mode(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_mode(self.0) }
    }
    fn set_mode(&self, value: i32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_mode(self.0, value) }
    }
    fn manual_val(&self) -> f32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_manual_val(self.0) }
    }
    fn set_manual_val(&self, value: f32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_manual_val(self.0, value) }
    }
    fn manual_target(&self) -> f32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_manual_target(self.0) }
    }
    fn set_manual_target(&self, value: f32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_manual_target(self.0, value) }
    }
    fn texrender(&self, idx: usize) -> Texrender {
        // SAFETY: see the impl.
        Texrender(unsafe { obs_source_transition_seam_get_texrender(self.0, idx) })
    }
    fn set_texrender(&self, idx: usize, value: Texrender) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_texrender(self.0, idx, value.0) }
    }
    fn lock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_lock(self.0) };
    }
    fn unlock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_unlock(self.0) };
    }
    fn lock_textures(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_lock_textures(self.0) };
    }
    fn unlock_textures(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_unlock_textures(self.0) };
    }
    fn get_ref(&self, child_source: Option<ChildSource>) -> Option<ChildSource> {
        // SAFETY: see the impl; NULL for an empty slot, as the C passes it.
        child(unsafe { L::get_ref(child_ptr(child_source)) })
    }
    fn release(&self, child_source: Option<ChildSource>) {
        // SAFETY: see the impl; NULL for an empty slot, as the C passes it.
        unsafe { L::release(child_ptr(child_source)) }
    }
    fn add_active_child(&self, child_source: ChildSource) -> bool {
        // SAFETY: see the impl.
        unsafe { L::add_active_child(self.0, child_source.0) }
    }
    fn remove_active_child(&self, child_source: ChildSource) {
        // SAFETY: see the impl.
        unsafe { L::remove_active_child(self.0, child_source.0) }
    }
    fn warn_recursion(&self, child_source: ChildSource) {
        // SAFETY: see the impl; the names live while the call runs (the
        // destination is locked, and the child is referenced until the
        // release that follows the warning), and blog() formats them.
        unsafe {
            let dest_name = L::get_name(self.0);
            let child_name = L::get_name(child_source.0);
            blog(
                LOG_WARNING,
                RECURSION_WARNING.as_ptr(),
                dest_name,
                child_name,
            );
        }
    }
}

/// `obs_transition_clear()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
pub unsafe fn clear<L: Calls>(transition: *mut c_void) {
    let seam = (!transition.is_null()).then(|| Seam::<L>::new(transition));
    core::clear(seam.as_ref());
}

/// `obs_transition_enum_sources()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is a live `obs_source_t` (the C does not check it) and `cb`
/// a function that takes `param`.
pub unsafe fn enum_sources<L: Calls>(
    transition: *mut c_void,
    cb: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void),
    param: *mut c_void,
) {
    core::enum_sources(&Seam::<L>::new(transition), |c| {
        // SAFETY: the caller's callback, with the transition, a child in one
        // of its slots (held while the lock is) and the caller's param.
        unsafe { cb(transition, c.0, param) }
    });
}

/// `obs_transition_swap_begin()` with the libobs calls of `L`.
///
/// # Safety
/// Both are live `obs_source_t` (the C does not check them).
pub unsafe fn swap_begin<L: Calls>(tr_dest: *mut c_void, tr_source: *mut c_void) {
    core::swap_begin(&Seam::<L>::new(tr_dest), &Seam::<L>::new(tr_source));
}

/// `obs_transition_swap_end()` with the libobs calls of `L`.
///
/// # Safety
/// Both are live `obs_source_t` (the C does not check them), whose texture
/// locks [`swap_begin`] took on this thread.
pub unsafe fn swap_end<L: Calls>(tr_dest: *mut c_void, tr_source: *mut c_void) {
    core::swap_end(&Seam::<L>::new(tr_dest), &Seam::<L>::new(tr_source));
}

/// `void obs_transition_clear(obs_source_t *transition)`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_clear(transition: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { clear::<Libobs>(transition) }
}

/// `void obs_transition_enum_sources(obs_source_t *transition,
/// obs_source_enum_proc_t cb, void *param)`.
///
/// # Safety
/// `transition` is a live `obs_source_t` and `cb` a function that takes
/// `param`, as the C requires.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_enum_sources(
    transition: *mut c_void,
    cb: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void),
    param: *mut c_void,
) {
    // SAFETY: the caller's contract.
    unsafe { enum_sources::<Libobs>(transition, cb, param) }
}

/// `void obs_transition_swap_begin(obs_source_t *tr_dest, obs_source_t
/// *tr_source)`.
///
/// # Safety
/// Both are live `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_swap_begin(tr_dest: *mut c_void, tr_source: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { swap_begin::<Libobs>(tr_dest, tr_source) }
}

/// `void obs_transition_swap_end(obs_source_t *tr_dest, obs_source_t
/// *tr_source)`.
///
/// # Safety
/// Both are live `obs_source_t`, after `obs_transition_swap_begin()` on
/// the same pair and thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_swap_end(tr_dest: *mut c_void, tr_source: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { swap_end::<Libobs>(tr_dest, tr_source) }
}
