//! C ABI and host services for the safe module locale loader.
use std::ffi::{CString, c_char, c_int, c_void};
use std::ptr;

use obs_util::ffi::darray::bfree;
use obs_util::ffi::text_lookup::{text_lookup_add, text_lookup_create};

use super::module_metadata::obs_get_module_file_name;
use super::module_paths::bytes;
use crate::module_load_locale::{self, LocaleLoader};

unsafe extern "C" {
    fn obs_find_module_file(module: *mut c_void, name: *const c_char) -> *mut c_char;
    fn blog(level: c_int, format: *const c_char, ...);
}

struct Host {
    module: *mut c_void,
}
impl Host {
    fn find(&self, name: &[u8]) -> *mut c_char {
        let Ok(name) = CString::new(name) else {
            return ptr::null_mut();
        };
        // SAFETY: only a validated live module reaches these host operations.
        unsafe { obs_find_module_file(self.module, name.as_ptr()) }
    }
}
impl LocaleLoader for Host {
    type Lookup = *mut c_void;
    fn create(&mut self, name: &[u8]) -> Option<Self::Lookup> {
        let file = self.find(name);
        // SAFETY: find returns a bmalloc-owned NUL-terminated path or null.
        unsafe {
            let lookup = if file.is_null() {
                ptr::null_mut()
            } else {
                text_lookup_create(file)
            };
            bfree(file.cast());
            (!lookup.is_null()).then_some(lookup)
        }
    }
    fn add(&mut self, lookup: &mut Self::Lookup, name: &[u8]) -> bool {
        let file = self.find(name);
        // SAFETY: lookup came from create; add accepts a null path. The path
        // is borrowed by add and remains owned here until bfree.
        unsafe {
            let added = text_lookup_add(*lookup, file);
            bfree(file.cast());
            added
        }
    }
    fn invalid(&mut self) {
        // SAFETY: constant NUL-terminated format with no arguments.
        unsafe { blog(200, c"obs_module_load_locale: Invalid parameters".as_ptr()) }
    }
    fn failed(&mut self, locale: &[u8]) {
        let Ok(locale) = CString::new(locale) else {
            return;
        };
        // SAFETY: load checks the module before calling failed. Its borrowed
        // filename and the locale remain valid for the synchronous log call.
        unsafe {
            blog(
                200,
                c"Failed to load '%s' text for module: '%s'".as_ptr(),
                locale.as_ptr(),
                obs_get_module_file_name(self.module),
            );
        }
    }
}

/// Load a module's default locale and optional overlay.
///
/// # Safety
/// `module` is null or a live, synchronized obs_module_t. Locale strings are
/// null or readable NUL-terminated strings. The caller owns the returned lookup.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_module_load_locale(
    module: *mut c_void,
    default_locale: *const c_char,
    locale: *const c_char,
) -> *mut c_void {
    // SAFETY: caller supplies readable NUL-terminated strings or null.
    let (default_locale, locale) = unsafe { (bytes(default_locale), bytes(locale)) };
    module_load_locale::load(
        &mut Host { module },
        !module.is_null(),
        default_locale,
        locale,
    )
    .unwrap_or(ptr::null_mut())
}
