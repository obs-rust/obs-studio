//! C-owned allocation and opaque core/module field access for locale changes.
use super::module_paths::bstrdup;
use crate::set_locale::{self, Host};
use core::ffi::{c_char, c_void};
use core::ptr::NonNull;
use obs_util::ffi::darray::bfree;

pub type Callback = Option<unsafe extern "C" fn(*const c_char)>;
unsafe extern "C" {
    fn obs_core_set_locale_core() -> *mut c_void;
    fn obs_core_set_locale_field(core: *mut c_void) -> *mut *mut c_char;
    fn obs_core_set_locale_first(core: *mut c_void) -> *mut c_void;
    fn obs_core_set_locale_callback(module: *mut c_void) -> Callback;
    fn obs_core_set_locale_next(module: *mut c_void) -> *mut c_void;
}
struct Calls {
    core: NonNull<c_void>,
}
impl Host for Calls {
    type Locale = *const c_char;
    type Owned = *mut c_char;
    type Module = NonNull<c_void>;
    type Callback = unsafe extern "C" fn(*const c_char);
    fn owned_locale(&mut self) -> Option<Self::Owned> {
        // SAFETY: the API caller keeps the checked core live and exclusive.
        NonNull::new(unsafe { obs_core_set_locale_field(self.core.as_ptr()).read() })
            .map(NonNull::as_ptr)
    }
    fn free_locale(&mut self, owned: Self::Owned) {
        // SAFETY: locale storage is owned by libobs and allocated with bmem.
        unsafe { bfree(owned.cast()) };
    }
    fn copy_locale(&mut self, locale: Self::Locale) -> Self::Owned {
        // SAFETY: the caller supplies NULL or a live NUL-terminated string,
        // independent of the old locale that the safe core has just released.
        NonNull::new(locale.cast_mut()).map_or(core::ptr::null_mut(), |locale| {
            // SAFETY: the non-NULL input is NUL-terminated and remains live.
            bstrdup(unsafe { core::ffi::CStr::from_ptr(locale.as_ptr()) }.to_bytes())
        })
    }
    fn store_locale(&mut self, owned: Self::Owned) {
        // SAFETY: the checked core's field receives its new bmem-owned value.
        unsafe { obs_core_set_locale_field(self.core.as_ptr()).write(owned) };
    }
    fn first_module(&mut self) -> Option<Self::Module> {
        // SAFETY: the caller keeps the core and its module list live.
        NonNull::new(unsafe { obs_core_set_locale_first(self.core.as_ptr()) })
    }
    fn callback(&mut self, module: Self::Module) -> Option<Self::Callback> {
        // SAFETY: only non-NULL live list members reach this field accessor.
        unsafe { obs_core_set_locale_callback(module.as_ptr()) }
    }
    fn notify(&mut self, callback: Self::Callback, locale: Self::Locale) {
        // SAFETY: this module callback accepts the original caller argument.
        // No Rust reference or lock spans the possibly reentrant callback.
        unsafe { callback(locale) };
    }
    fn next_module(&mut self, module: Self::Module) -> Option<Self::Module> {
        // SAFETY: the caller retains this node across its callback, as in C.
        NonNull::new(unsafe { obs_core_set_locale_next(module.as_ptr()) })
    }
}

/// Replace the locale and notify present callbacks in linked-list order.
///
/// # Safety
/// OBS must be initialized and remain live without concurrent locale/list
/// writes. `locale` must be NULL or a live NUL-terminated string that does not
/// alias the old owned locale. Callbacks must retain their current list node.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_set_locale(locale: *const c_char) {
    // SAFETY: querying the global pointer does not access any core fields.
    let core = NonNull::new(unsafe { obs_core_set_locale_core() })
        .expect("locale replacement requires an initialized core");
    set_locale::set_locale(&mut Calls { core }, locale);
}
