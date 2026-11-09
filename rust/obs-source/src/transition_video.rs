//! Safe core of `libobs/obs-source-transition-video.c.h`: the video render
//! of a transition, moved out of `libobs/obs-source-transition.c.h`.
//!
//! - [`render`]: `obs_transition_video_render()`, [`render2`] with the
//!   core's transparent texture as the placeholder;
//! - [`render2`]: `obs_transition_video_render2()`, one frame of a
//!   transition: the video stop when it is due, then the draw that
//!   [`crate::transition_render::draw`] picks (both children into their
//!   texrenders and the type's callback blending them, or one child with
//!   its matrix), then the stop's signal and handler;
//! - [`get_color_space`]: `obs_transition_video_get_color_space()`, the
//!   space the children mix to ([`crate::transition_clock::mix_spaces`]);
//! - [`render_direct`]: `obs_transition_video_render_direct()`, one child
//!   with its matrix, picked by [`crate::transition_render::direct_index`],
//!   after the same video stop (which also ends the audio of a transition
//!   that is not active).
//!
//! Each is generic over a [`Transition`]: one method per field read or
//! write, per lock call and per call into the rest of libobs (the clock
//! read, stop and size helpers, the source calls, the graphics calls and
//! the type's draw callback), so a function makes the C's calls in the C's
//! order and under the C's locks. The helpers `get_texture()`,
//! `copy_transition_state()`, `render_child()` and `mix_spaces()` are folded
//! in, with their reads kept where the C makes them.
//!
//! The C shims live in the `obs-source-seam` crate, which implements
//! [`Transition`] over `libobs/obs-source-transition-seam.h` and the libobs
//! calls; only libobs-rust links it, so this crate calls nothing in libobs.
//!
//! The locks are the C's: the clock is read, the stop made and the state
//! copied under `transition_mutex`, which is let go before the texture
//! trylock and the draw; the texture lock, when it was taken, is let go
//! after the draw (the callback runs under it, with no transition lock
//! held). The video time takes the video stats mutex under
//! `transition_mutex`, with nothing in between. The state copy holds a
//! reference on each child for the draw, released after it; the clock
//! fields the C's `struct transition_state` leaves unset are not copied.
//!
//! Intentional differences from C:
//! - [`get_color_space`] asks `transition_valid()` first, as every other
//!   public render call does. The C asks nothing: a NULL transition crashes,
//!   and a source that is not a transition returns a space mixed from its
//!   unused slots. Here both get [`CS_SRGB`], and NULL is logged as the
//!   other calls log it.
//! - [`render_direct`] hands `transition_valid()` its own name. The C hands
//!   it `"obs_transition_video_render"`, so a NULL logged a warning naming
//!   the wrong function.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::todo
)]

use std::ffi::CStr;

use crate::transition_clock;
use crate::transition_render::{self, Draw, VideoStep};

pub use crate::transition_clock::{CS_709_EXTENDED, CS_709_SCRGB, CS_SRGB, CS_SRGB_16F};

/// The name [`render`] and [`render2`] hand `transition_valid()`, which
/// logs it for a NULL source.
pub const RENDER_LOG_NAME: &CStr = c"obs_transition_video_render";

/// The name [`render_direct`] hands `transition_valid()` (the C hands it
/// [`RENDER_LOG_NAME`]).
pub const RENDER_DIRECT_LOG_NAME: &CStr = c"obs_transition_video_render_direct";

/// The name [`get_color_space`] hands `transition_valid()` (the C asks
/// nothing).
pub const GET_COLOR_SPACE_LOG_NAME: &CStr = c"obs_transition_video_get_color_space";

/// The signals of a video stop: the core handler's, then the transition's.
pub const VIDEO_STOP_SIGNALS: (&CStr, &CStr) =
    (c"source_transition_video_stop", c"transition_video_stop");

/// The spaces [`get_color_space`] offers each child, in order.
pub const PREFERRED_SPACES: [i32; 3] = [CS_SRGB, CS_SRGB_16F, CS_709_EXTENDED];

