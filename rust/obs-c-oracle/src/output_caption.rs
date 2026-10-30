//! `oracle_*` build of `libobs/obs-output-caption.c.h`
//! (oracle/output_caption.c): the output caption plan, renamed.

use core::ffi::{c_char, c_int};

unsafe extern "C" {
    /// `codec` is NUL-terminated, `carrier` a writable `uint32_t`.
    pub fn oracle_obs_output_caption_begin(
        codec: *const c_char,
        hevc: bool,
        priority: c_int,
        carrier: *mut u32,
    ) -> bool;

    /// Reads up to 6 bytes of `data` (the start code and 2 header bytes):
    /// never call it with fewer readable; `header` is 2 writable bytes.
    pub fn oracle_obs_output_caption_hevc_sei_header(data: *const u8, header: *mut [u8; 2])
    -> bool;

    /// `triplet` is 3 readable bytes, `cc_data` a writable `uint16_t`.
    pub fn oracle_obs_output_caption_cc_keep(triplet: *const [u8; 3], cc_data: *mut u16) -> bool;
}
