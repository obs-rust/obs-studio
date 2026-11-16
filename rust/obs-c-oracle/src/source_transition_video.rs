//! `oracle_*` builds of `libobs/obs-source-transition-video.c.h`
//! (oracle/source_transition_video.c): the video render of a transition,
//! renamed. They run on the slots oracle's graph
//! (`crate::source_transition_slots`), reading the fields and taking both
//! transition mutexes directly, as the C does; the Rust shims reach the same
//! fields and locks through the transition seam. The C's other calls are
//! the recording stand-ins `oracle_video_*`, which the parity test hands
//! the Rust shims as well: the clock read and size (set by
//! [`oracle_video_reset`]), the stop (the stop oracle's
//! `oracle_transition_stop`, `crate::source_transition_stop`), the stop
//! handler, `obs_source_active()`, the reference calls (the slots
//! oracle's), the signal, the colour-space reads (each source's set by
//! [`oracle_video_set_space`]), the child render and every graphics call,
//! on fake texrenders and textures ([`oracle_video_texrender`],
//! [`oracle_video_texture`]) whose format, texture and begin result a test
//! sets. [`oracle_video_callback`] is a recording draw callback. Each logs
//! one [`Event`], with whether the watched transition's `transition_mutex`
//! (bit 0) and `transition_tex_mutex` (bit 1) were held.
//!
//! A test holds `crate::source_transition_slots::graph_lock` around every
//! use, and calls [`oracle_video_reset`] with each graph.

use core::ffi::{c_char, c_int, c_void};

/// The kinds of [`Event`], in the C's order.
pub mod kind {
    use core::ffi::c_int;

    pub const VIDEO_TIME: c_int = 1;
    pub const STOP: c_int = 2;
    pub const HANDLE_STOP: c_int = 3;
    pub const CX: c_int = 4;
    pub const CY: c_int = 5;
    pub const ACTIVE: c_int = 6;
    pub const GET_REF: c_int = 7;
    pub const RELEASE: c_int = 8;
    pub const SIGNAL: c_int = 9;
    pub const COLOR_SPACE: c_int = 10;
    pub const RENDER: c_int = 11;
    pub const TEX_TEXTURE: c_int = 12;
    pub const TEX_FORMAT: c_int = 13;
    pub const TEX_DESTROY: c_int = 14;
    pub const TEX_CREATE: c_int = 15;
    pub const TEX_BEGIN: c_int = 16;
    pub const TEX_END: c_int = 17;
    pub const FORMAT_FROM_SPACE: c_int = 18;
    pub const CURRENT_SPACE: c_int = 19;
    pub const CLEAR: c_int = 20;
    pub const ORTHO: c_int = 21;
    pub const PUSH: c_int = 22;
    pub const MUL: c_int = 23;
    pub const POP: c_int = 24;
    pub const BLEND_PUSH: c_int = 25;
    pub const BLEND_FUNCTION: c_int = 26;
    pub const BLEND_POP: c_int = 27;
    pub const CALLBACK: c_int = 28;
}

/// How many fake texrenders (and textures) there are.
pub const HANDLES: usize = 32;

/// `struct oracle_video_event`: one call, with up to three pointers, two
/// signal names, four ints, sixteen floats (as their bits) and the held
/// mutexes of the watched transition (-1 when none is watched).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Event {
    pub kind: c_int,
    pub p: [*const c_void; 3],
    pub name: [*const c_char; 2],
    pub i: [c_int; 4],
    pub f: [u32; 16],
    pub held: c_int,
}

/// `obs_transition_video_render_callback_t`.
pub type RenderCallback =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, f32, u32, u32);

