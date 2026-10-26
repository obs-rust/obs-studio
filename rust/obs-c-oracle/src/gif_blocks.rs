//! `oracle_*` build of `libobs/graphics/libnsgif/gif-blocks.c.h`
//! (oracle/gif_blocks.c). The C enum comes back as `int`; the result structs
//! are the safe core's `#[repr(C)]` types, checked against the C layout
//! through `oracle_gif_blocks_layout`.

/// Index of each entry in `oracle_gif_blocks_layout`, in the C's order.
pub const LAYOUT_NAMES: [&str; 31] = [
    "sizeof(nsgif_error)",
    "NSGIF_OK",
    "NSGIF_ERR_OOM",
    "NSGIF_ERR_DATA",
    "NSGIF_ERR_BAD_FRAME",
    "NSGIF_ERR_DATA_FRAME",
    "NSGIF_ERR_END_OF_DATA",
    "NSGIF_ERR_DATA_COMPLETE",
    "NSGIF_ERR_FRAME_DISPLAY",
    "NSGIF_ERR_ANIMATION_END",
    "NSGIF_MAX_COLOURS",
    "sizeof(struct nsgif_colour_layout)",
    "sizeof(struct nsgif_screen)",
    "offsetof(screen, height)",
    "offsetof(screen, global_palette)",
    "offsetof(screen, colour_table_size)",
    "offsetof(screen, bg_index)",
    "offsetof(screen, aspect_ratio)",
    "offsetof(screen, loop_max)",
    "sizeof(struct nsgif_blocks_frame_ext)",
    "offsetof(frame_ext, disposal)",
    "offsetof(frame_ext, transparency)",
    "offsetof(frame_ext, transparency_index)",
    "offsetof(frame_ext, redraw_required)",
    "sizeof(struct nsgif_blocks_image)",
    "offsetof(image, rect)",
    "offsetof(image, interlaced)",
    "sizeof(nsgif_rect_t)",
    "NSGIF_DISPOSAL_RESTORE_BG",
    "NSGIF_DISPOSAL_RESTORE_PREV",
    "NSGIF_DISPOSAL_RESTORE_QUIRK",
];

unsafe extern "C" {
    pub fn oracle_nsgif_blocks_header(
        data: *const u8,
        len: usize,
        pos: *mut usize,
        strict: bool,
    ) -> i32;
    pub fn oracle_nsgif_blocks_screen(
        data: *const u8,
        len: usize,
        pos: *mut usize,
        out: *mut u8,
    ) -> i32;
    pub fn oracle_nsgif_blocks_screen_size_quirk(width: *mut u32, height: *mut u32);
    pub fn oracle_nsgif_blocks_extensions(
        data: *const u8,
        len: usize,
        pos: *mut usize,
        decode: bool,
        ext: *mut u8,
        loop_max: *mut i32,
    ) -> i32;
    pub fn oracle_nsgif_blocks_image_descriptor(
        data: *const u8,
        len: usize,
        pos: *mut usize,
        decode: bool,
        first_frame: bool,
        out: *mut u8,
        width: *mut u32,
        height: *mut u32,
    ) -> i32;
    pub fn oracle_nsgif_blocks_colour_table(
        colour_table: *mut u32,
        layout: *const u8,
        entries: usize,
        data: *const u8,
        data_len: usize,
        used: *mut usize,
        decode: bool,
    ) -> i32;
    pub fn oracle_nsgif_blocks_colour_table_decode(
        colour_table: *mut u32,
        layout: *const u8,
        entries: usize,
        data: *const u8,
    );
    /// The entry named `LAYOUT_NAMES[which]`; 0 out of range.
    pub fn oracle_gif_blocks_layout(which: usize) -> usize;
}
