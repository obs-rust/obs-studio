//! `oracle_*` builds of `libobs/obs-source-deinterlace.c.h`
//! (oracle/source_deinterlace.c): the deinterlace of an async source,
//! renamed. They read the fields and take `async_mutex` directly, as the C
//! does; the Rust shims reach the same fields and lock through the
//! deinterlace seam, compiled into the test binaries by
//! oracle/source_deinterlace_links.c. Both read the oracle's core (its
//! frame interval, video time and effect slots, set by
//! [`oracle_deint_reset`]). The C's other calls are the recording
//! stand-ins `oracle_deint_*`, which the parity test hands the Rust shims as
//! well: the frame cache, the async texture upload, the deinterlace texture
//! size, the effect load (an empty slot gets a fake effect unless the load
//! fails), the graphics context, the SDR white level and every graphics
//! call, on fake textures, texrenders, effects and parameters. Each logs one
//! [`Event`], with whether the watched source's `async_mutex` was held and
//! the graphics depth.
//!
//! The stand-ins keep process-wide state: a test holds [`lock`] around
//! every use, from [`oracle_deint_reset`] to the last read of the log.

use core::ffi::{c_char, c_int, c_long, c_void};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The one lock on the stand-ins' state.
pub fn lock() -> MutexGuard<'static, ()> {
    static STATE: Mutex<()> = Mutex::new(());
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The kinds of [`Event`], in the C's order.
pub mod kind {
    use core::ffi::c_int;

    pub const REMOVE: c_int = 1;
    pub const TEXTURE_SIZE: c_int = 2;
    pub const UPDATE_TEXTURES: c_int = 3;
    pub const RELEASE: c_int = 4;
    pub const DEINTERLACE_SIZE: c_int = 5;
    pub const LOAD_EFFECT: c_int = 6;
    pub const ENTER: c_int = 7;
    pub const LEAVE: c_int = 8;
    pub const WHITE_LEVEL: c_int = 9;
    pub const PARAM: c_int = 10;
    pub const TEXRENDER_TEXTURE: c_int = 11;
    pub const LINEAR_SRGB: c_int = 12;
    pub const COLOR_SPACE: c_int = 13;
    pub const FRAMEBUFFER_SRGB: c_int = 14;
    pub const ENABLE_FRAMEBUFFER_SRGB: c_int = 15;
    pub const SET_TEXTURE_SRGB: c_int = 16;
    pub const SET_TEXTURE: c_int = 17;
    pub const SET_FLOAT: c_int = 18;
    pub const SET_INT: c_int = 19;
    pub const SET_VEC2: c_int = 20;
    pub const SET_BOOL: c_int = 21;
    pub const EFFECT_LOOP: c_int = 22;
    pub const DRAW_SPRITE: c_int = 23;
    pub const TEXTURE_DESTROY: c_int = 24;
    pub const TEXRENDER_DESTROY: c_int = 25;
}

/// How many fake handles of each kind there are.
pub const HANDLES: usize = 32;
/// How many fake parameters there are, one per name the render looks up.
pub const PARAMS: usize = 6;

/// `struct oracle_deint_event`: one call, with up to two pointers, a name,
/// three ints, two floats (as their bits), whether the watched source's
/// `async_mutex` was held (-1 when none is watched) and the graphics depth.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Event {
    pub kind: c_int,
    pub p: [*const c_void; 2],
    pub name: *const c_char,
    pub i: [c_int; 3],
    pub f: [u32; 2],
    pub held: c_int,
    pub graphics: c_int,
}

/// `struct oracle_deint_answers`: what the stand-ins answer.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Answers {
    pub texture_size_ok: bool,
    pub load_ok: bool,
    pub params_found: bool,
    pub texrender_texture: bool,
    pub linear_srgb: bool,
    pub framebuffer_srgb: bool,
    pub color_space: c_int,
    pub loops: c_int,
    pub white_level: f32,
}