unsafe extern "C" {
    /// `obs_transition_video_render`.
    pub fn oracle_obs_transition_video_render(
        transition: *mut c_void,
        callback: Option<RenderCallback>,
    );
    /// `obs_transition_video_render2`.
    pub fn oracle_obs_transition_video_render2(
        transition: *mut c_void,
        callback: Option<RenderCallback>,
        placeholder: *mut c_void,
    );
    /// `obs_transition_video_get_color_space`.
    pub fn oracle_obs_transition_video_get_color_space(transition: *mut c_void) -> c_int;
    /// `obs_transition_video_render_direct`.
    pub fn oracle_obs_transition_video_render_direct(
        transition: *mut c_void,
        target: c_int,
    ) -> bool;

    /// Watches `watched` and sets what the stand-ins give; forgets the
    /// spaces, texrender formats and textures, the transparent texture and
    /// the log.
    pub fn oracle_video_reset(
        watched: *mut c_void,
        time: f32,
        cx: u32,
        cy: u32,
        active: bool,
        current_space: c_int,
        begins: bool,
        next_texrender: usize,
    );
    /// The space `obs_source_get_color_space()` gives `source` (else sRGB).
    pub fn oracle_video_set_space(source: *mut c_void, space: c_int);
    /// Fake texrender `n`'s format, and whether its texture is NULL.
    pub fn oracle_video_set_texrender(n: usize, format: c_int, no_texture: bool);
    /// Fake texrender `n` (never NULL).
    pub fn oracle_video_texrender(n: usize) -> *mut c_void;
    /// Fake texture `n` (never NULL), fake texrender `n`'s texture.
    pub fn oracle_video_texture(n: usize) -> *mut c_void;
    /// Sets the core's transparent texture.
    pub fn oracle_video_set_transparent(texture: *mut c_void);
    /// The recording draw callback.
    pub fn oracle_video_callback(
        data: *mut c_void,
        a: *mut c_void,
        b: *mut c_void,
        t: f32,
        cx: u32,
        cy: u32,
    );
    /// How many events the log holds (at most 256).
    pub fn oracle_video_logged() -> usize;
    /// Event `i`, NULL past the end.
    pub fn oracle_video_event(i: usize) -> *const Event;
    /// `gs_get_format_from_space()`, the `graphics.h` inline.
    pub fn oracle_gs_get_format_from_space(space: c_int) -> c_int;

    // The stand-ins, under the names the C oracle calls.
    pub fn oracle_video_transparent_texture() -> *mut c_void;
    pub fn oracle_video_time(transition: *mut c_void) -> f32;
    pub fn oracle_video_stop(transition: *mut c_void);
    pub fn oracle_video_handle_stop(transition: *mut c_void);
    pub fn oracle_video_cx(transition: *mut c_void) -> u32;
    pub fn oracle_video_cy(transition: *mut c_void) -> u32;
    pub fn oracle_video_active(source: *mut c_void) -> bool;
    pub fn oracle_video_get_ref(source: *mut c_void) -> *mut c_void;
    pub fn oracle_video_release(source: *mut c_void);
    pub fn oracle_video_dosignal(
        source: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    );
    pub fn oracle_video_color_space(
        source: *mut c_void,
        count: usize,
        preferred_spaces: *const c_int,
    ) -> c_int;
    pub fn oracle_video_render(source: *mut c_void);
    pub fn oracle_video_texrender_get_texture(texrender: *mut c_void) -> *mut c_void;
    pub fn oracle_video_texrender_get_format(texrender: *mut c_void) -> c_int;
    pub fn oracle_video_texrender_destroy(texrender: *mut c_void);
    pub fn oracle_video_texrender_create(format: c_int, zsformat: c_int) -> *mut c_void;
    pub fn oracle_video_texrender_begin(
        texrender: *mut c_void,
        cx: u32,
        cy: u32,
        space: c_int,
    ) -> bool;
    pub fn oracle_video_texrender_end(texrender: *mut c_void);
    pub fn oracle_video_format_from_space(space: c_int) -> c_int;
    pub fn oracle_video_current_color_space() -> c_int;
    pub fn oracle_video_clear(clear_flags: u32, color: *const [f32; 4], depth: f32, stencil: u8);
    pub fn oracle_video_ortho(left: f32, right: f32, top: f32, bottom: f32, znear: f32, zfar: f32);
    pub fn oracle_video_matrix_push();
    pub fn oracle_video_matrix_mul(matrix: *const [f32; 16]);
    pub fn oracle_video_matrix_pop();
    pub fn oracle_video_blend_state_push();
    pub fn oracle_video_blend_function(src: c_int, dest: c_int);
    pub fn oracle_video_blend_state_pop();
}
