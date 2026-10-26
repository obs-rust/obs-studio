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

/// `struct nsgif_screen`: what the logical screen descriptor sets.
/// The C layout, so the shim takes it by pointer.
#[repr(C)]
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
#[repr(C)]
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
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Image {
    pub flags: u32,
    pub rect: Rect,
    pub interlaced: bool,
}

/// GIF flags of the screen descriptor and the image descriptor.
const COLOUR_TABLE_MASK: u8 = 0x80;
const COLOUR_TABLE_SIZE_MASK: u8 = 0x07;

const EXT_INTRODUCER: u8 = 0x21;
const EXT_GRAPHIC_CONTROL: u8 = 0xf9;
const EXT_COMMENT: u8 = 0xfe;
const EXT_APPLICATION: u8 = 0xff;
const BLOCK_TERMINATOR: u8 = 0x00;

const IMAGE_DESCRIPTOR_LEN: usize = 10;
const IMAGE_SEPARATOR: u8 = 0x2c;
const MASK_INTERLACE: u8 = 0x40;

/// The byte at `p`; 0 past the data, where the C never reads.
fn byte(data: &[u8], p: usize) -> u8 {
    data.get(p).copied().unwrap_or(0)
}

/// Byte `b` of the table as the C's `uint8_t *` view of it; nothing past
/// the table.
fn set_table_byte(table: &mut [u32; MAX_COLOURS], b: usize, value: u8) {
    if let Some(word) = table.get_mut(b / 4) {
        let mut bytes = word.to_ne_bytes();
        if let Some(slot) = bytes.get_mut(b % 4) {
            *slot = value;
        }
        *word = u32::from_ne_bytes(bytes);
    }
}

/// `nsgif_blocks__graphic_control`: `data` from the label on.
fn graphic_control(ext: &mut FrameExt, data: &[u8]) -> NsgifError {
    const MASK_TRANSPARENCY: u8 = 0x01;
    const MASK_DISPOSAL: u8 = 0x1c;
    // Label, size, packed fields, delay (2), transparent index.
    let Some(&[_, _, packed, d0, d1, index]) = data.get(..6) else {
        return NsgifError::EndOfData;
    };

    ext.delay = u32::from(u16::from_le_bytes([d0, d1]));
    if packed & MASK_TRANSPARENCY != 0 {
        ext.transparency = true;
        ext.transparency_index = u32::from(index);
    }
    ext.disposal = (packed & MASK_DISPOSAL) >> 2;
    // 4 in the wild for "restore previous": the quirk.
    if ext.disposal == disposal::RESTORE_QUIRK {
        ext.disposal = disposal::RESTORE_PREV;
    }
    ext.redraw_required =
        ext.disposal == disposal::RESTORE_BG || ext.disposal == disposal::RESTORE_PREV;
    NsgifError::Ok
}

/// `nsgif_blocks__application`: `data` from the label on.
fn application(loop_max: &mut i32, data: &[u8]) -> NsgifError {
    const LOOP_COUNT_BLOCK_SIZE: u8 = 0x0b;
    const LOOP_COUNT_SUB_BLOCK_SIZE: u8 = 0x03;
    const LOOP_COUNT_SUB_BLOCK_ID: u8 = 0x01;
    // Label, size, identifier (8) and code (3), then the first sub-block.
    let Some(head) = data.get(..17) else {
        return NsgifError::EndOfData;
    };
    let is_loop_count = head.get(1) == Some(&LOOP_COUNT_BLOCK_SIZE)
        && matches!(head.get(2..13), Some(b"NETSCAPE2.0" | b"ANIMEXTS1.0"));
    if let (true, &[.., size, id, lo, hi]) = (is_loop_count, head)
        && size == LOOP_COUNT_SUB_BLOCK_SIZE
        && id == LOOP_COUNT_SUB_BLOCK_ID
    {
        *loop_max = i32::from(u16::from_le_bytes([lo, hi]));
        // N means N repeats after the first play; 0 means forever.
        if *loop_max > 0 {
            *loop_max = loop_max.wrapping_add(1);
        }
    }
    NsgifError::Ok
}

/// `nsgif_blocks_header`: `GIF` and, when `strict`, `87a` or `89a`. Six
/// bytes from `pos`.
pub fn header(data: &[u8], pos: &mut usize, strict: bool) -> NsgifError {
    let Some(head) = data.get(*pos..).and_then(|rest| rest.get(..6)) else {
        return NsgifError::EndOfData;
    };
    if head.get(..3) != Some(b"GIF".as_slice()) {
        return NsgifError::Data;
    }
    if strict {
        let version = head.get(3..6);
        if version != Some(b"87a".as_slice()) && version != Some(b"89a".as_slice()) {
            return NsgifError::Data;
        }
    }
    *pos = pos.wrapping_add(6);
    NsgifError::Ok
}

/// `nsgif_blocks_screen`: the seven-byte logical screen descriptor. `out`
/// is set on [`NsgifError::Ok`] only.
pub fn screen(data: &[u8], pos: &mut usize, out: &mut Screen) -> NsgifError {
    let Some(&[w0, w1, h0, h1, packed, bg, aspect]) =
        data.get(*pos..).and_then(|rest| rest.get(..7))
    else {
        return NsgifError::EndOfData;
    };
    *out = Screen {
        width: u32::from(u16::from_le_bytes([w0, w1])),
        height: u32::from(u16::from_le_bytes([h0, h1])),
        global_palette: packed & COLOUR_TABLE_MASK != 0,
        colour_table_size: 2u32 << (packed & COLOUR_TABLE_SIZE_MASK),
        bg_index: u32::from(bg),
        aspect_ratio: u32::from(aspect),
        loop_max: 1,
    };
    *pos = pos.wrapping_add(7);
    NsgifError::Ok
}