/// `struct oracle_deint_state`: the fields of a test source the
/// deinterlace reads or writes, besides the queue.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct State {
    pub unbuffered: bool,
    pub last_sys_timestamp: u64,
    pub last_frame_ts: u64,
    pub offset: u64,
    pub frame_ts: u64,
    pub half_duration: u32,
    pub cur: *mut c_void,
    pub prev: *mut c_void,
    pub format: c_int,
    pub trc: c_int,
    pub width: u32,
    pub height: u32,
    pub flip: bool,
    pub textures: [*mut c_void; 8],
    pub prev_textures: [*mut c_void; 8],
    pub texrender: *mut c_void,
    pub prev_texrender: *mut c_void,
    pub rendered: bool,
    pub effect: *mut c_void,
    pub mode: c_int,
    pub top_first: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            unbuffered: false,
            last_sys_timestamp: 0,
            last_frame_ts: 0,
            offset: 0,
            frame_ts: 0,
            half_duration: 0,
            cur: core::ptr::null_mut(),
            prev: core::ptr::null_mut(),
            format: 0,
            trc: 0,
            width: 0,
            height: 0,
            flip: false,
            textures: [core::ptr::null_mut(); 8],
            prev_textures: [core::ptr::null_mut(); 8],
            texrender: core::ptr::null_mut(),
            prev_texrender: core::ptr::null_mut(),
            rendered: false,
            effect: core::ptr::null_mut(),
            mode: 0,
            top_first: false,
        }
    }
}

