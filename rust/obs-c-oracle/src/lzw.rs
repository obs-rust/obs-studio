//! `oracle_*` build of `libobs/graphics/libnsgif/lzw.c.h` (oracle/lzw.c).
//! The C enum comes back as `int`; the context is opaque.

use core::ffi::c_void;

/// Index of each entry in `oracle_lzw_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 11] = [
    "sizeof(lzw_result)",
    "LZW_OK",
    "LZW_OK_EOD",
    "LZW_NO_MEM",
    "LZW_NO_DATA",
    "LZW_EOI_CODE",
    "LZW_NO_COLOUR",
    "LZW_BAD_ICODE",
    "LZW_BAD_PARAM",
    "LZW_BAD_CODE",
    "LZW_CODE_MAX",
];

unsafe extern "C" {
    pub fn oracle_lzw_context_create(ctx: *mut *mut c_void) -> i32;
    pub fn oracle_lzw_context_destroy(ctx: *mut c_void);
    pub fn oracle_lzw_decode_init(
        ctx: *mut c_void,
        minimum_code_size: u8,
        input_data: *const u8,
        input_length: usize,
        input_pos: usize,
    ) -> i32;
    pub fn oracle_lzw_decode(
        ctx: *mut c_void,
        output_data: *mut *const u8,
        output_written: *mut u32,
    ) -> i32;
    pub fn oracle_lzw_decode_init_map(
        ctx: *mut c_void,
        minimum_code_size: u8,
        transparency_idx: u32,
        colour_table: *const u32,
        input_data: *const u8,
        input_length: usize,
        input_pos: usize,
    ) -> i32;
    pub fn oracle_lzw_decode_map(
        ctx: *mut c_void,
        output_data: *mut u32,
        output_length: u32,
        output_written: *mut u32,
    ) -> i32;
    /// `memset` of the whole context with `byte`.
    pub fn oracle_lzw_context_fill(ctx: *mut c_void, byte: i32);
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_lzw_layout(which: usize) -> usize;
}
