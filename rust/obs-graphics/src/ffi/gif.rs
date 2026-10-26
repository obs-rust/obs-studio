//! C ABI for `libobs/graphics/libnsgif/gif.c.h`: the public API of
//! `nsgif.h`.
//!
//! `gs_image_file` (`libobs/graphics/image-file.c.h` and its Rust port)
//! calls these; they are not `EXPORT`, so the shims stay hidden in libobs
//! (`libobs/cmake/rust-hidden.txt`). `nsgif_t` is opaque to its callers:
//! here it is [`NsgifCtx`], the safe [`Gif`] plus the buffer of the last
//! scan, which the C keeps too and the decodes read. The bitmap callbacks
//! are [`BitmapCbVt`], `nsgif_bitmap_cb_vt` as the client passes it.
//!
//! A bitmap from `get_buffer` holds `max(width, rowspan) * height` pixels,
//! 4-byte aligned, as the C casts it to `uint32_t *`.
//!
//! Intentional differences, where C dereferences null and no caller gets
//! there: a null `nsgif_t`, callback table or out-pointer makes
//! `nsgif_create` return `NSGIF_ERR_DATA` and the other calls do nothing
//! (or return `NSGIF_ERR_DATA`, `NULL`, `false`); a missing `create` or
//! `get_buffer` callback is a bitmap that cannot be made (`NSGIF_ERR_OOM`)
//! or has no pixels. The safe core lists the rest ([`crate::gif`]).

use core::ffi::{c_char, c_int, c_void};
use core::ptr;

use crate::gif::{self, Bitmaps, FrameInfo, Gif, Info};
use crate::gif_blocks::{MAX_COLOURS, NsgifError, Rect};

/// `nsgif_bitmap_cb_vt`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BitmapCbVt {
    pub create: Option<unsafe extern "C" fn(width: c_int, height: c_int) -> *mut c_void>,
    pub destroy: Option<unsafe extern "C" fn(bitmap: *mut c_void)>,
    pub get_buffer: Option<unsafe extern "C" fn(bitmap: *mut c_void) -> *mut u8>,
    pub set_opaque: Option<unsafe extern "C" fn(bitmap: *mut c_void, opaque: bool)>,
    pub test_opaque: Option<unsafe extern "C" fn(bitmap: *mut c_void) -> bool>,
    pub modified: Option<unsafe extern "C" fn(bitmap: *mut c_void)>,
    pub get_rowspan: Option<unsafe extern "C" fn(bitmap: *mut c_void) -> u32>,
}

/// `nsgif_bitmap_t *`, a handle the client made with `create`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CBitmap(*mut c_void);

impl Bitmaps for BitmapCbVt {
    type Bitmap = CBitmap;

    fn create(&mut self, width: u32, height: u32) -> Option<CBitmap> {
        let create = self.create?;
        // SAFETY: the client's callback (the nsgif_create contract).
        let bitmap = unsafe { create(width as c_int, height as c_int) };
        (!bitmap.is_null()).then_some(CBitmap(bitmap))
    }

    fn destroy(&mut self, CBitmap(bitmap): CBitmap) {
        if let Some(destroy) = self.destroy {
            // SAFETY: a bitmap from `create`, destroyed once.
            unsafe { destroy(bitmap) };
        }
    }

    fn pixels(
        &mut self,
        CBitmap(bitmap): CBitmap,
        width: u32,
        height: u32,
        rowspan: u32,
    ) -> &mut [u32] {
        let Some(get_buffer) = self.get_buffer else {
            return &mut [];
        };
        // SAFETY: a bitmap from `create` (the contract).
        let buffer = unsafe { get_buffer(bitmap) };
        if buffer.is_null() {
            return &mut [];
        }
        let len = (width.max(rowspan) as usize).saturating_mul(height as usize);
        // SAFETY: `len` aligned pixels in the client's bitmap (the module
        // docs), live until the bitmap is destroyed.
        unsafe { core::slice::from_raw_parts_mut(buffer.cast::<u32>(), len) }
    }

    fn rowspan(&mut self, CBitmap(bitmap): CBitmap) -> Option<u32> {
        // SAFETY: the client's callback on its bitmap.
        self.get_rowspan.map(|f| unsafe { f(bitmap) })
    }

    fn set_opaque(&mut self, CBitmap(bitmap): CBitmap, opaque: bool) {
        if let Some(f) = self.set_opaque {
            // SAFETY: the client's callback on its bitmap.
            unsafe { f(bitmap, opaque) };
        }
    }

