//! `libobs/graphics/libnsgif/lzw.c.h` shims against the C oracle. The
//! fixture is a whole 1920x1080 GIF frame through `lzw_decode_map`, as
//! `gif.c` decodes it: a code stream with 8-bit roots, records of 2 to 40
//! values and a clear whenever the table is full, in 255-byte sub-blocks.

use core::ffi::c_void;
use core::ptr;
use std::hint::black_box;

use criterion::Criterion;
use obs_c_oracle::lzw as c_lzw;
use obs_graphics::ffi::lzw::{
    LzwCtx, lzw_context_create, lzw_context_destroy, lzw_decode_init_map, lzw_decode_map,
};

use crate::pair;

const PIXELS: usize = 1920 * 1080;

/// Codes with the decoder's width rule (as the emitter of the Tier 1 tests).
fn stream() -> Vec<u8> {
    let (min, clear, eoi) = (8u32, 256u32, 257u32);
    let mut raw = vec![0u8; PIXELS * 2];
    let mut bits = 0usize;
    let mut put = |code: u32, width: u32, bits: &mut usize| {
        for i in 0..width {
            if (code >> i) & 1 != 0 {
                raw[*bits >> 3] |= 1 << (*bits & 7);
            }
            *bits += 1;
        }
    };
    let (mut width, mut table_size, mut first) = (min + 1, eoi + 1, true);
    // Record lengths of the table entries, to count the pixels.
    let mut lengths = vec![1u32; 4096];
    let (mut pixels, mut prev_len, mut n) = (0usize, 1u32, 0u32);
    put(clear, width, &mut bits);
    while pixels < PIXELS {
        n = n.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let code = if table_size > eoi + 1 && n % 4 == 0 {
            eoi + 1 + (n >> 8) % (table_size - eoi - 1)
        } else {
            (n >> 16) % 256
        };
        let len = if code > eoi {
            lengths[code as usize]
        } else {
            1
        };
        if len > 40 {
            continue;
        }
        put(code, width, &mut bits);
        pixels += len as usize;
        if first {
            first = false;
        } else if table_size < 4096 {
            lengths[table_size as usize] = prev_len + 1;
            if table_size == (1 << width) - 1 && width < 12 {
                width += 1;
            }
            table_size += 1;
        } else {
            put(clear, width, &mut bits);
            width = min + 1;
            table_size = eoi + 1;
            first = true;
        }
        prev_len = len;
    }
    put(eoi, width, &mut bits);
    let bytes = bits.div_ceil(8);
    let mut out = Vec::with_capacity(bytes + bytes / 255 + 2);
    for chunk in raw[..bytes].chunks(255) {
        out.push(chunk.len() as u8);
        out.extend_from_slice(chunk);
    }
    out.push(0);
    out
}

pub fn benchmarks(criterion: &mut Criterion) {
    let data = stream();
    let colours: Vec<u32> = (0..256u32)
        .map(|i| 0xff00_0000 | (i * 0x0001_0101))
        .collect();
    let mut rust_px = vec![0u32; PIXELS];
    let mut c_px = vec![0u32; PIXELS];
    let mut rust_ctx: *mut LzwCtx = ptr::null_mut();
    let mut c_ctx: *mut c_void = ptr::null_mut();
    // SAFETY: valid out-pointers.
    unsafe {
        lzw_context_create(&mut rust_ctx);
        c_lzw::oracle_lzw_context_create(&mut c_ctx);
    }
    pair(
        criterion,
        "lzw/decode_map_1080p",
        || {
            let mut written = 0u32;
            // SAFETY: a live context, a 256-value table, `PIXELS` outputs.
            unsafe {
                lzw_decode_init_map(
                    rust_ctx,
                    8,
                    256,
                    colours.as_ptr(),
                    data.as_ptr(),
                    data.len(),
                    0,
                );
                lzw_decode_map(rust_ctx, rust_px.as_mut_ptr(), PIXELS as u32, &mut written);
            }
            (written, black_box(rust_px[PIXELS - 1]))
        },
        || {
            let mut written = 0u32;
            // SAFETY: as above, through the C signature.
            unsafe {
                c_lzw::oracle_lzw_decode_init_map(
                    c_ctx,
                    8,
                    256,
                    colours.as_ptr(),
                    data.as_ptr(),
                    data.len(),
                    0,
                );
                c_lzw::oracle_lzw_decode_map(c_ctx, c_px.as_mut_ptr(), PIXELS as u32, &mut written);
            }
            (written, black_box(c_px[PIXELS - 1]))
        },
    );
    // SAFETY: from the create functions, not used afterwards.
    unsafe {
        lzw_context_destroy(rust_ctx);
        c_lzw::oracle_lzw_context_destroy(c_ctx);
    }
}
