use core::ffi::{c_int, c_long, c_void};

unsafe extern "C" {
    pub fn oracle_bmalloc(size: usize) -> *mut c_void;
    pub fn oracle_brealloc(ptr: *mut c_void, size: usize) -> *mut c_void;
    pub fn oracle_bfree(ptr: *mut c_void);
    pub fn oracle_bnum_allocs() -> c_long;
    pub fn oracle_base_get_alignment() -> c_int;
    pub fn oracle_bmemdup(ptr: *const c_void, size: usize) -> *mut c_void;
}
