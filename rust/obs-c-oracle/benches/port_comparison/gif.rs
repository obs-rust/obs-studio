//! `libobs/graphics/libnsgif/gif.c.h` shims against the C oracle, through
//! the public API: an animated GIF of five 1920x1080 frames (the code
//! stream of the LZW bench, 256 global colours), scanned and every frame
//! decoded, as `gs_image_file` loads a GIF. The Rust side is all Rust, the C
//! side all C.

use core::ffi::{c_int, c_void};
use core::hint::black_box;
use core::ptr;

use criterion::Criterion;
use obs_c_oracle::gif as c;
use obs_graphics::ffi::gif::{
    BitmapCbVt, NsgifCtx, nsgif_create, nsgif_data_complete, nsgif_data_scan, nsgif_destroy,
    nsgif_frame_decode,
};

use crate::pair;

const FRAMES: u32 = 5;

unsafe extern "C" fn create(width: c_int, height: c_int) -> *mut c_void {
    let px = vec![0u32; (width.max(0) as usize) * (height.max(0) as usize)];
    Box::into_raw(Box::new(px)).cast()
}

unsafe extern "C" fn destroy(bitmap: *mut c_void) {
    // SAFETY: from `create`.
    drop(unsafe { Box::from_raw(bitmap.cast::<Vec<u32>>()) });
}

unsafe extern "C" fn get_buffer(bitmap: *mut c_void) -> *mut u8 {
    // SAFETY: from `create`.
    unsafe { (*bitmap.cast::<Vec<u32>>()).as_mut_ptr().cast() }
}

static VT: BitmapCbVt = BitmapCbVt {
    create: Some(create),
    destroy: Some(destroy),
    get_buffer: Some(get_buffer),
    set_opaque: None,
    test_opaque: None,
    modified: None,
    get_rowspan: None,
};

fn gif() -> Vec<u8> {
    let mut d = b"GIF89a".to_vec();
    d.extend_from_slice(&[0x80, 0x07, 0x38, 0x04, 0xF7, 0, 0]); // 1920x1080, 256 colours
    d.extend((0..256u32).flat_map(|i| [i as u8, (255 - i) as u8, (i * 7) as u8]));
    let stream = crate::lzw::stream();
    for _ in 0..FRAMES {
        d.extend_from_slice(&[0x21, 0xF9, 4, 1 << 2, 5, 0, 0, 0]);
        d.extend_from_slice(&[0x2C, 0, 0, 0, 0, 0x80, 0x07, 0x38, 0x04, 0]);
        d.push(8);
        d.extend_from_slice(&stream);
    }
    d.push(0x3B);
    d
}

pub fn benchmarks(criterion: &mut Criterion) {
    let data = gif();
    pair(
        criterion,
        "gif/scan_decode_5x1080p",
        || {
            let mut g: *mut NsgifCtx = ptr::null_mut();
            let mut sum = 0u64;
            // SAFETY: a live GIF over live data; local out-pointers.
            unsafe {
                nsgif_create(&VT, 0, &mut g);
                let r = nsgif_data_scan(g, data.len(), data.as_ptr());
                nsgif_data_complete(g);
                for f in 0..FRAMES {
                    let mut b = ptr::null_mut();
                    nsgif_frame_decode(g, f, &mut b);
                    sum += u64::from(*b.cast::<u32>().add(1920 * 1080 - 1));
                }
                nsgif_destroy(g);
                (r as i32, black_box(sum))
            }
        },
        || {
            let mut g: *mut c_void = ptr::null_mut();
            let mut sum = 0u64;
            // SAFETY: as above, through the C signatures.
            unsafe {
                c::oracle_nsgif_create((&raw const VT).cast(), 0, &mut g);
                let r = c::oracle_nsgif_data_scan(g, data.len(), data.as_ptr());
                c::oracle_nsgif_data_complete(g);
                for f in 0..FRAMES {
                    let mut b = ptr::null_mut();
                    c::oracle_nsgif_frame_decode(g, f, &mut b);
                    sum += u64::from(*b.cast::<u32>().add(1920 * 1080 - 1));
                }
                c::oracle_nsgif_destroy(g);
                (r, black_box(sum))
            }
        },
    );
}
