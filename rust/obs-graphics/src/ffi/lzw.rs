//! C ABI for `libobs/graphics/libnsgif/lzw.c.h`.
//!
//! `gif.c` (libnsgif, still C) calls these through `lzw.h`; they are not
//! `EXPORT`, so the shims stay hidden in libobs
//! (`libobs/cmake/rust-hidden.txt`). `struct lzw_ctx` is opaque to `gif.c`:
//! here it is [`LzwCtx`], the safe [`Decoder`] plus the two pointers the C
//! context keeps, the input buffer from init and the colour table. It is a
//! zeroed allocation, so `lzw_context_create` returns `LZW_NO_MEM` when the
//! allocation fails, as the C's `malloc` does.
//!
//! Intentional differences, where C dereferences null or reads memory it
//! never initialized, and `gif.c` never gets there: a null `ctx`, output
//! pointer or `output_written` returns `LZW_BAD_PARAM`; a null input reads
//! as empty; decoding before any init reads no data (`LZW_NO_DATA`, or
//! `LZW_NO_COLOUR` for the map); decoding on after `LZW_BAD_ICODE` from a
//! clear code inside the data reads zeroed entries, where the C follows
//! links it never initialized (a wild read; `lzw_parity` has the details).
//! The safe core lists the rest ([`crate::lzw`]).

use core::ptr;
use std::alloc::{Layout, alloc_zeroed};

use crate::lzw::{Decoder, LZW_CODE_MAX, LzwResult};

/// `struct lzw_ctx`.
pub struct LzwCtx {
    decoder: Decoder,
    data: *const u8,
    data_len: usize,
    colour_map: *const u32,
}

/// The input from init.
///
/// # Safety
///
/// `data` is null or points to `len` readable bytes that stay readable for
/// `'a` (the contract of the init functions: the buffer outlives the
/// decode).
unsafe fn input<'a>(data: *const u8, len: usize) -> &'a [u8] {
    if data.is_null() {
        &[]
    } else {
        // SAFETY: the caller's contract above.
        unsafe { core::slice::from_raw_parts(data, len) }
    }
}

impl LzwCtx {
    /// The pointers init stores before it can fail, as the C does.
    fn store_input(&mut self, minimum_code_size: u8, data: *const u8, len: usize) {
        if minimum_code_size < LZW_CODE_MAX {
            self.data = data;
            self.data_len = len;
        }
    }
}

/// `lzw_context_create()`.
///
/// # Safety
///
/// `ctx` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lzw_context_create(ctx: *mut *mut LzwCtx) -> LzwResult {
    if ctx.is_null() {
        return LzwResult::BadParam;
    }
    // SAFETY: a non-zero-sized layout. An all-zero `LzwCtx` is valid: its
    // fields are integers, bools, arrays of them and raw pointers.
    let c = unsafe { alloc_zeroed(Layout::new::<LzwCtx>()) }.cast::<LzwCtx>();
    if c.is_null() {
        return LzwResult::NoMem;
    }
    // SAFETY: `ctx` is writable (the contract).
    unsafe { ptr::write(ctx, c) };
    LzwResult::Ok
}

/// `lzw_context_destroy()`.
///
/// # Safety
///
/// `ctx` is null or from `lzw_context_create`, and not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lzw_context_destroy(ctx: *mut LzwCtx) {
    if !ctx.is_null() {
        // SAFETY: allocated by the global allocator with this layout in
        // `lzw_context_create`; `LzwCtx` has no drop glue.
        drop(unsafe { Box::from_raw(ctx) });
    }
}

