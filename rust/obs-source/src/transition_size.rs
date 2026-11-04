//! Safe core of `libobs/obs-source-transition-size.c.h`: the size and child
//! matrices of a transition and its per-frame tick, moved out of
//! `libobs/obs-source-transition.c.h`.
//!
//! - [`get_cx`] and [`get_cy`]: `transition_get_cx()` and
//!   `transition_get_cy()`, the size set with `obs_transition_set_size()`,
//!   or the actual size when none is set;
//! - [`recalculate_matrix`] and [`recalculate_matrices`]: the C's file-local
//!   `recalculate_transition_matrix()` and
//!   `transition_recalculate_matrices()`, the matrix each slot's child is
//!   drawn with;
//! - [`recalculate_size`]: `transition_recalculate_size()`, the actual size,
//!   the larger width and the larger height of the two children;
//! - [`tick`]: `obs_transition_tick()`, which recalculates the size and the
//!   matrices, moves the manual value and resets both render targets.
//!
//! Each is generic over a [`Transition`]: one method per field read or
//! write, per lock call and per call into the rest of libobs
//! (`obs_source_get_width()`, `obs_source_get_height()`, `add_alignment()`,
//! the matrix built with `matrix4_identity()`, `matrix4_scale3f()` and
//! `matrix4_translate3f()`, `calc_torquef()` and `gs_texrender_reset()`), so
//! a function makes the C's calls in the C's order and under the C's locks.
//! The rules are the ones ported with the transition clock and render:
//! [`crate::transition_clock::size`], [`crate::transition_clock::fold`],
//! [`crate::transition_clock::fit`], [`crate::transition_clock::manual`] and
//! [`crate::transition_render::offset`].
//!
//! The C shims live in the `obs-source-seam` crate, which implements
//! [`Transition`] over `libobs/obs-source-transition-seam.h` and the libobs
//! calls; only libobs-rust links it, so this crate calls nothing in libobs.
//!
//! The locks are the C's, taken where it takes them: `transition_mutex`
//! around the child reads of the size and of each matrix and around the store
//! of each matrix, and a trylock of `transition_tex_mutex` around the render
//! target resets of the tick. As in the C, the sizes, the scale type, the
//! alignment and the manual fields are read and written outside
//! `transition_mutex`.
//!
//! Intentional difference from C: the two `(int)` casts of the room left
//! for a child saturate when it is past `int`, which only a transition size
//! of about 2^31 or more gives (a saved width of -1 is loaded as
//! 2^32 - 1). The C is undefined there: x86-64 gives `INT_MIN`, arm64
//! saturates, as here.

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

use crate::transition_clock::{Fit, Step, fit, fold, manual};
use crate::transition_render::offset;

/// The two slots, A (0) and B (1).
pub const SLOTS: [usize; 2] = [0, 1];

/// The fields, locks and libobs calls of a transition that its size, matrices
/// and tick use: in libobs, the transition seam
/// (`obs-source-transition-seam.h`) and the libobs functions the C calls,
/// one call per method.
pub trait Transition {
    /// A child in a slot, as the seam hands it back.
    type Child: Copy;
    /// A `gs_texrender_t *` of the two render targets.
    type Texrender: Copy;

    /// `transition_sources[idx]`, `None` for an empty slot.
    fn source(&self, idx: usize) -> Option<Self::Child>;
    /// `transition_cx`, the size set with `obs_transition_set_size()`.
    fn cx(&self) -> u32;
    /// `transition_cy`.
    fn cy(&self) -> u32;
    /// `transition_actual_cx`, the children's size.
    fn actual_cx(&self) -> u32;
    /// Writes `transition_actual_cx`.
    fn set_actual_cx(&self, value: u32);
    /// `transition_actual_cy`.
    fn actual_cy(&self) -> u32;
    /// Writes `transition_actual_cy`.
    fn set_actual_cy(&self, value: u32);
    /// `transition_scale_type`, as its `int`.
    fn scale_type(&self) -> i32;
    /// `transition_alignment`.
    fn alignment(&self) -> u32;
    /// `transition_mode`, as its `int`.
    fn mode(&self) -> i32;
    /// `transition_manual_val`.
    fn manual_val(&self) -> f32;
    /// Writes `transition_manual_val`.
    fn set_manual_val(&self, value: f32);
    /// `transition_manual_target`.
    fn manual_target(&self) -> f32;
    /// `transition_manual_torque`.
    fn manual_torque(&self) -> f32;
    /// `transition_manual_clamp`.
    fn manual_clamp(&self) -> f32;
    /// `transition_texrender[idx]`.
    fn texrender(&self, idx: usize) -> Self::Texrender;

    /// Locks `transition_mutex`.
    fn lock(&self);
    /// Unlocks `transition_mutex`.
    fn unlock(&self);
    /// `pthread_mutex_trylock()` of `transition_tex_mutex`: 0 when it took
    /// the lock, a pthread error code otherwise.
    fn trylock_textures(&self) -> i32;
    /// Unlocks `transition_tex_mutex`.
    fn unlock_textures(&self);

    /// `obs_source_get_width(child)`.
    fn child_width(&self, child: Self::Child) -> u32;
    /// `obs_source_get_height(child)`.
    fn child_height(&self, child: Self::Child) -> u32;
    /// `add_alignment(&pos, align, cx, cy)` on a zeroed `struct vec2 pos`:
    /// the position it leaves, `(x, y)`.
    fn alignment_position(&self, align: u32, cx: i32, cy: i32) -> (f32, f32);
    /// Builds the matrix of slot `idx` as the C does and stores it in
    /// `transition_matrices[idx]`: `matrix4_identity()`, then
    /// `matrix4_scale3f(scale.0, scale.1, 1)`, then
    /// `matrix4_translate3f(pos.0, pos.1, 0)`, then `matrix4_copy()`.
    fn set_matrix(&self, idx: usize, scale: (f32, f32), pos: (f32, f32));
    /// `calc_torquef(val1, val2, torque, min_adjust, t)`.
    fn calc_torquef(&self, val1: f32, val2: f32, torque: f32, min_adjust: f32, t: f32) -> f32;
    /// `gs_texrender_reset(texrender)`.
    fn texrender_reset(&self, texrender: Self::Texrender);
}

