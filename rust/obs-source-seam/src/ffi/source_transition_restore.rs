//! C ABI for `libobs/obs-source-transition-restore.c.h`, the load of a
//! saved transition: `obs_transition_load()`. It is declared in
//! `obs-internal.h` without `EXPORT` (`obs_load_source()` calls it) and
//! stays hidden (`rust-hidden.d`).
//!
//! The function is the safe core's (`obs_source::transition_load::load`),
//! run on the sizing shims' [`Seam`], which implements its `Transition`
//! over the transition seam (`libobs/obs-source-transition-seam.h`): the
//! slot A accessors, the field writes and the lock of `transition_mutex`,
//! one seam call per method, and `context.name` through
//! `obs_source_transition_seam_get_child_name()`. `struct obs_source` is
//! never mirrored. The saved data is read through `obs_data_get_string()`
//! and `obs_data_get_int()`, and the warnings go to `blog(LOG_WARNING,
//! ...)`, as in the C. The calls into the rest of libobs that find, link or
//! release a source (`obs_get_source_by_name()`,
//! `obs_source_add_active_child()`, `obs_source_remove_active_child()` and
//! `obs_source_release()`) go through [`Calls`], on top of the sizing's:
//! [`Libobs`] calls the C symbols, and the parity test hands in the
//! oracle's recording stand-ins, as the C oracle calls them. The size and
//! matrix recalculation after the unlock are the sizing's, on the same
//! [`Seam`].
//!
//! The lock is the C's, taken where it takes it: `transition_mutex` around
//! the read of the old slot A and the field writes, with the lookup, the
//! active-child calls, the releases and the warnings outside it.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

use obs_source::transition_load::{self as core, NOT_FOUND_WARNING, RECURSION_WARNING, Transition};

pub use super::source_transition_size::Libobs;
use super::source_transition_size::{self as size, ChildSource, Seam};

/// `LOG_WARNING`.
const LOG_WARNING: c_int = 200;

unsafe extern "C" {
    fn obs_source_transition_seam_set_sources(
        transition: *mut c_void,
        idx: usize,
        value: *mut c_void,
    );
    fn obs_source_transition_seam_get_child_name(child: *const c_void) -> *const c_char;
    fn obs_source_transition_seam_get_source_active(transition: *const c_void, idx: usize) -> bool;
    fn obs_source_transition_seam_set_source_active(
        transition: *mut c_void,
        idx: usize,
        value: bool,
    );
    fn obs_source_transition_seam_set_alignment(transition: *mut c_void, value: u32);
    fn obs_source_transition_seam_set_mode(transition: *mut c_void, value: c_int);
    fn obs_source_transition_seam_set_scale_type(transition: *mut c_void, value: c_int);
    fn obs_source_transition_seam_set_cx(transition: *mut c_void, value: u32);
    fn obs_source_transition_seam_set_cy(transition: *mut c_void, value: u32);

    fn obs_data_get_string(data: *mut c_void, name: *const c_char) -> *const c_char;
    fn obs_data_get_int(data: *mut c_void, name: *const c_char) -> i64;

    fn blog(level: c_int, format: *const c_char, ...);
}

/// The calls the load makes into the rest of libobs that find, link or
/// release a source, one C function each, with the sizing's.
///
/// # Safety
/// Each is called with the pointers the C passes: a NUL-terminated name,
/// live sources (or NULL where the C allows it).
pub unsafe trait Calls: size::Calls {
    /// `obs_get_source_by_name(name)`.
    ///
    /// # Safety
    /// `name` is a NUL-terminated string.
    unsafe fn get_source_by_name(name: *const c_char) -> *mut c_void;
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
    /// `obs_source_release(source)`.
    ///
    /// # Safety
    /// `source` is NULL or a live `obs_source_t` the caller holds a
    /// reference on.
    unsafe fn release(source: *mut c_void);
}

unsafe extern "C" {
    fn obs_get_source_by_name(name: *const c_char) -> *mut c_void;
    fn obs_source_add_active_child(parent: *mut c_void, child: *mut c_void) -> bool;
    fn obs_source_remove_active_child(parent: *mut c_void, child: *mut c_void);
    fn obs_source_release(source: *mut c_void);
}

