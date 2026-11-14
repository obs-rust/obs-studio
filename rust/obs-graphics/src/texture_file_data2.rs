//! `gs_create_texture_file_data2` from `libobs/graphics/texture-file-data2.c.h`.
//!
//! The legacy loader is `gs_create_texture_file_data3` without its colour
//! space output: it forwards the file, the alpha mode and the format and
//! size outputs unchanged, gives data3 a private colour-space slot of its
//! own, discards whatever data3 writes there, and returns data3's result
//! (an owned `bmalloc` block, or null) to the caller unchanged. The decode
//! itself stays in C (`graphics-ffmpeg.c.h`); the safe core only fixes the
//! delegation.
//!
//! The C leaves its slot uninitialized. Data3 writes it only on success and
//! nothing reads it afterwards, so its starting value is not observable; the
//! Rust slot starts at [`ColorSpace::default`].
//!
//! No intentional differences.

use crate::texture_render::ColorSpace;

/// Runs `data3` exactly once with a private colour-space slot and returns
/// its result unchanged. What `data3` writes to the slot is discarded.
///
/// `FnOnce` makes "exactly once" part of the type: the closure carries the
/// caller's file, alpha mode and output pointers, and the core cannot call
/// it twice or keep it.
pub fn create_texture_file_data2<R>(data3: impl FnOnce(&mut ColorSpace) -> R) -> R {
    let _ = data3;
    todo!("gs_create_texture_file_data2 (#4827)")
}
