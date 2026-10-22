//! `oracle_*` build of `libobs/obs-output-protocols.c.h`
//! (oracle/output_protocols.c): the protocol-list match, renamed.

use core::ffi::c_char;

unsafe extern "C" {
    /// `obs_output_protocol_list_count`: `protocols` is NULL or a
    /// NUL-terminated string; `protocol` is a NUL-terminated string (the C
    /// crashes on NULL).
    pub fn oracle_obs_output_protocol_list_count(
        protocols: *const c_char,
        protocol: *const c_char,
    ) -> usize;
}