unsafe extern "C" {
    /// `deinterlace_process_last_frame`.
    pub fn oracle_deinterlace_process_last_frame(source: *mut c_void, sys_time: u64);
    /// `deinterlace_update_async_video`.
    pub fn oracle_deinterlace_update_async_video(source: *mut c_void);
    /// `deinterlace_render`.
    pub fn oracle_deinterlace_render(source: *mut c_void);
    /// `obs_source_set_deinterlace_mode`.
    pub fn oracle_obs_source_set_deinterlace_mode(source: *mut c_void, mode: c_int);
    /// `obs_source_set_deinterlace_field_order`.
    pub fn oracle_obs_source_set_deinterlace_field_order(source: *mut c_void, field_order: c_int);

    /// Watches `watched`, sets what the stand-ins answer and the core's
    /// frame interval and video time, empties its effect slots, and
    /// forgets the log.
    pub fn oracle_deint_reset(
        watched: *mut c_void,
        answers: *const Answers,
        interval: u64,
        video_time: u64,
    );
    /// How many events the stand-ins logged.
    pub fn oracle_deint_logged() -> usize;
    /// Event `i`, or NULL past the end.
    pub fn oracle_deint_event(i: usize) -> *const Event;
    /// The core's effect slot `idx` (0 discard to 7 yadif 2x).
    pub fn oracle_deint_core_effect(idx: usize) -> *mut c_void;
    /// Fake texture `n` (never NULL).
    pub fn oracle_deint_texture(n: usize) -> *mut c_void;
    /// Fake texrender `n` (never NULL).
    pub fn oracle_deint_texrender(n: usize) -> *mut c_void;
    /// Fake effect `n` (never NULL).
    pub fn oracle_deint_effect(n: usize) -> *mut c_void;
    /// Fake parameter `n` (never NULL).
    pub fn oracle_deint_param(n: usize) -> *mut c_void;

    /// A zeroed test source with a default `async_mutex` and an empty
    /// queue.
    pub fn oracle_deint_source_new() -> *mut c_void;
    /// Frees a test source (not its frames).
    pub fn oracle_deint_source_free(source: *mut c_void);
    /// Queues `frame` at the back.
    pub fn oracle_deint_push(source: *mut c_void, frame: *mut c_void);
    /// The queue's length.
    pub fn oracle_deint_queued(source: *const c_void) -> usize;
    /// Queued frame `idx`, or NULL.
    pub fn oracle_deint_queue(source: *const c_void, idx: usize) -> *mut c_void;
    /// Writes the fields.
    pub fn oracle_deint_set_state(source: *mut c_void, state: *const State);
    /// Reads the fields.
    pub fn oracle_deint_get_state(source: *const c_void, state: *mut State);
    /// A test frame with one reference.
    pub fn oracle_deint_frame_new(timestamp: u64, prev_frame: bool) -> *mut c_void;
    /// Frees a test frame.
    pub fn oracle_deint_frame_free(frame: *mut c_void);
    /// A frame's `refs`.
    pub fn oracle_deint_frame_refs(frame: *const c_void) -> c_long;
    /// Locks the source's `async_mutex`, as `async_tick()` does.
    pub fn oracle_deint_lock(source: *mut c_void);
    /// Unlocks it.
    pub fn oracle_deint_unlock(source: *mut c_void);

    /// The stand-ins, under the names the C oracle calls them by.
    pub fn oracle_deint_remove_async_frame(source: *mut c_void, frame: *mut c_void);
    pub fn oracle_deint_set_async_texture_size(source: *mut c_void, frame: *const c_void) -> bool;
    pub fn oracle_deint_update_async_textures(
        source: *mut c_void,
        frame: *const c_void,
        textures: *mut *mut c_void,
        texrender: *mut c_void,
    ) -> bool;
    pub fn oracle_deint_release_frame(source: *mut c_void, frame: *mut c_void);
    pub fn oracle_deint_set_deinterlace_texture_size(source: *mut c_void);
    pub fn oracle_deint_load_effect(effect: *mut *mut c_void, file: *const c_char) -> *mut c_void;
    pub fn oracle_deint_enter_graphics();
    pub fn oracle_deint_leave_graphics();
    pub fn oracle_deint_sdr_white_level() -> f32;
    pub fn oracle_deint_get_param_by_name(
        effect: *const c_void,
        name: *const c_char,
    ) -> *mut c_void;
    pub fn oracle_deint_texrender_get_texture(texrender: *const c_void) -> *mut c_void;
    pub fn oracle_deint_get_linear_srgb() -> bool;
    pub fn oracle_deint_get_color_space() -> c_int;
    pub fn oracle_deint_framebuffer_srgb_enabled() -> bool;
    pub fn oracle_deint_enable_framebuffer_srgb(enable: bool);
    pub fn oracle_deint_set_texture_srgb(param: *mut c_void, texture: *mut c_void);
    pub fn oracle_deint_set_texture(param: *mut c_void, texture: *mut c_void);
    pub fn oracle_deint_set_float(param: *mut c_void, value: f32);
    pub fn oracle_deint_set_int(param: *mut c_void, value: c_int);
    pub fn oracle_deint_set_vec2(param: *mut c_void, value: *const [f32; 2]);
    pub fn oracle_deint_set_bool(param: *mut c_void, value: bool);
    pub fn oracle_deint_effect_loop(effect: *mut c_void, name: *const c_char) -> bool;
    pub fn oracle_deint_draw_sprite(texture: *mut c_void, flip: u32, width: u32, height: u32);
    pub fn oracle_deint_texture_destroy(texture: *mut c_void);
    pub fn oracle_deint_texrender_destroy(texrender: *mut c_void);

    /// `sizeof`, `_Alignof` and `offsetof` of `struct vec2`.
    pub fn oracle_deint_vec2_size() -> usize;
    pub fn oracle_deint_vec2_align() -> usize;
    pub fn oracle_deint_vec2_offset_x() -> usize;
    pub fn oracle_deint_vec2_offset_y() -> usize;
    /// The same of `struct obs_source_deinterlace_fields`.
    pub fn oracle_deint_fields_size() -> usize;
    pub fn oracle_deint_fields_align() -> usize;
    pub fn oracle_deint_fields_offset_mode() -> usize;
    pub fn oracle_deint_fields_offset_top_first() -> usize;
    /// `sizeof` of `struct obs_source`'s `deinterlace_mode` and
    /// `deinterlace_top_first`, the fields it points at.
    pub fn oracle_deint_mode_size() -> usize;
    pub fn oracle_deint_top_first_size() -> usize;
}
