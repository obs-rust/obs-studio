use core::ffi::{c_char, c_void};

// `lookup_t` is opaque; it is passed around as `*mut c_void`.
unsafe extern "C" {
    pub fn oracle_text_lookup_create(path: *const c_char) -> *mut c_void;
    pub fn oracle_text_lookup_add(lookup: *mut c_void, path: *const c_char) -> bool;
    pub fn oracle_text_lookup_destroy(lookup: *mut c_void);
    pub fn oracle_text_lookup_getstr(
        lookup: *mut c_void,
        lookup_val: *const c_char,
        out: *mut *const c_char,
    ) -> bool;
}