/// `GS_CLEAR_COLOR`, the clear of a child's texrender.
pub const CLEAR_COLOR: u32 = 1;

/// `GS_ZS_NONE`, the depth format of a re-created texrender.
pub const ZS_NONE: i32 = 0;

/// `GS_BLEND_ONE` and `GS_BLEND_INVSRCALPHA`, the blend around the callback.
pub const BLEND_ONE: i32 = 1;
/// See [`BLEND_ONE`].
pub const BLEND_INVSRCALPHA: i32 = 5;

/// The near and far planes of a child's ortho projection.
pub const ORTHO_Z: (f32, f32) = (-100.0, 100.0);

/// `GS_RGBA` and `GS_RGBA16F` (`enum gs_color_format`), the formats
/// [`format_from_space`] gives.
pub const FORMAT_RGBA: i32 = 3;
/// See [`FORMAT_RGBA`].
pub const FORMAT_RGBA16F: i32 = 9;

/// `gs_get_format_from_space()`, a `static inline` of `graphics.h` that
/// libobs exports no symbol for: [`FORMAT_RGBA16F`] for `GS_CS_SRGB_16F`,
/// `GS_CS_709_EXTENDED` and `GS_CS_709_SCRGB`, [`FORMAT_RGBA`] for
/// `GS_CS_SRGB` and any other value.
pub fn format_from_space(space: i32) -> i32 {
    match space {
        CS_SRGB_16F | CS_709_EXTENDED | CS_709_SCRGB => FORMAT_RGBA16F,
        _ => FORMAT_RGBA,
    }
}

/// A `struct matrix4`, as its sixteen floats (row `x`, `y`, `z`, then `t`).
pub type Matrix = [f32; 16];

/// The fields, locks and libobs calls of a transition that its video
/// render uses: in libobs, the transition seam
/// (`obs-source-transition-seam.h`) and the libobs functions, one call per
/// method. Every method takes `&self`, as the C writes through its
/// `obs_source_t *`; none caches a field.
pub trait Transition {
    /// A child source, as a slot holds it.
    type Child: Copy + PartialEq;
    /// A `gs_texrender_t *` that is not NULL.
    type Texrender: Copy + PartialEq;
    /// A `gs_texture_t *` that is not NULL.
    type Texture: Copy + PartialEq;
    /// The type's draw callback (`obs_transition_video_render_callback_t`).
    type Callback: Copy;

