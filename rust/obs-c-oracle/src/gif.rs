//! `oracle_*` build of `libobs/graphics/libnsgif/gif.c.h` (oracle/gif.c):
//! the whole of libnsgif in C, its LZW, block parsers and frame decode
//! included. `nsgif_t` is opaque; the info structs and the callback table
//! are the shim's `#[repr(C)]` types.

use core::ffi::{c_char, c_void};

unsafe extern "C" {
    pub fn oracle_nsgif_create(
        bitmap_vt: *const c_void,
        bitmap_fmt: u32,
        gif_out: *mut *mut c_void,
    ) -> i32;
    pub fn oracle_nsgif_destroy(gif: *mut c_void);
    pub fn oracle_nsgif_set_frame_delay_behaviour(
        gif: *mut c_void,
        delay_min: u16,
        delay_default: u16,
    );
    pub fn oracle_nsgif_data_scan(gif: *mut c_void, size: usize, data: *const u8) -> i32;
    pub fn oracle_nsgif_data_complete(gif: *mut c_void);
    pub fn oracle_nsgif_reset(gif: *mut c_void) -> i32;
    pub fn oracle_nsgif_frame_prepare(
        gif: *mut c_void,
        area: *mut c_void,
        delay_cs: *mut u32,
        frame_new: *mut u32,
    ) -> i32;
    pub fn oracle_nsgif_frame_decode(gif: *mut c_void, frame: u32, bitmap: *mut *mut c_void)
    -> i32;
    pub fn oracle_nsgif_get_info(gif: *const c_void) -> *const c_void;
    pub fn oracle_nsgif_get_frame_info(gif: *const c_void, frame: u32) -> *const c_void;
    pub fn oracle_nsgif_global_palette(gif: *const c_void, table: *mut u32, entries: *mut usize);
    pub fn oracle_nsgif_local_palette(
        gif: *const c_void,
        frame: u32,
        table: *mut u32,
        entries: *mut usize,
    ) -> bool;
    pub fn oracle_nsgif_strerror(err: u32) -> *const c_char;
    pub fn oracle_nsgif_str_disposal(disposal: u32) -> *const c_char;
}
