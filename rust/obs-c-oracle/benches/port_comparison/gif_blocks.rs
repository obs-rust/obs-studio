//! `libobs/graphics/libnsgif/gif-blocks.c.h` shims against the C oracle.
//! The fixture is what `nsgif_data_scan` reads before the image data of
//! each frame, for an animated GIF of 1000 frames: a graphic control
//! extension, a comment, the image descriptor and a 256-entry local colour
//! table, then the LZW sub-blocks, which the bench skips the same way on
//! both sides.

use core::hint::black_box;

use criterion::Criterion;
use obs_c_oracle::gif_blocks as c;
use obs_graphics::ffi::gif_blocks::{
    nsgif_blocks_colour_table, nsgif_blocks_extensions, nsgif_blocks_image_descriptor,
};
use obs_graphics::gif_blocks::{ColourLayout, FrameExt, Image, MAX_COLOURS};

use crate::pair;

const FRAMES: usize = 1000;

fn gif() -> Vec<u8> {
    let mut data = Vec::new();
    for f in 0..FRAMES {
        data.extend_from_slice(&[0x21, 0xF9, 0x04, 0x09, 0x04, 0x00, (f % 256) as u8, 0x00]);
        data.extend_from_slice(&[0x21, 0xFE, 0x05, b'f', b'r', b'a', b'm', b'e', 0x00]);
        data.extend_from_slice(&[0x2C, 0, 0, 0, 0, 64, 0, 64, 0, 0x87]);
        data.extend((0..768usize).map(|i| (i * 13 + f) as u8));
        data.push(8);
        for _ in 0..4 {
            data.push(255);
            data.extend(core::iter::repeat_n(0x55, 255));
        }
        data.push(0);
    }
    data.push(0x3B);
    data
}

/// The LZW sub-blocks after the minimum code size.
fn skip_image_data(data: &[u8], mut pos: usize) -> usize {
    pos += 1;
    while let Some(&size) = data.get(pos) {
        pos += usize::from(size) + 1;
        if size == 0 {
            break;
        }
    }
    pos
}

pub fn benchmarks(criterion: &mut Criterion) {
    let data = gif();
    let layout = ColourLayout {
        r: 0,
        g: 1,
        b: 2,
        a: 3,
    };
    let mut rust_table = [0u32; MAX_COLOURS];
    let mut c_table = [0u32; MAX_COLOURS];
    pair(
        criterion,
        "gif_blocks/scan_1000_frames",
        || {
            let (mut pos, mut sum) = (0usize, 0u64);
            for f in 0..FRAMES {
                let mut ext = FrameExt::default();
                let mut loop_max = 1;
                let mut image = Image::default();
                let (mut w, mut h, mut used) = (1u32, 1u32, 0usize);
                // SAFETY: valid pointers into live buffers.
                unsafe {
                    nsgif_blocks_extensions(
                        data.as_ptr(),
                        data.len(),
                        &mut pos,
                        true,
                        &mut ext,
                        &mut loop_max,
                    );
                    nsgif_blocks_image_descriptor(
                        data.as_ptr(),
                        data.len(),
                        &mut pos,
                        true,
                        f == 0,
                        &mut image,
                        &mut w,
                        &mut h,
                    );
                    nsgif_blocks_colour_table(
                        &mut rust_table,
                        &layout,
                        256,
                        data.as_ptr().add(pos),
                        data.len() - pos,
                        &mut used,
                        true,
                    );
                }
                pos = skip_image_data(&data, pos + used);
                sum += u64::from(ext.transparency_index)
                    + u64::from(image.rect.x1)
                    + u64::from(rust_table[255]);
            }
            (pos, black_box(sum))
        },
        || {
            let (mut pos, mut sum) = (0usize, 0u64);
            for f in 0..FRAMES {
                let mut ext = FrameExt::default();
                let mut loop_max = 1;
                let mut image = Image::default();
                let (mut w, mut h, mut used) = (1u32, 1u32, 0usize);
                // SAFETY: as above, through the C signatures.
                unsafe {
                    c::oracle_nsgif_blocks_extensions(
                        data.as_ptr(),
                        data.len(),
                        &mut pos,
                        true,
                        (&raw mut ext).cast(),
                        &mut loop_max,
                    );
                    c::oracle_nsgif_blocks_image_descriptor(
                        data.as_ptr(),
                        data.len(),
                        &mut pos,
                        true,
                        f == 0,
                        (&raw mut image).cast(),
                        &mut w,
                        &mut h,
                    );
                    c::oracle_nsgif_blocks_colour_table(
                        c_table.as_mut_ptr(),
                        (&raw const layout).cast(),
                        256,
                        data.as_ptr().add(pos),
                        data.len() - pos,
                        &mut used,
                        true,
                    );
                }
                pos = skip_image_data(&data, pos + used);
                sum += u64::from(ext.transparency_index)
                    + u64::from(image.rect.x1)
                    + u64::from(c_table[255]);
            }
            (pos, black_box(sum))
        },
    );
}