    fn test_opaque(&mut self, CBitmap(bitmap): CBitmap) -> Option<bool> {
        // SAFETY: the client's callback on its bitmap.
        self.test_opaque.map(|f| unsafe { f(bitmap) })
    }

    fn modified(&mut self, CBitmap(bitmap): CBitmap) {
        if let Some(f) = self.modified {
            // SAFETY: the client's callback on its bitmap.
            unsafe { f(bitmap) };
        }
    }
}

/// `struct nsgif`.
pub struct NsgifCtx {
    gif: Gif<BitmapCbVt>,
    buf: *const u8,
    buf_len: usize,
}

impl NsgifCtx {
    /// The buffer of the last scan.
    ///
    /// # Safety
    ///
    /// The client keeps that buffer readable while it uses the GIF (the
    /// `nsgif_data_scan` contract).
    unsafe fn data(&self) -> &[u8] {
        if self.buf.is_null() {
            &[]
        } else {
            // SAFETY: the caller's contract above.
            unsafe { core::slice::from_raw_parts(self.buf, self.buf_len) }
        }
    }
}

/// `nsgif_create()`.
///
/// # Safety
///
/// `bitmap_vt` is null or a valid callback table; `gif_out` is null or
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_create(
    bitmap_vt: *const BitmapCbVt,
    bitmap_fmt: u32,
    gif_out: *mut *mut NsgifCtx,
) -> NsgifError {
    // SAFETY: null or valid (the contract).
    let Some(vt) = (unsafe { bitmap_vt.as_ref() }) else {
        return NsgifError::Data;
    };
    if gif_out.is_null() {
        return NsgifError::Data;
    }
    let ctx = Box::new(NsgifCtx {
        gif: Gif::new(*vt, bitmap_fmt),
        buf: ptr::null(),
        buf_len: 0,
    });
    // SAFETY: writable (the contract).
    unsafe { ptr::write(gif_out, Box::into_raw(ctx)) };
    NsgifError::Ok
}

/// `nsgif_destroy()`.
///
/// # Safety
///
/// `gif` is null or from `nsgif_create`, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_destroy(gif: *mut NsgifCtx) {
    if !gif.is_null() {
        // SAFETY: from `nsgif_create` (Box::into_raw), freed once.
        drop(unsafe { Box::from_raw(gif) });
    }
}

/// `nsgif_set_frame_delay_behaviour()`.
///
/// # Safety
///
/// `gif` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_set_frame_delay_behaviour(
    gif: *mut NsgifCtx,
    delay_min: u16,
    delay_default: u16,
) {
    // SAFETY: null or live (the contract).
    if let Some(c) = unsafe { gif.as_mut() } {
        c.gif.set_frame_delay_behaviour(delay_min, delay_default);
    }
}

/// `nsgif_data_scan()`.
///
/// # Safety
///
/// `gif` is null or live; `data` points to `size` readable bytes that stay
/// readable while the GIF is used, as the C requires.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_data_scan(
    gif: *mut NsgifCtx,
    size: usize,
    data: *const u8,
) -> NsgifError {
    // SAFETY: null or live (the contract).
    let Some(c) = (unsafe { gif.as_mut() }) else {
        return NsgifError::Data;
    };
    let slice = if data.is_null() {
        &[][..]
    } else {
        // SAFETY: `size` readable bytes (the contract).
        unsafe { core::slice::from_raw_parts(data, size) }
    };
    let ret = c.gif.data_scan(slice);
    // The C stores the buffer after its data-complete check.
    if ret != NsgifError::DataComplete {
        c.buf = data;
        c.buf_len = size;
    }
    ret
}

/// `nsgif_data_complete()`.
///
/// # Safety
///
/// `gif` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_data_complete(gif: *mut NsgifCtx) {
    // SAFETY: null or live (the contract).
    if let Some(c) = unsafe { gif.as_mut() } {
        c.gif.data_complete();
    }
}

/// `nsgif_reset()`.
///
/// # Safety
///
/// `gif` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_reset(gif: *mut NsgifCtx) -> NsgifError {
    // SAFETY: null or live (the contract).
    match unsafe { gif.as_mut() } {
        Some(c) => c.gif.reset(),
        None => NsgifError::Data,
    }
}

