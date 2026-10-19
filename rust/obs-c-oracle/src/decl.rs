//! `oracle_*` build of `libobs/callback/decl.c.h` (oracle/decl.c), on the
//! original C parser. Pointers are the Rust `decl_info` struct; the layout
//! test checks that struct against these `sizeof` / `offsetof` results.

use core::ffi::{c_char, c_int, c_void};

unsafe extern "C" {
    pub fn oracle_parse_decl_string(decl: *mut c_void, decl_string: *const c_char) -> bool;

    /// Starts capturing the `Errors/warnings for '...'` warnings logged on
    /// the calling thread. Hold the test binary's log-handler lock until
    /// [`oracle_decl_log_end`]: the handler is process-wide.
    pub fn oracle_decl_log_begin();
    /// Stops capturing. Copies up to `capacity` bytes of the captured text
    /// to `out`, and returns the full length.
    pub fn oracle_decl_log_end(out: *mut c_char, capacity: usize, level: *mut c_int) -> usize;

    pub fn oracle_call_param_type_size() -> usize;
    pub fn oracle_decl_param_size() -> usize;
    pub fn oracle_decl_param_align() -> usize;
    pub fn oracle_decl_param_offset_name() -> usize;
    pub fn oracle_decl_param_offset_type() -> usize;
    pub fn oracle_decl_param_offset_flags() -> usize;
    pub fn oracle_decl_info_size() -> usize;
    pub fn oracle_decl_info_align() -> usize;
    pub fn oracle_decl_info_offset_name() -> usize;
    pub fn oracle_decl_info_offset_decl_string() -> usize;
    pub fn oracle_decl_info_offset_params() -> usize;
}
