//! `oracle_*` build of `libobs/graphics/effect.c.h`
//! (oracle/effect_destroy.c), and the recorder its `bfree` and
//! `gs_shader_destroy` calls go to. Effects cross as untyped pointers; the
//! caller lays them out (the mirrors of `obs_graphics::ffi::effect_lookups`)
//! on the real bmalloc.

use core::ffi::c_void;

unsafe extern "C" {
    pub fn oracle_gs_effect_destroy(effect: *mut c_void);
    pub fn oracle_gs_effect_actually_destroy(effect: *mut c_void);
    /// `effect_free()` of `graphics/effect.h`, instantiated.
    pub fn oracle_effect_free(effect: *mut c_void);

    /// Records a `bfree`, then frees the block.
    pub fn oracle_destroy_bfree(ptr: *mut c_void);
    /// Records a `gs_shader_destroy`.
    pub fn oracle_destroy_shader_destroy(shader: *mut c_void);
    /// Recording on by default; the bench turns it off.
    pub fn oracle_destroy_set_recording(recording: bool);
    pub fn oracle_destroy_reset();
    pub fn oracle_destroy_count() -> usize;
    /// Events past the log's 4096.
    pub fn oracle_destroy_dropped() -> usize;
    /// The i-th event: `'f'` a `bfree`, `'s'` a shader destroy, 0 past the
    /// end; its pointer in `ptr`.
    pub fn oracle_destroy_event(i: usize, ptr: *mut *const c_void) -> i32;
}