/// `nsgif_frame_prepare()`.
///
/// # Safety
///
/// `gif` is null or live; the three out-pointers are null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_frame_prepare(
    gif: *mut NsgifCtx,
    area: *mut Rect,
    delay_cs: *mut u32,
    frame_new: *mut u32,
) -> NsgifError {
    // SAFETY: null or live (the contract).
    let Some(c) = (unsafe { gif.as_mut() }) else {
        return NsgifError::Data;
    };
    if area.is_null() || delay_cs.is_null() || frame_new.is_null() {
        return NsgifError::Data;
    }
    match c.gif.frame_prepare() {
        Ok((rect, delay, frame)) => {
            // SAFETY: writable (the contract).
            unsafe {
                ptr::write(frame_new, frame);
                ptr::write(delay_cs, delay);
                ptr::write(area, rect);
            }
            NsgifError::Ok
        }
        Err(err) => err,
    }
}

/// `nsgif_frame_decode()`.
///
/// # Safety
///
/// `gif` is null or live, with the buffer of its last scan still readable;
/// `bitmap` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_frame_decode(
    gif: *mut NsgifCtx,
    frame: u32,
    bitmap: *mut *mut c_void,
) -> NsgifError {
    // SAFETY: null or live (the contract).
    let Some(c) = (unsafe { gif.as_mut() }) else {
        return NsgifError::Data;
    };
    // The buffer of the last scan, from a copy of the pointer, so the
    // decode can borrow the state mutably.
    let (buf, buf_len) = (c.buf, c.buf_len);
    let data = if buf.is_null() {
        &[][..]
    } else {
        // SAFETY: the buffer of the last scan (the contract).
        unsafe { core::slice::from_raw_parts(buf, buf_len) }
    };
    match c.gif.frame_decode(data, frame) {
        Ok(image) => {
            if !bitmap.is_null() {
                // SAFETY: writable (the contract).
                unsafe { ptr::write(bitmap, image.map_or(ptr::null_mut(), |CBitmap(b)| b)) };
            }
            NsgifError::Ok
        }
        Err(err) => err,
    }
}

/// `nsgif_get_info()`.
///
/// # Safety
///
/// `gif` is null or live; the info stays valid until `nsgif_destroy`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_get_info(gif: *const NsgifCtx) -> *const Info {
    // SAFETY: null or live (the contract).
    match unsafe { gif.as_ref() } {
        Some(c) => c.gif.info(),
        None => ptr::null(),
    }
}

/// `nsgif_get_frame_info()`.
///
/// # Safety
///
/// `gif` is null or live; the frame info stays valid until the next scan.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_get_frame_info(
    gif: *const NsgifCtx,
    frame: u32,
) -> *const FrameInfo {
    // SAFETY: null or live (the contract).
    unsafe { gif.as_ref() }
        .and_then(|c| c.gif.frame_info(frame))
        .map_or(ptr::null(), ptr::from_ref)
}

/// `nsgif_global_palette()`.
///
/// # Safety
///
/// `gif` is null or live; `table` is null or holds `NSGIF_MAX_COLOURS`
/// writable entries; `entries` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_global_palette(
    gif: *const NsgifCtx,
    table: *mut [u32; MAX_COLOURS],
    entries: *mut usize,
) {
    // SAFETY: null or valid (the contract).
    if let (Some(c), Some(table), false) = (
        unsafe { gif.as_ref() },
        unsafe { table.as_mut() },
        entries.is_null(),
    ) {
        let n = c.gif.global_palette(table);
        // SAFETY: writable (the contract).
        unsafe { ptr::write(entries, n) };
    }
}

/// `nsgif_local_palette()`.
///
/// # Safety
///
/// As [`nsgif_global_palette`], with the buffer of the last scan still
/// readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nsgif_local_palette(
    gif: *const NsgifCtx,
    frame: u32,
    table: *mut [u32; MAX_COLOURS],
    entries: *mut usize,
) -> bool {
    // SAFETY: null or valid (the contract).
    let (Some(c), Some(table), false) = (
        unsafe { gif.as_ref() },
        unsafe { table.as_mut() },
        entries.is_null(),
    ) else {
        return false;
    };
    // SAFETY: the buffer of the last scan (the contract).
    let data = unsafe { c.data() };
    match c.gif.local_palette(data, frame, table) {
        Some(n) => {
            // SAFETY: writable (the contract).
            unsafe { ptr::write(entries, n) };
            true
        }
        None => false,
    }
}

/// `nsgif_strerror()`.
#[unsafe(no_mangle)]
pub extern "C" fn nsgif_strerror(err: u32) -> *const c_char {
    gif::strerror(err).as_ptr()
}

/// `nsgif_str_disposal()`.
#[unsafe(no_mangle)]
pub extern "C" fn nsgif_str_disposal(disposal: u32) -> *const c_char {
    gif::str_disposal(disposal).as_ptr()
}