/// `nsgif_blocks_screen_size_quirk`: the screen sizes that broken GIFs
/// report become 1x1.
pub fn screen_size_quirk(width: &mut u32, height: &mut u32) {
    let (w, h) = (*width, *height);
    if matches!(
        (w, h),
        (640, 480) | (640, 512) | (800, 600) | (1024, 768) | (1280, 1024) | (1600, 1200)
    ) || w == 0
        || h == 0
        || w > 2048
        || h > 2048
    {
        *width = 1;
        *height = 1;
    }
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
    let len = data.len();
    let mut p = *pos;
    // The C's pointer difference, end minus position, as an int: negative
    // once a skip has gone past the end.
    let remaining = |p: usize| (len as isize).wrapping_sub(p as isize) as i32;
    let mut bytes = remaining(p);

    while bytes > 0 && byte(data, p) == EXT_INTRODUCER {
        let mut block_step = true;

        p = p.wrapping_add(1);
        bytes = bytes.wrapping_sub(1);
        if bytes == 0 {
            return NsgifError::EndOfData;
        }

        let ext_data = data.get(p..).unwrap_or_default();
        match byte(data, p) {
            EXT_GRAPHIC_CONTROL => {
                if decode {
                    let ret = graphic_control(ext, ext_data);
                    if ret != NsgifError::Ok {
                        return ret;
                    }
                }
            }
            EXT_APPLICATION => {
                if decode {
                    let ret = application(loop_max, ext_data);
                    if ret != NsgifError::Ok {
                        return ret;
                    }
                }
            }
            EXT_COMMENT => {
                // To the first sub-block: past the label only.
                p = p.wrapping_add(1);
                block_step = false;
            }
            _ => {}
        }

        if block_step {
            // Past the label, the size byte and the size.
            if bytes < 2 {
                return NsgifError::EndOfData;
            }
            p = p
                .wrapping_add(2)
                .wrapping_add(usize::from(byte(data, p.wrapping_add(1))));
        }

        // The sub-blocks up to a zero-length one, or the end of the data.
        while p < len && byte(data, p) != BLOCK_TERMINATOR {
            p = p.wrapping_add(usize::from(byte(data, p))).wrapping_add(1);
            if p >= len {
                return NsgifError::EndOfData;
            }
        }
        p = p.wrapping_add(1);
        bytes = remaining(p);
    }

    *pos = p.min(len);
    NsgifError::Ok
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
    let Some(desc) = data
        .get(*pos..)
        .and_then(|rest| rest.get(..IMAGE_DESCRIPTOR_LEN))
    else {
        return NsgifError::EndOfData;
    };

    if decode {
        let &[separator, x0, x1, y0, y1, w0, w1, h0, h1, flags] = desc else {
            return NsgifError::EndOfData;
        };
        if separator != IMAGE_SEPARATOR {
            return NsgifError::DataFrame;
        }
        let x = u32::from(u16::from_le_bytes([x0, x1]));
        let y = u32::from(u16::from_le_bytes([y0, y1]));
        let w = u32::from(u16::from_le_bytes([w0, w1]));
        let h = u32::from(u16::from_le_bytes([h0, h1]));
        // 16-bit sums: no wrap in u32.
        let (right, bottom) = (x.wrapping_add(w), y.wrapping_add(h));
        *out = Image {
            flags: u32::from(flags),
            rect: Rect {
                x0: x,
                y0: y,
                x1: right,
                y1: bottom,
            },
            interlaced: flags & MASK_INTERLACE != 0,
        };

        // The first frame can grow the image.
        if first_frame {
            if right > *width {
                *width = right;
            }
            if bottom > *height {
                *height = bottom;
            }
        }
    }

    *pos = pos.wrapping_add(IMAGE_DESCRIPTOR_LEN);
    NsgifError::Ok
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
    let size = entries.saturating_mul(3);
    if data.len() < size {
        return NsgifError::EndOfData;
    }
    if decode {
        colour_table_decode(table, layout, entries, data);
    }
    *used = size;
    NsgifError::Ok
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
    let triplets = data.as_chunks::<3>().0.iter().take(entries);
    let [ro, go, bo, ao] = [layout.r, layout.g, layout.b, layout.a].map(usize::from);
    if ro < 4 && go < 4 && bo < 4 && ao < 4 {
        // Every component inside its entry (every layout gif.c makes): one
        // entry at a time, in the C's write order, each byte as a shift of
        // the native-endian word.
        let shift = |offset: usize| {
            let mut one = [0u8; 4];
            if let Some(slot) = one.get_mut(offset) {
                *slot = 1;
            }
            u32::from_ne_bytes(one).trailing_zeros()
        };
        let [rs, gs, bs, as_] = [ro, go, bo, ao].map(shift);
        let put = |word: u32, s: u32, value: u8| (word & !(0xFF << s)) | (u32::from(value) << s);
        for (word, &[r, g, b]) in table.iter_mut().zip(triplets) {
            *word = put(put(put(put(*word, rs, r), gs, g), bs, b), as_, 0xFF);
        }
        return;
    }
    for (entry, &[r, g, b]) in triplets.enumerate() {
        // Byte offsets into the whole table, as the C's byte pointer: an
        // offset past the entry is in the next one.
        let base = entry.wrapping_mul(4);
        for (offset, value) in [
            (layout.r, r),
            (layout.g, g),
            (layout.b, b),
            (layout.a, 0xFF),
        ] {
            set_table_byte(table, base.wrapping_add(usize::from(offset)), value);
        }
    }
}
