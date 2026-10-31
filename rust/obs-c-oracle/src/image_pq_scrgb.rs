//! `oracle_*` build of `libobs/graphics/graphics-ffmpeg-host-pq.c.h`
//! (oracle/image_pq_scrgb.c): the JPEG XR PQ-to-scRGB conversion of the
//! image loader's Windows host, renamed, with its static `pq_to_linear()`
//! and `half.h`'s `half_from_float()` reachable through wrappers.

unsafe extern "C" {
    /// Reads `src_size` bytes and writes `2 * src_size`: never call it
    /// with a `dst` shorter than that.
    pub fn oracle_gs_image_pq10_to_scrgb(
        dst: *mut u8,
        dst_size: u32,
        src: *const u8,
        src_size: u32,
    );

    pub fn oracle_pq_to_linear(u: f32) -> f32;

    /// Undefined for a float below 2^-45 in magnitude (biased exponent 81
    /// or less): never call it with one.
    pub fn oracle_half_from_float(f: f32) -> u16;
}
