//! C ABI for `libobs/graphics/libnsgif/gif-frame.c.h`.
//!
//! `gif.c` (libnsgif, still C) calls these through `gif-frame.h`; they are
//! not `EXPORT`, so the shims stay hidden in libobs
//! (`libobs/cmake/rust-hidden.txt`). The decode takes the LZW context of
//! `lzw.h`, which in the hybrid libobs is the Rust [`LzwCtx`], and decodes
//! on its safe decoder directly.
//!
//! The bitmap holds `max(width, rowspan) * height` pixels: what the client
//! creates for the image (`width * height`, or `rowspan` per row when it
//! reports a row span). Every write is checked against that length.
//!
//! Intentional differences, where C dereferences null or writes past the
//! bitmap, and `gif.c` never gets there: a null context, canvas, frame or
//! colour table returns `NSGIF_ERR_DATA` (the restore and the recover do
//! nothing); a null bitmap or saved frame is empty; a data position past the
//! data is `NSGIF_ERR_END_OF_DATA`. The safe core lists the rest
//! ([`crate::gif_frame`]).

use crate::ffi::lzw::LzwCtx;
use crate::gif_blocks::{MAX_COLOURS, NsgifError};
use crate::gif_frame::{self, Canvas, FrameArea};

/// `struct nsgif_canvas`.
#[repr(C)]
pub struct CCanvas {
    pub bitmap: *mut u32,
    pub width: u32,
    pub height: u32,
    pub rowspan: u32,
    pub background: u32,
}

impl CCanvas {
    fn canvas(&self) -> Canvas {
        Canvas {
            width: self.width,
            height: self.height,
            rowspan: self.rowspan,
            background: self.background,
        }
    }

    /// The bitmap's pixels.
    ///
    /// # Safety
    ///
    /// `bitmap` is null or holds `max(width, rowspan) * height` writable
    /// pixels for `'a`.
    unsafe fn pixels<'a>(&self) -> &'a mut [u32] {
        if self.bitmap.is_null() {
            return &mut [];
        }
        let len = (self.width.max(self.rowspan) as usize).saturating_mul(self.height as usize);
        // SAFETY: the caller's contract above.
        unsafe { core::slice::from_raw_parts_mut(self.bitmap, len) }
    }
}

/// `nsgif_frame_pixels_decode()`.
///
/// # Safety
///
/// `lzw` is null or a live context from `lzw_context_create`; `buf` is null
/// or points to `buf_len` readable bytes; `canvas` and `frame` are null or
/// valid, with the bitmap as in the module docs; `colour_table` is null or
/// points to `NSGIF_MAX_COLOURS` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_frame_pixels_decode(
    lzw: *mut LzwCtx,
    buf: *const u8,
    buf_len: usize,
    data_pos: usize,
    canvas: *const CCanvas,
    frame: *const FrameArea,
    colour_table: *const u32,
    data_complete: bool,
) -> NsgifError {
    // SAFETY: the caller's contract above.
    let (Some(lzw), Some(canvas), Some(frame), Some(colours)) = (
        unsafe { lzw.as_mut() },
        unsafe { canvas.as_ref() },
        unsafe { frame.as_ref() },
        unsafe { colour_table.cast::<[u32; MAX_COLOURS]>().as_ref() },
    ) else {
        return NsgifError::Data;
    };
    let data = if buf.is_null() {
        &[][..]
    } else {
        // SAFETY: `buf_len` readable bytes (the contract).
        unsafe { core::slice::from_raw_parts(buf, buf_len) }
    };
    // SAFETY: the bitmap contract.
    let pixels = unsafe { canvas.pixels() };
    gif_frame::decode(
        lzw.decoder_mut(),
        data,
        data_pos,
        &canvas.canvas(),
        pixels,
        frame,
        colours,
        data_complete,
    )
}

/// `nsgif_frame_restore_bg()`.
///
/// # Safety
///
/// `canvas` is null or valid, with the bitmap as in the module docs;
/// `frame` is null (the whole image) or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_frame_restore_bg(canvas: *const CCanvas, frame: *const FrameArea) {
    // SAFETY: the caller's contract above.
    if let Some(canvas) = unsafe { canvas.as_ref() } {
        // SAFETY: the bitmap contract; `frame` null or valid.
        let (pixels, frame) = unsafe { (canvas.pixels(), frame.as_ref()) };
        gif_frame::restore_bg(&canvas.canvas(), pixels, frame);
    }
}

/// `nsgif_frame_recover()`.
///
/// # Safety
///
/// `canvas` is null or valid, with the bitmap as in the module docs;
/// `prev_frame` is null or points to `width * height` readable pixels.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_frame_recover(canvas: *const CCanvas, prev_frame: *const u32) {
    // SAFETY: the caller's contract above.
    let Some(canvas) = (unsafe { canvas.as_ref() }) else {
        return;
    };
    let saved = if prev_frame.is_null() {
        &[][..]
    } else {
        let len = (canvas.width as usize).saturating_mul(canvas.height as usize);
        // SAFETY: `width * height` readable pixels (the contract).
        unsafe { core::slice::from_raw_parts(prev_frame, len) }
    };
    // SAFETY: the bitmap contract.
    gif_frame::recover(&canvas.canvas(), unsafe { canvas.pixels() }, saved);
}
