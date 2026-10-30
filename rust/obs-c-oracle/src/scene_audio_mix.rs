//! `oracle_*` build of `libobs/obs-scene-audio-mix.c.h`
//! (oracle/scene_audio_mix.c): the scene audio mix kernels, renamed.

unsafe extern "C" {
    /// `out` writable from `pos` for `count` floats, `input` and `buf`
    /// readable for `count`.
    pub fn oracle_obs_scene_audio_mix_with_buf(
        out: *mut f32,
        input: *const f32,
        buf: *const f32,
        pos: usize,
        count: usize,
    );

    /// `out` writable from `pos` for `count` floats, `input` readable for
    /// `count`.
    pub fn oracle_obs_scene_audio_mix_add(
        out: *mut f32,
        input: *const f32,
        pos: usize,
        count: usize,
    );
}
