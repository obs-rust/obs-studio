//! C ABI for `libobs/graphics/libnsgif/gif-blocks.c.h`.
//!
//! `gif.c` (libnsgif, still C) calls these through `gif-blocks.h`; they
//! are not `EXPORT`, so the shims stay hidden in libobs
//! (`libobs/cmake/rust-hidden.txt`). The result structs are the safe
//! core's `#[repr(C)]` types ([`Screen`], [`FrameExt`], [`Image`],
//! [`ColourLayout`]).
//!
//! Intentional differences, where C dereferences null or computes a length
//! that wraps, and `gif.c` never gets there: a null `pos`, result or
//! layout pointer returns `NSGIF_ERR_DATA` (the enum has no parameter
//! error) and writes nothing; a null data pointer reads as empty; a
//! position past the data reads nothing (`NSGIF_ERR_END_OF_DATA`, and
//! [`nsgif_blocks_extensions`] returns the end of the data). The safe core
//! lists the rest ([`crate::gif_blocks`]).

use crate::gif_blocks::{self, ColourLayout, FrameExt, Image, MAX_COLOURS, NsgifError, Screen};

/// The GIF data.
///
/// # Safety
///
/// `data` is null or points to `len` readable bytes for `'a`.
unsafe fn input<'a>(data: *const u8, len: usize) -> &'a [u8] {
    if data.is_null() {
        &[]
    } else {
        // SAFETY: the caller's contract above.
        unsafe { core::slice::from_raw_parts(data, len) }
    }
}

/// `nsgif_blocks_header()`.
///
/// # Safety
///
/// `data` points to `len` readable bytes (or is null); `pos` is null or
/// valid for reads and writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_blocks_header(
    data: *const u8,
    len: usize,
    pos: *mut usize,
    strict: bool,
) -> NsgifError {
    // SAFETY: the caller's contract above.
    let (Some(pos), data) = (unsafe { pos.as_mut() }, unsafe { input(data, len) }) else {
        return NsgifError::Data;
    };
    gif_blocks::header(data, pos, strict)
}

/// `nsgif_blocks_screen()`.
///
/// # Safety
///
/// As [`nsgif_blocks_header`]; `out` is null or valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_blocks_screen(
    data: *const u8,
    len: usize,
    pos: *mut usize,
    out: *mut Screen,
) -> NsgifError {
    // SAFETY: the caller's contract above.
    let (Some(pos), Some(out), data) = (unsafe { pos.as_mut() }, unsafe { out.as_mut() }, unsafe {
        input(data, len)
    }) else {
        return NsgifError::Data;
    };
    gif_blocks::screen(data, pos, out)
}

/// `nsgif_blocks_screen_size_quirk()`.
///
/// # Safety
///
/// `width` and `height` are null or valid for reads and writes; with
/// either null, nothing changes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_blocks_screen_size_quirk(width: *mut u32, height: *mut u32) {
    // SAFETY: the caller's contract above.
    if let (Some(width), Some(height)) = (unsafe { width.as_mut() }, unsafe { height.as_mut() }) {
        gif_blocks::screen_size_quirk(width, height);
    }
}

/// `nsgif_blocks_extensions()`.
///
/// # Safety
///
/// As [`nsgif_blocks_header`]; `ext` and `loop_max` are null or valid for
/// reads and writes, and `ext` holds a valid `struct nsgif_blocks_frame_ext`
/// (its `bool` fields 0 or 1).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_blocks_extensions(
    data: *const u8,
    len: usize,
    pos: *mut usize,
    decode: bool,
    ext: *mut FrameExt,
    loop_max: *mut i32,
) -> NsgifError {
    // SAFETY: the caller's contract above.
    let (Some(pos), Some(ext), Some(loop_max), data) = (
        unsafe { pos.as_mut() },
        unsafe { ext.as_mut() },
        unsafe { loop_max.as_mut() },
        unsafe { input(data, len) },
    ) else {
        return NsgifError::Data;
    };
    gif_blocks::extensions(data, pos, decode, ext, loop_max)
}

/// `nsgif_blocks_image_descriptor()`.
///
/// # Safety
///
/// As [`nsgif_blocks_header`]; `out` is null or valid for writes, `width`
/// and `height` null or valid for reads and writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_blocks_image_descriptor(
    data: *const u8,
    len: usize,
    pos: *mut usize,
    decode: bool,
    first_frame: bool,
    out: *mut Image,
    width: *mut u32,
    height: *mut u32,
) -> NsgifError {
    // SAFETY: the caller's contract above.
    let (Some(pos), Some(out), Some(width), Some(height), data) = (
        unsafe { pos.as_mut() },
        unsafe { out.as_mut() },
        unsafe { width.as_mut() },
        unsafe { height.as_mut() },
        unsafe { input(data, len) },
    ) else {
        return NsgifError::Data;
    };
    gif_blocks::image_descriptor(data, pos, decode, first_frame, out, width, height)
}

/// `nsgif_blocks_colour_table()`.
///
/// # Safety
///
/// `colour_table` is null or valid for writes of `NSGIF_MAX_COLOURS`
/// entries; `layout` and `used` null or valid; `data` points to `data_len`
/// readable bytes (or is null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_blocks_colour_table(
    colour_table: *mut [u32; MAX_COLOURS],
    layout: *const ColourLayout,
    entries: usize,
    data: *const u8,
    data_len: usize,
    used: *mut usize,
    decode: bool,
) -> NsgifError {
    // SAFETY: the caller's contract above.
    let (Some(table), Some(layout), Some(used), data) = (
        unsafe { colour_table.as_mut() },
        unsafe { layout.as_ref() },
        unsafe { used.as_mut() },
        unsafe { input(data, data_len) },
    ) else {
        return NsgifError::Data;
    };
    gif_blocks::colour_table(table, *layout, entries, data, used, decode)
}

/// `nsgif_blocks_colour_table_decode()`.
///
/// # Safety
///
/// `colour_table` is null or valid for writes of `NSGIF_MAX_COLOURS`
/// entries; `layout` null or valid; `data` is null or points to `entries`
/// readable r,g,b triplets (the C checks no length either).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_blocks_colour_table_decode(
    colour_table: *mut [u32; MAX_COLOURS],
    layout: *const ColourLayout,
    entries: usize,
    data: *const u8,
) {
    // SAFETY: the caller's contract above.
    if let (Some(table), Some(layout), data) = (
        unsafe { colour_table.as_mut() },
        unsafe { layout.as_ref() },
        unsafe { input(data, entries.saturating_mul(3)) },
    ) {
        gif_blocks::colour_table_decode(table, *layout, entries, data);
    }
}
