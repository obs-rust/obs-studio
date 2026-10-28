//! Safe core of `libobs/graphics/graphics-ffmpeg-orient.c.h`: the image
//! orientation and size check of `graphics-ffmpeg.c`, which loads every
//! image file through FFmpeg (the Image Source, the Image Slide Show and
//! the other users of `gs_image_file`).
//!
//! - [`matrix_orientation`] (`display_matrix_to_orientation()`, FFmpeg
//!   builds without the EXIF API): the EXIF orientation from the signs of
//!   the frame's display matrix;
//! - [`orient_needed`], [`orient_swaps`] and [`orient_copy`]
//!   (`ffmpeg_image_orient()`): whether the decoded RGBA image is turned,
//!   whether its width and height swap, and the copy that turns it, in
//!   16 x 16 tiles;
//! - [`too_large`] (`ffmpeg_image_too_large()`): whether the image is
//!   larger than the device's maximum texture size, so it is refused before
//!   it is decoded.
//!
//! `graphics-ffmpeg.c` keeps every FFmpeg call, the orientation read, the
//! allocation of the turned image and the free of the decoded one, the size
//! update, and the warning. These keep no state and call nothing.
//!
//! The orientation and the sizes come from the image file, so nothing here
//! can panic on any input, and every pixel the copy writes is bounds
//! checked. A size whose `cx * cy * 4` overflows, or a buffer shorter than
//! that, is not copied at all (the C reads and writes out of bounds there;
//! `graphics-ffmpeg.c` always passes buffers of that size).
//!
//! Intentional difference (#2809, the claim's decision): an orientation
//! outside 0 to 8, which reaches the C through `atoi()` of the file's
//! "Orientation" metadata on FFmpeg builds without the EXIF API, is the
//! image as stored, as an EXIF reader treats an unknown value:
//! [`orient_needed`] is false for it, and [`orient_copy`] copies it
//! unturned. The C turns it and writes every pixel to the first one.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// Bytes of one RGBA pixel.
pub const BYTES_PER_PIXEL: usize = 4;

/// The side of the square tiles [`orient_copy`] walks.
pub const TILE_SIZE: usize = 16;

/// `display_matrix_to_orientation()`: the EXIF orientation (1 to 8) whose
/// display matrix has the signs of `a`, `b`, `c` and `d` (`matrix[0]`,
/// `[1]`, `[3]` and `[4]`), or 0 for none.
#[must_use]
pub fn matrix_orientation(a: i32, b: i32, c: i32, d: i32) -> i32 {
    // The display matrix FFmpeg derives from each EXIF orientation, as the
    // signs of its 2x2 part, in EXIF order.
    const ORIENTATIONS: [[i32; 4]; 8] = [
        [1, 0, 0, 1],
        [-1, 0, 0, 1],
        [-1, 0, 0, -1],
        [1, 0, 0, -1],
        [0, 1, 1, 0],
        [0, 1, -1, 0],
        [0, -1, -1, 0],
        [0, -1, 1, 0],
    ];
    let signs = [a.signum(), b.signum(), c.signum(), d.signum()];
    ORIENTATIONS
        .iter()
        .zip(1..)
        .find(|(m, _)| **m == signs)
        .map_or(0, |(_, orient)| orient)
}

/// `ffmpeg_image_orient()`: the image is turned for this orientation (2 to
/// 8).
#[must_use]
pub fn orient_needed(orient: i32) -> bool {
    (2..=8).contains(&orient)
}

/// `ffmpeg_image_orient()`: width and height swap for this orientation (5
/// to 8).
#[must_use]
pub fn orient_swaps(orient: i32) -> bool {
    (5..=8).contains(&orient)
}

