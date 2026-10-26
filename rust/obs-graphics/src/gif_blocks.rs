//! Safe core of `libobs/graphics/libnsgif/gif-blocks.c.h`: libnsgif's GIF
//! block parsers.
//!
//! The parsers read the blocks that come before the image data of a GIF,
//! each from a byte position in the whole GIF data, and move that position
//! on success: the header ([`header`]), the logical screen descriptor
//! ([`screen`], with [`screen_size_quirk`]), the extensions before a frame
//! ([`extensions`]), the image descriptor ([`image_descriptor`]) and the
//! colour tables ([`colour_table`], [`colour_table_decode`]). `gif.c` keeps
//! the state (`struct nsgif`, `struct nsgif_frame`) and copies the values
//! in and out.
//!
//! Characterized, not endorsed (`test_nsgif_decode`), and kept as the C has
//! them:
//! - the lenient header takes any version after `GIF`;
//! - the graphic control extension does not check its size byte, and the
//!   skip after it starts from that size byte;
//! - [`extensions`] writes the frame fields and the loop count as it goes,
//!   also when a later extension then returns an error;
//! - an extension size that skips past the end of the data is not an error:
//!   the position stops at the end;
//! - without `decode`, the image descriptor's separator is not checked.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `NSGIF_MAX_COLOURS`: entries in a colour table.
pub const MAX_COLOURS: usize = 256;

/// `nsgif_error`, in the C's order.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NsgifError {
    /// `NSGIF_OK`.
    Ok = 0,
    /// `NSGIF_ERR_OOM`.
    Oom,
    /// `NSGIF_ERR_DATA`: not a GIF.
    Data,
    /// `NSGIF_ERR_BAD_FRAME`.
    BadFrame,
    /// `NSGIF_ERR_DATA_FRAME`: a bad frame block.
    DataFrame,
    /// `NSGIF_ERR_END_OF_DATA`: the data ends inside a block.
    EndOfData,
    /// `NSGIF_ERR_DATA_COMPLETE`.
    DataComplete,
    /// `NSGIF_ERR_FRAME_DISPLAY`.
    FrameDisplay,
    /// `NSGIF_ERR_ANIMATION_END`.
    AnimationEnd,
}

/// `enum nsgif_disposal` values that the graphic control extension maps.
pub mod disposal {
    /// `NSGIF_DISPOSAL_RESTORE_BG`.
    pub const RESTORE_BG: u8 = 2;
    /// `NSGIF_DISPOSAL_RESTORE_PREV`.
    pub const RESTORE_PREV: u8 = 3;
    /// `NSGIF_DISPOSAL_RESTORE_QUIRK`: read as [`RESTORE_PREV`].
    pub const RESTORE_QUIRK: u8 = 4;
}

/// `struct nsgif_colour_layout`: the byte offset of each component in a
/// 32-bit pixel.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ColourLayout {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// `struct nsgif_blocks_screen`: what the logical screen descriptor sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
    pub global_palette: bool,
    pub colour_table_size: u32,
    pub bg_index: u32,
    pub aspect_ratio: u32,
    pub loop_max: i32,
}

/// `struct nsgif_blocks_frame_ext`: the frame fields that the graphic
/// control extension sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameExt {
    pub delay: u32,
    pub disposal: u8,
    pub transparency: bool,
    pub transparency_index: u32,
    pub redraw_required: bool,
}

/// `nsgif_rect_t`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

/// `struct nsgif_blocks_image`: what the image descriptor sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Image {
    pub flags: u32,
    pub rect: Rect,
    pub interlaced: bool,
}

/// `nsgif_blocks_header`: `GIF` and, when `strict`, `87a` or `89a`. Six
/// bytes from `pos`.
pub fn header(data: &[u8], pos: &mut usize, strict: bool) -> NsgifError {
    let _ = (data, pos, strict);
    todo!("header")
}

/// `nsgif_blocks_screen`: the seven-byte logical screen descriptor. `out`
/// is set on [`NsgifError::Ok`] only.
pub fn screen(data: &[u8], pos: &mut usize, out: &mut Screen) -> NsgifError {
    let _ = (data, pos, out);
    todo!("screen")
}

/// `nsgif_blocks_screen_size_quirk`: the screen sizes that broken GIFs
/// report become 1x1.
pub fn screen_size_quirk(width: &mut u32, height: &mut u32) {
    let _ = (width, height);
    todo!("screen_size_quirk")
}

/// `nsgif_blocks_extensions`: the extensions before a frame. With
/// `decode`, the graphic control extension sets `ext` and a loop count
/// application extension sets `loop_max`, as they are parsed.
pub fn extensions(
    data: &[u8],
    pos: &mut usize,
    decode: bool,
    ext: &mut FrameExt,
    loop_max: &mut i32,
) -> NsgifError {
    let _ = (data, pos, decode, ext, loop_max);
    todo!("extensions")
}

/// `nsgif_blocks_image_descriptor`: the ten-byte image descriptor. With
/// `decode`, `out` is set and the first frame can grow `width` and
/// `height`.
pub fn image_descriptor(
    data: &[u8],
    pos: &mut usize,
    decode: bool,
    first_frame: bool,
    out: &mut Image,
    width: &mut u32,
    height: &mut u32,
) -> NsgifError {
    let _ = (data, pos, decode, first_frame, out, width, height);
    todo!("image_descriptor")
}

/// `nsgif_blocks_colour_table`: `entries` r,g,b triplets at the start of
/// `data`, decoded into `table` when `decode`. `used` is the byte count on
/// [`NsgifError::Ok`].
pub fn colour_table(
    table: &mut [u32; MAX_COLOURS],
    layout: ColourLayout,
    entries: usize,
    data: &[u8],
    used: &mut usize,
    decode: bool,
) -> NsgifError {
    let _ = (table, layout, entries, data, used, decode);
    todo!("colour_table")
}

/// `nsgif_blocks_colour_table_decode`: `entries` r,g,b triplets into
/// `table` in the layout, alpha 0xff. The C checks no length; here the
/// triplets past `data` and the bytes past `table` are not written.
pub fn colour_table_decode(
    table: &mut [u32; MAX_COLOURS],
    layout: ColourLayout,
    entries: usize,
    data: &[u8],
) {
    let _ = (table, layout, entries, data);
    todo!("colour_table_decode")
}
