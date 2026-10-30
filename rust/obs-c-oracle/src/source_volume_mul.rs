//! `oracle_*` build of `libobs/obs-source-volume-mul.c.h`
//! (oracle/source_volume_mul.c): the source volume multiply kernels,
//! renamed.

unsafe extern "C" {
    /// `out` points to `count` writable floats.
    pub fn oracle_obs_source_volume_mul_block(out: *mut f32, count: usize, vol: f32);

    /// `out` points to `count` writable floats, `vol_data` to `count`
    /// readable ones.
    pub fn oracle_obs_source_volume_mul_data(out: *mut f32, vol_data: *const f32, count: usize);
}