/// `get_dst_position()` as a linear map, in pixels: source pixel (`x`,
/// `y`) of a `cx` x `cy` image lands at `start + x * step_x + y * step_y` in
/// the turned image. The steps are two's complement (`wrapping_*`), so a
/// negative step walks backwards. The `(dx, dy)` of each orientation is the
/// C's.
fn pixel_map(orient: i32, cx: usize, cy: usize) -> (usize, usize, usize) {
    let back = |n: usize| 0_usize.wrapping_sub(n);
    let last_x = cx.wrapping_sub(1);
    let last_y = cy.wrapping_sub(1);
    match orient {
        // (cx - 1 - x, y) in a cx-wide image
        2 => (last_x, back(1), cx),
        // (cx - 1 - x, cy - 1 - y)
        3 => (
            last_x.wrapping_add(last_y.wrapping_mul(cx)),
            back(1),
            back(cx),
        ),
        // (x, cy - 1 - y)
        4 => (last_y.wrapping_mul(cx), 1, back(cx)),
        // (y, x) in a cy-wide image
        5 => (0, cy, 1),
        // (cy - 1 - y, x)
        6 => (last_y, cy, back(1)),
        // (cy - 1 - y, cx - 1 - x)
        7 => (
            last_y.wrapping_add(last_x.wrapping_mul(cy)),
            back(cy),
            back(1),
        ),
        // (y, cx - 1 - x)
        8 => (last_x.wrapping_mul(cy), back(cy), 1),
        // as stored
        _ => (0, 1, cx),
    }
}

/// `ffmpeg_image_orient()`: copies the `cx` x `cy` RGBA image `src` into
/// `dst`, turned for `orient` (as stored for one that [`orient_needed`]
/// says is not turned).
pub fn orient_copy(dst: &mut [u8], src: &[u8], cx: usize, cy: usize, orient: i32) {
    let Some(len) = cx
        .checked_mul(cy)
        .and_then(|n| n.checked_mul(BYTES_PER_PIXEL))
    else {
        return;
    };
    let (Some(dst), Some(src)) = (dst.get_mut(..len), src.get(..len)) else {
        return;
    };
    if !orient_needed(orient) || len == 0 {
        // The intentional difference (module docs): as stored.
        dst.copy_from_slice(src);
        return;
    }
    let (start, step_x, step_y) = pixel_map(orient, cx, cy);
    // Tiles, as the C walks them, so the turned rows stay in the cache.
    // cx * cy * 4 fits in usize (checked above), so no product below wraps
    // for a pixel inside the image.
    for y0 in (0..cy).step_by(TILE_SIZE) {
        let y1 = cy.min(y0.saturating_add(TILE_SIZE));
        for x0 in (0..cx).step_by(TILE_SIZE) {
            let x1 = cx.min(x0.saturating_add(TILE_SIZE));
            for y in y0..y1 {
                let row = y.wrapping_mul(cx);
                let Some(from) = src.get(
                    row.wrapping_add(x0).wrapping_mul(BYTES_PER_PIXEL)
                        ..row.wrapping_add(x1).wrapping_mul(BYTES_PER_PIXEL),
                ) else {
                    continue;
                };
                let mut at = start
                    .wrapping_add(x0.wrapping_mul(step_x))
                    .wrapping_add(y.wrapping_mul(step_y));
                let (pixels, _) = from.as_chunks::<BYTES_PER_PIXEL>();
                for pixel in pixels {
                    let byte = at.wrapping_mul(BYTES_PER_PIXEL);
                    if let Some(to) = dst.get_mut(byte..byte.wrapping_add(BYTES_PER_PIXEL)) {
                        to.copy_from_slice(pixel);
                    }
                    at = at.wrapping_add(step_x);
                }
            }
        }
    }
}

/// `ffmpeg_image_too_large()`: an image of `cx` x `cy` is larger than
/// `max_size` (the device's maximum texture size; 0 for unknown) in either
/// direction. A size not above 0 is left to the decoder.
#[must_use]
pub fn too_large(max_size: u32, cx: i32, cy: i32) -> bool {
    if max_size == 0 || cx <= 0 || cy <= 0 {
        return false;
    }
    cx.unsigned_abs() > max_size || cy.unsigned_abs() > max_size
}