// SAFETY: each method is the one libobs function of its name, called with
// the caller's pointers.
unsafe impl Calls for Libobs {
    unsafe fn get_source_by_name(name: *const c_char) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { obs_get_source_by_name(name) }
    }
    unsafe fn add_active_child(parent: *mut c_void, child: *mut c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { obs_source_add_active_child(parent, child) }
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

/// An `obs_data_t *` of a saved transition.
pub struct Data(*mut c_void);

/// A non-NULL `const char *` read from the saved data, live while the data
/// holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Name(*const c_char);

fn child_ptr(child: Option<ChildSource>) -> *mut c_void {
    child.map_or(ptr::null_mut(), |c| c.0)
}

// SAFETY (every method): as the sizing's impl, `self.0` is the live source
// the C caller passed and each call is the one seam accessor, `obs_data`
// read or libobs function that names it; the data is the caller's live
// `obs_data_t`, and each name a string it holds. `idx` is a slot, 0 or 1.
impl<L: Calls> Transition for Seam<L> {
    type Data = Data;
    type Name = Name;

    fn get_string(&self, data: &Data, key: &'static CStr) -> Option<Name> {
        // SAFETY: see the impl; the key is NUL-terminated.
        let s = unsafe { obs_data_get_string(data.0, key.as_ptr()) };
        (!s.is_null()).then_some(Name(s))
    }
    fn get_int(&self, data: &Data, key: &'static CStr) -> i64 {
        // SAFETY: see the impl; the key is NUL-terminated.
        unsafe { obs_data_get_int(data.0, key.as_ptr()) }
    }
    fn source_by_name(&self, name: Name) -> Option<ChildSource> {
        // SAFETY: see the impl.
        let s = unsafe { L::get_source_by_name(name.0) };
        (!s.is_null()).then_some(ChildSource(s))
    }
    fn add_active_child(&self, child: ChildSource) -> bool {
        // SAFETY: see the impl.
        unsafe { L::add_active_child(self.0, child.0) }
    }
    fn remove_active_child(&self, child: ChildSource) {
        // SAFETY: see the impl.
        unsafe { L::remove_active_child(self.0, child.0) }
    }
    fn release(&self, child: Option<ChildSource>) {
        // SAFETY: see the impl; NULL for an empty slot, as the C passes it.
        unsafe { L::release(child_ptr(child)) }
    }
    fn warn_recursion(&self, name: Name) {
        // SAFETY: see the impl; the transition's name and the saved name
        // live while the call runs, and blog() formats them.
        unsafe {
            let tr_name = obs_source_transition_seam_get_child_name(self.0);
            blog(LOG_WARNING, RECURSION_WARNING.as_ptr(), tr_name, name.0);
        }
    }
    fn warn_not_found(&self, name: Name) {
        // SAFETY: as warn_recursion.
        unsafe {
            let tr_name = obs_source_transition_seam_get_child_name(self.0);
            blog(LOG_WARNING, NOT_FOUND_WARNING.as_ptr(), name.0, tr_name);
        }
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
    fn set_alignment(&self, value: u32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_alignment(self.0, value) }
    }
    fn set_mode(&self, value: i32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_mode(self.0, value) }
    }
    fn set_scale_type(&self, value: i32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_scale_type(self.0, value) }
    }
    fn set_cx(&self, value: u32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_cx(self.0, value) }
    }
    fn set_cy(&self, value: u32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_cy(self.0, value) }
    }
}

/// `obs_transition_load()` with the libobs calls of `L`.
///
/// # Safety
/// `tr` is a live transition `obs_source_t` and `data` a live `obs_data_t`
/// (the C checks neither).
pub unsafe fn load<L: Calls>(tr: *mut c_void, data: *mut c_void) {
    core::load(&Seam::<L>::new(tr), &Data(data));
}

/// `void obs_transition_load(obs_source_t *tr, obs_data_t *data)`.
///
/// # Safety
/// `tr` is a live transition `obs_source_t` and `data` a live `obs_data_t`,
/// as the C requires.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_load(tr: *mut c_void, data: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { load::<Libobs>(tr, data) }
}