/// `lzw_decode_init()`.
///
/// # Safety
///
/// `ctx` is null or from `lzw_context_create`; `input_data` is null or
/// points to `input_length` bytes that stay readable while the context
/// decodes them.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lzw_decode_init(
    ctx: *mut LzwCtx,
    minimum_code_size: u8,
    input_data: *const u8,
    input_length: usize,
    input_pos: usize,
) -> LzwResult {
    // SAFETY: null or a live context (the contract).
    let Some(c) = (unsafe { ctx.as_mut() }) else {
        return LzwResult::BadParam;
    };
    c.store_input(minimum_code_size, input_data, input_length);
    // SAFETY: the input contract.
    let data = unsafe { input(c.data, c.data_len) };
    let res = c.decoder.init(data, minimum_code_size, input_pos);
    if res == LzwResult::Ok {
        c.colour_map = ptr::null();
    }
    res
}

/// `lzw_decode_init_map()`.
///
/// # Safety
///
/// As [`lzw_decode_init`]; `colour_table` is null or points to 256 values
/// (`NSGIF_MAX_COLOURS`) that stay readable while the context decodes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lzw_decode_init_map(
    ctx: *mut LzwCtx,
    minimum_code_size: u8,
    transparency_idx: u32,
    colour_table: *const u32,
    input_data: *const u8,
    input_length: usize,
    input_pos: usize,
) -> LzwResult {
    if colour_table.is_null() {
        return LzwResult::BadParam;
    }
    // SAFETY: null or a live context (the contract).
    let Some(c) = (unsafe { ctx.as_mut() }) else {
        return LzwResult::BadParam;
    };
    c.store_input(minimum_code_size, input_data, input_length);
    // SAFETY: the input contract.
    let data = unsafe { input(c.data, c.data_len) };
    let res = c
        .decoder
        .init_map(data, minimum_code_size, transparency_idx, input_pos);
    if res == LzwResult::Ok {
        c.colour_map = colour_table;
    }
    res
}

/// `lzw_decode()`.
///
/// # Safety
///
/// `ctx` is null or a live context; `output_data` and `output_written` are
/// null or writable. The values stay valid until the next call on `ctx`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lzw_decode(
    ctx: *mut LzwCtx,
    output_data: *mut *const u8,
    output_written: *mut u32,
) -> LzwResult {
    // SAFETY: null or a live context (the contract).
    let Some(c) = (unsafe { ctx.as_mut() }) else {
        return LzwResult::BadParam;
    };
    if output_data.is_null() || output_written.is_null() {
        return LzwResult::BadParam;
    }
    // SAFETY: the input contract of the init functions.
    let data = unsafe { input(c.data, c.data_len) };
    let (res, written) = c.decoder.decode(data);
    // SAFETY: both writable (the contract); the stack lives in `*ctx`.
    unsafe {
        ptr::write(output_data, c.decoder.output().as_ptr());
        ptr::write(output_written, written);
    }
    res
}

/// `lzw_decode_map()`.
///
/// # Safety
///
/// `ctx` is null or a live context; `output_data` is null or points to
/// `output_length` writable values; `output_written` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lzw_decode_map(
    ctx: *mut LzwCtx,
    output_data: *mut u32,
    output_length: u32,
    output_written: *mut u32,
) -> LzwResult {
    // SAFETY: null or a live context (the contract).
    let Some(c) = (unsafe { ctx.as_mut() }) else {
        return LzwResult::BadParam;
    };
    if output_written.is_null() || (output_data.is_null() && output_length != 0) {
        return LzwResult::BadParam;
    }
    // SAFETY: the input contract of the init functions.
    let data = unsafe { input(c.data, c.data_len) };
    let colours = if c.colour_map.is_null() {
        None
    } else {
        // SAFETY: 256 readable values (the init_map contract).
        Some(unsafe { &*c.colour_map.cast::<[u32; 256]>() })
    };
    let out: &mut [u32] = if output_length == 0 {
        &mut []
    } else {
        // SAFETY: `output_length` writable values (the contract).
        unsafe { core::slice::from_raw_parts_mut(output_data, output_length as usize) }
    };
    let (res, written) = c.decoder.decode_map(data, colours, out);
    // SAFETY: writable (the contract).
    unsafe { ptr::write(output_written, written) };
    res
}
