//! Borrowed private-source access. Ordering lives in the safe core.
use crate::save_load::{self, Backend};
use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    fn obs_source_save_load_data(source: *mut c_void) -> *mut c_void;
    fn obs_source_save_load_callback(source: *mut c_void, save: bool);
    fn obs_source_save_load_signal(source: *mut c_void, save: bool);
    fn obs_source_save_load_filter_count(source: *mut c_void) -> usize;
    fn obs_source_save_load_filter(source: *mut c_void, index: usize) -> *mut c_void;
    fn blog(level: c_int, format: *const c_char, ...);
}

struct Source;
impl Backend for Source {
    type Source = *mut c_void;
    fn valid(&mut self, source: Self::Source, name: &'static str) -> bool {
        if source.is_null() {
            // SAFETY: format is terminated and name is a static Rust string with explicit length.
            unsafe {
                blog(
                    400,
                    c"%.*s: Null 'source' parameter".as_ptr(),
                    name.len() as c_int,
                    name.as_ptr(),
                );
            }
            return false;
        }
        // SAFETY: null was rejected; the C caller keeps the source live for this call.
        !unsafe { obs_source_save_load_data(source) }.is_null()
    }
    fn signal(&mut self, source: Self::Source, save: bool) {
        // SAFETY: core validated source; helper preserves the existing calldata and privacy rules.
        unsafe { obs_source_save_load_signal(source, save) }
    }
    fn callback(&mut self, source: Self::Source, save: bool) {
        // SAFETY: borrowed live source; helper reads current callback/data/settings after preceding signals.
        unsafe { obs_source_save_load_callback(source, save) }
    }
    fn filter_count(&mut self, source: Self::Source) -> usize {
        // SAFETY: core validated the parent; callback lifetime is the original API's contract.
        unsafe { obs_source_save_load_filter_count(source) }
    }
    fn filter(&mut self, source: Self::Source, index: usize) -> Self::Source {
        // SAFETY: core uses the original reverse index loop over the caller-owned filter array.
        unsafe { obs_source_save_load_filter(source, index) }
    }
}

/// # Safety
/// Source must be null or a live source obeying the original callback lifetime contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_save(source: *mut c_void) {
    save_load::save(&mut Source, source);
}
/// # Safety
/// Source must be null or a live source obeying the original callback lifetime contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_load(source: *mut c_void) {
    save_load::load(&mut Source, source);
}
/// # Safety
/// Source and its filters must obey the original callback lifetime and mutation contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_source_load2(source: *mut c_void) {
    save_load::load2(&mut Source, source);
}