/// The C's `(float)` of a `uint32_t`: rounded to the nearest `f32`.
#[expect(clippy::as_conversions, reason = "the C's (float) cast")]
fn to_f32(value: u32) -> f32 {
    value as f32
}

/// The C's `(int)` of a `float`, toward zero. Outside `int` the C is
/// undefined (x86-64 gives `INT_MIN`, arm64 saturates); this saturates on
/// every host (NaN is 0), the module's one intentional difference. A
/// child's room is within `int` for any transition size well below 2^31.
#[expect(clippy::as_conversions, reason = "the C's (int) cast")]
fn to_int(value: f32) -> i32 {
    value as i32
}

/// `transition_get_cx()`: `transition_cx`, or `transition_actual_cx` when
/// it is 0 ([`crate::transition_clock::size`]).
pub fn get_cx<T: Transition>(transition: &T) -> u32 {
    crate::transition_clock::size(transition.cx(), transition.actual_cx())
}

/// `transition_get_cy()`: `transition_cy`, or `transition_actual_cy` when
/// it is 0.
pub fn get_cy<T: Transition>(transition: &T) -> u32 {
    crate::transition_clock::size(transition.cy(), transition.actual_cy())
}

/// `recalculate_transition_matrix()`: the matrix of slot `idx`.
///
/// Reads the transition's size ([`get_cx`], [`get_cy`]) and scale type,
/// then, under the lock, the slot's child and its width and height. Nothing
/// more for an empty slot, or for a child with no width or height
/// ([`crate::transition_clock::Fit::NoMatrix`]). Otherwise the child is
/// scaled by the fit (1 by 1 for a scale type outside the enum), placed by
/// the transition's alignment within the room left
/// ([`crate::transition_render::offset`], each cast to `int` as the C casts
/// it), and the matrix is stored.
pub fn recalculate_matrix<T: Transition>(transition: &T, idx: usize) {
    let tr_cx = to_f32(get_cx(transition));
    let tr_cy = to_f32(get_cy(transition));
    let scale_type = transition.scale_type();

    transition.lock();
    let Some(child) = transition.source(idx) else {
        transition.unlock();
        return;
    };
    let source_cx = to_f32(transition.child_width(child));
    let source_cy = to_f32(transition.child_height(child));
    transition.unlock();

    let scale = match fit(tr_cx, tr_cy, source_cx, source_cy, scale_type) {
        Fit::NoMatrix => return,
        Fit::Scale { x, y } => (x, y),
        // the C's scale stays at its initial 1 by 1
        Fit::Unset => (1.0, 1.0),
    };

    let room = offset(tr_cx, tr_cy, source_cx, source_cy, scale.0, scale.1);
    let pos = transition.alignment_position(transition.alignment(), to_int(room.x), to_int(room.y));

    // The graphics thread and a caller of obs_transition_start() both get here,
    // and the render reads the matrices under this lock (#3570).
    transition.lock();
    transition.set_matrix(idx, scale, pos);
    transition.unlock();
}

/// `transition_recalculate_matrices()`: [`recalculate_matrix`] of slot A,
/// then of slot B.
pub fn recalculate_matrices<T: Transition>(transition: &T) {
    for idx in SLOTS {
        recalculate_matrix(transition, idx);
    }
}

/// `transition_recalculate_size()`: under the lock, folds each non-empty
/// slot's width and height, slot A first
/// ([`crate::transition_clock::fold`], from 0); after the unlock, writes
/// them as `transition_actual_cx` and `transition_actual_cy`.
pub fn recalculate_size<T: Transition>(transition: &T) {
    let mut cx = 0;
    let mut cy = 0;

    transition.lock();
    for idx in SLOTS {
        if let Some(child) = transition.source(idx) {
            let new_cx = transition.child_width(child);
            let new_cy = transition.child_height(child);
            cx = fold(cx, new_cx);
            cy = fold(cy, new_cy);
        }
    }
    transition.unlock();

    transition.set_actual_cx(cx);
    transition.set_actual_cy(cy);
}

/// `obs_transition_tick()`: [`recalculate_size`], [`recalculate_matrices`],
/// then the manual value for the mode and torque
/// ([`crate::transition_clock::manual`]): the target for a snap,
/// `calc_torquef(value, target, torque, clamp, t)` for a step, left as it is
/// otherwise. Last, when the texture trylock takes the lock, resets both
/// render targets (slot A's first) and unlocks it.
pub fn tick<T: Transition>(transition: &T, t: f32) {
    recalculate_size(transition);
    recalculate_matrices(transition);

    match manual(transition.mode(), transition.manual_torque()) {
        Step::Snap => transition.set_manual_val(transition.manual_target()),
        Step::Step => {
            let value = transition.calc_torquef(
                transition.manual_val(),
                transition.manual_target(),
                transition.manual_torque(),
                transition.manual_clamp(),
                t,
            );
            transition.set_manual_val(value);
        }
        Step::Hold => {}
    }

    if transition.trylock_textures() == 0 {
        for idx in SLOTS {
            transition.texrender_reset(transition.texrender(idx));
        }
        transition.unlock_textures();
    }
}
