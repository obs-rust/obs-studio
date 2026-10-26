//! `oracle_*` build of `libobs/graphics/libnsgif/gif-frame.c.h`
//! (oracle/gif_frame.c), with its LZW calls on the C LZW oracle
//! ([`crate::lzw`]): pass a context from `oracle_lzw_context_create`. The
//! C enum comes back as `int`; the canvas and the frame are the shim's
//! `#[repr(C)]` types, checked through `oracle_gif_frame_layout`.

use core::ffi::c_void;

/// Index of each entry in `oracle_gif_frame_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 11] = [
    "sizeof(struct nsgif_canvas)",
    "offsetof(canvas, width)",
    "offsetof(canvas, height)",
    "offsetof(canvas, rowspan)",
    "offsetof(canvas, background)",
    "sizeof(struct nsgif_frame_area)",
    "offsetof(frame_area, rect)",
    "offsetof(frame_area, interlaced)",
    "offsetof(frame_area, display)",
    "offsetof(frame_area, transparency)",
    "offsetof(frame_area, transparency_index)",
];

unsafe extern "C" {
    pub fn oracle_nsgif_frame_pixels_decode(
        lzw: *mut c_void,
        buf: *const u8,
        buf_len: usize,
        data_pos: usize,
        canvas: *const c_void,
        frame: *const c_void,
        colour_table: *const u32,
        data_complete: bool,
    ) -> i32;
    pub fn oracle_nsgif_frame_restore_bg(canvas: *const c_void, frame: *const c_void);
    pub fn oracle_nsgif_frame_recover(canvas: *const c_void, prev_frame: *const u32);
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_gif_frame_layout(which: usize) -> usize;
}