    /// `transition_valid(source, func)`: false for no source (the C logs
    /// `func` at debug level) and for one that is not a transition.
    fn valid(source: Option<&Self>, func: &'static CStr) -> bool;

    /// `transition_sources[idx]`, `None` for an empty slot.
    fn source(&self, idx: usize) -> Option<Self::Child>;
    /// `transitioning_video`.
    fn transitioning_video(&self) -> bool;
    /// Writes `transitioning_video`.
    fn set_transitioning_video(&self, value: bool);
    /// `transitioning_audio`.
    fn transitioning_audio(&self) -> bool;
    /// Writes `transitioning_audio`.
    fn set_transitioning_audio(&self, value: bool);
    /// `transition_matrices[idx]`.
    fn matrix(&self, idx: usize) -> Matrix;
    /// `transition_texrender[idx]`.
    fn texrender(&self, idx: usize) -> Option<Self::Texrender>;
    /// Writes `transition_texrender[idx]`.
    fn set_texrender(&self, idx: usize, value: Option<Self::Texrender>);

    /// Locks `transition_mutex`.
    fn lock(&self);
    /// Unlocks `transition_mutex`.
    fn unlock(&self);
    /// Tries `transition_tex_mutex`: whether it was taken.
    fn trylock_textures(&self) -> bool;
    /// Unlocks `transition_tex_mutex`.
    fn unlock_textures(&self);

    /// `transition_get_video_time(self)`.
    fn video_time(&self) -> f32;
    /// `transition_stop(self)`.
    fn stop(&self);
    /// `transition_handle_stop(self)`.
    fn handle_stop(&self);
    /// `transition_get_cx(self)`.
    fn cx(&self) -> u32;
    /// `transition_get_cy(self)`.
    fn cy(&self) -> u32;

    /// `obs_source_active(self)`.
    fn active(&self) -> bool;
    /// `obs_source_get_ref(child)`, which gives `None` for `None`.
    fn get_ref(&self, child: Option<Self::Child>) -> Option<Self::Child>;
    /// `obs_source_release(child)`, which does nothing for `None`.
    fn release(&self, child: Option<Self::Child>);
    /// `obs_source_dosignal(self, signal_obs, signal_source)`.
    fn dosignal(&self, signal_obs: &'static CStr, signal_source: &'static CStr);
    /// `obs_source_get_color_space(self, count, preferred)`.
    fn color_space(&self, preferred: &[i32]) -> i32;
    /// `obs_source_get_color_space(child, count, preferred)`.
    fn child_color_space(&self, child: Self::Child, preferred: &[i32]) -> i32;
    /// `obs_source_video_render(child)`.
    fn video_render(&self, child: Self::Child);
    /// The type's draw callback on the transition's `context.data`, read
    /// now: `callback(data, a, b, t, cx, cy)`.
    fn call_render(
        &self,
        callback: Self::Callback,
        a: Option<Self::Texture>,
        b: Option<Self::Texture>,
        t: f32,
        cx: u32,
        cy: u32,
    );
    /// `obs->video.transparent_texture`.
    fn transparent_texture() -> Option<Self::Texture>;

    /// `gs_texrender_get_texture(texrender)`.
    fn texrender_texture(&self, texrender: Option<Self::Texrender>) -> Option<Self::Texture>;
    /// `gs_texrender_get_format(texrender)`.
    fn texrender_format(&self, texrender: Option<Self::Texrender>) -> i32;
    /// `gs_texrender_destroy(texrender)`.
    fn texrender_destroy(&self, texrender: Option<Self::Texrender>);
    /// `gs_texrender_create(format, zsformat)`.
    fn texrender_create(&self, format: i32, zsformat: i32) -> Option<Self::Texrender>;
    /// `gs_texrender_begin_with_color_space(texrender, cx, cy, space)`.
    fn texrender_begin(
        &self,
        texrender: Option<Self::Texrender>,
        cx: u32,
        cy: u32,
        space: i32,
    ) -> bool;
    /// `gs_texrender_end(texrender)`.
    fn texrender_end(&self, texrender: Option<Self::Texrender>);
    /// `gs_get_format_from_space(space)`.
    fn format_from_space(&self, space: i32) -> i32;
    /// `gs_get_color_space()`.
    fn current_color_space(&self) -> i32;
    /// `gs_clear(flags, &color, depth, stencil)`.
    fn clear(&self, flags: u32, color: [f32; 4], depth: f32, stencil: u8);
    /// `gs_ortho(left, right, top, bottom, znear, zfar)`.
    fn ortho(&self, left: f32, right: f32, top: f32, bottom: f32, znear: f32, zfar: f32);
    /// `gs_matrix_push()`.
    fn matrix_push(&self);
    /// `gs_matrix_mul(matrix)`.
    fn matrix_mul(&self, matrix: &Matrix);
    /// `gs_matrix_pop()`.
    fn matrix_pop(&self);
    /// `gs_blend_state_push()`.
    fn blend_state_push(&self);
    /// `gs_blend_function(src, dest)`.
    fn blend_function(&self, src: i32, dest: i32);
    /// `gs_blend_state_pop()`.
    fn blend_state_pop(&self);
}

/// `copy_transition_state()`: both children, referenced (slot A first,
/// each read then referenced), then both flags. The clock fields the C's
/// `struct transition_state` leaves unset are not copied.
fn copy_state<T: Transition>(transition: &T) -> ([Option<T::Child>; 2], bool, bool) {
    let a = transition.get_ref(transition.source(0));
    let b = transition.get_ref(transition.source(1));
    let video = transition.transitioning_video();
    let audio = transition.transitioning_audio();
    ([a, b], video, audio)
}

#[expect(clippy::as_conversions, reason = "the C's (float) cast")]
fn to_float(value: u32) -> f32 {
    value as f32
}

/// `render_child()`: `child` into slot `idx`'s texrender, re-created first
/// when its format is not `space`'s. The size is read before anything else,
/// and the texrender field again for each call that takes it.
fn render_child<T: Transition>(
    transition: &T,
    child: T::Child,
    idx: usize,
    matrix: &Matrix,
    space: i32,
) {
    let cx = transition.cx();
    let cy = transition.cy();

    let format = transition.format_from_space(space);
    if transition.texrender_format(transition.texrender(idx)) != format {
        transition.texrender_destroy(transition.texrender(idx));
        transition.set_texrender(idx, transition.texrender_create(format, ZS_NONE));
    }

    if transition.texrender_begin(transition.texrender(idx), cx, cy, space) {
        transition.clear(CLEAR_COLOR, [0.0; 4], 0.0, 0);
        transition.ortho(0.0, to_float(cx), 0.0, to_float(cy), ORTHO_Z.0, ORTHO_Z.1);

        transition.matrix_push();
        transition.matrix_mul(matrix);
        transition.video_render(child);
        transition.matrix_pop();

        transition.texrender_end(transition.texrender(idx));
    }
}

/// One child with its matrix, when there is one.
fn draw_child<T: Transition>(transition: &T, child: Option<T::Child>, matrix: &Matrix) {
    if let Some(child) = child {
        transition.matrix_push();
        transition.matrix_mul(matrix);
        transition.video_render(child);
        transition.matrix_pop();
    }
}

/// `obs_transition_video_render()`: [`render2`] with the core's transparent
/// texture (read first, as the C's argument) as the placeholder.
pub fn render<T: Transition>(transition: Option<&T>, callback: Option<T::Callback>) {
    render2(transition, callback, T::transparent_texture());
}

/// `obs_transition_video_render2()`: one frame of the transition.
///
/// Under `transition_mutex`: the video time, then, when the video stop is
/// due ([`crate::transition_render::video_due`]), `transitioning_video`
/// ends and the transition stops unless its audio runs on; then the
/// children (referenced), both flags and both matrices are copied. After
/// the lock: the texture trylock while the video transitions, and the draw
/// [`crate::transition_render::draw`] picks. The callback draw renders each
/// child into its texrender (re-created when its format no longer fits the
/// transition's space), hands the callback both textures (`placeholder` for
/// an empty slot or a texrender with no texture) under a blend of
/// [`BLEND_ONE`] and [`BLEND_INVSRCALPHA`], and only when the size is not
/// zero; the other draws render one child with its matrix. Then the
/// texture lock is let go, the children released, and the video stop
/// signalled and handled.
pub fn render2<T: Transition>(
    transition: Option<&T>,
    callback: Option<T::Callback>,
    placeholder: Option<T::Texture>,
) {
    if !T::valid(transition, RENDER_LOG_NAME) {
        return;
    }
    let Some(transition) = transition else {
        return;
    };
    let mut stopped = false;
    let mut video_stopped = false;

    // Starts write the clock under this lock with the transitioning flags;
    // reading it before the lock could pair a new start with an old clock.
    transition.lock();

    let t = transition.video_time();

    if transition_render::video_due(t, transition.transitioning_video()) {
        transition.set_transitioning_video(false);
        video_stopped = true;

        // no inactive rule here: active is true
        let step = transition_render::video_stop(transition.transitioning_audio(), true);
        if step != VideoStep::End {
            transition.stop();
            stopped = true;
        }
    }
    let ([a, b], video, audio) = copy_state(transition);
    let matrix_a = transition.matrix(0);
    let matrix_b = transition.matrix(1);

    transition.unlock();

    let locked = video && transition.trylock_textures();

    match transition_render::draw(video, locked, callback.is_some(), audio) {
        Draw::Callback => {
            if let Some(callback) = callback {
                let current_space = transition.current_color_space();
                let source_space = transition.color_space(&[current_space]);
                let texture = |idx: usize, child: Option<T::Child>, matrix: &Matrix| {
                    let child = child?;
                    render_child(transition, child, idx, matrix, source_space);
                    transition.texrender_texture(transition.texrender(idx))
                };
                let tex_a = texture(0, a, &matrix_a).or(placeholder);
                let tex_b = texture(1, b, &matrix_b).or(placeholder);

                let cx = transition.cx();
                let cy = transition.cy();
                if cx != 0 && cy != 0 {
                    transition.blend_state_push();
                    transition.blend_function(BLEND_ONE, BLEND_INVSRCALPHA);

                    transition.call_render(callback, tex_a, tex_b, t, cx, cy);

                    transition.blend_state_pop();
                }
            }
        }
        Draw::B => draw_child(transition, b, &matrix_b),
        Draw::A => draw_child(transition, a, &matrix_a),
    }

    if locked {
        transition.unlock_textures();
    }

    transition.release(a);
    transition.release(b);

    if video_stopped {
        transition.dosignal(VIDEO_STOP_SIGNALS.0, VIDEO_STOP_SIGNALS.1);
    }
    if stopped {
        transition.handle_stop();
    }
}

/// `obs_transition_video_get_color_space()`: [`CS_SRGB`] mixed with each
/// child's space from [`PREFERRED_SPACES`], slot A's first.
///
/// [`CS_SRGB`] for no transition and for a source that is not one (an
/// intentional difference: the C crashes on the first and mixes the
/// second's unused slots).
pub fn get_color_space<T: Transition>(transition: Option<&T>) -> i32 {
    if !T::valid(transition, GET_COLOR_SPACE_LOG_NAME) {
        return CS_SRGB;
    }
    let Some(transition) = transition else {
        return CS_SRGB;
    };
    let a = transition.source(0);
    let b = transition.source(1);

    let mut space = CS_SRGB;
    if let Some(a) = a {
        let child = transition.child_color_space(a, &PREFERRED_SPACES);
        space = transition_clock::mix_spaces(space, child);
    }
    if let Some(b) = b {
        let child = transition.child_color_space(b, &PREFERRED_SPACES);
        space = transition_clock::mix_spaces(space, child);
    }
    space
}

/// `obs_transition_video_render_direct()`: the child `target` names while
/// the transition runs (slot A once it is done), with its matrix, after the
/// video stop that [`render2`] makes, which here also ends the audio of a
/// transition that is not active. Whether the transition still runs (audio
/// or video), from the copied state.
///
/// Logs its own name for no transition (an intentional difference: the C
/// logs `"obs_transition_video_render"`).
pub fn render_direct<T: Transition>(transition: Option<&T>, target: i32) -> bool {
    if !T::valid(transition, RENDER_DIRECT_LOG_NAME) {
        return false;
    }
    let Some(transition) = transition else {
        return false;
    };
    let mut stopped = false;
    let mut video_stopped = false;

    // Starts write the clock under this lock with the transitioning flags;
    // reading it before the lock could pair a new start with an old clock.
    transition.lock();

    let t = transition.video_time();

    if transition_render::video_due(t, transition.transitioning_video()) {
        transition.set_transitioning_video(false);
        video_stopped = true;

        let step =
            transition_render::video_stop(transition.transitioning_audio(), transition.active());
        if step == VideoStep::StopAudio {
            transition.set_transitioning_audio(false);
        }

        if step != VideoStep::End {
            transition.stop();
            stopped = true;
        }
    }
    let ([a, b], video, audio) = copy_state(transition);
    let transitioning = audio || video;
    let matrix_a = transition.matrix(0);
    let matrix_b = transition.matrix(1);

    transition.unlock();

    if transition_render::direct_index(transitioning, target) == 0 {
        draw_child(transition, a, &matrix_a);
    } else {
        draw_child(transition, b, &matrix_b);
    }

    transition.release(a);
    transition.release(b);

    if video_stopped {
        transition.dosignal(VIDEO_STOP_SIGNALS.0, VIDEO_STOP_SIGNALS.1);
    }
    if stopped {
        transition.handle_stop();
    }

    transitioning
}
