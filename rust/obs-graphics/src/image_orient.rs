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
//! checked: a buffer shorter than its size says is copied as far as it
//! goes.
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
    let _ = (a, b, c, d);
    todo!()
}

/// `ffmpeg_image_orient()`: the image is turned for this orientation (2 to
/// 8).
#[must_use]
pub fn orient_needed(orient: i32) -> bool {
    let _ = orient;
    todo!()
}

/// `ffmpeg_image_orient()`: width and height swap for this orientation (5
/// to 8).
#[must_use]
pub fn orient_swaps(orient: i32) -> bool {
    let _ = orient;
    todo!()
}

/// `ffmpeg_image_orient()`: copies the `cx` x `cy` RGBA image `src` into
/// `dst`, turned for `orient` (as stored for one that [`orient_needed`]
/// says is not turned).
pub fn orient_copy(dst: &mut [u8], src: &[u8], cx: usize, cy: usize, orient: i32) {
    let _ = (dst, src, cx, cy, orient);
    todo!()
}

/// `ffmpeg_image_too_large()`: an image of `cx` x `cy` is larger than
/// `max_size` (the device's maximum texture size; 0 for unknown) in either
/// direction. A size not above 0 is left to the decoder.
#[must_use]
pub fn too_large(max_size: u32, cx: i32, cy: i32) -> bool {
    let _ = (max_size, cx, cy);
    todo!()
}
