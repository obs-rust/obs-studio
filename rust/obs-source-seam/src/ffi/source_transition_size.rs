//! C ABI for `libobs/obs-source-transition-size.c.h`, the size and child
//! matrices of a transition and its tick: `transition_get_cx()`,
//! `transition_get_cy()`, `transition_recalculate_matrices()`,
//! `transition_recalculate_size()` and `obs_transition_tick()`. None is
//! `EXPORT`: the four helpers are declared in the private
//! `obs-source-transition-size.h` and the tick in `obs-internal.h`, so all
//! five stay hidden (`rust-hidden.d`).
//!
//! The functions are the safe core's (`obs_source::transition_size`), run
//! on [`Seam`], which implements its `Transition` over the transition seam
//! (`libobs/obs-source-transition-seam.h`, still C in `libobs.core-source`):
//! one accessor per field, the matrix write, and the lock, unlock and
//! trylock of `transition_mutex` and `transition_tex_mutex`, one seam call
//! per method. `struct obs_source` is never mirrored. The calls into the
//! rest of libobs that read a source or touch a render target
//! (`obs_source_get_width()`, `obs_source_get_height()` and
//! `gs_texrender_reset()`) go through [`Calls`]: [`Libobs`] calls the C
//! symbols, and the parity test hands in the oracle's recording stand-ins,
//! as the C oracle calls them. The arithmetic the C calls goes to the C
//! symbols directly: `add_alignment()`, `calc_torquef()`, and the matrix,
//! built as the C's inline `matrix4_identity()`, `matrix4_scale3f()` and
//! `matrix4_translate3f()` build it (the identity, then `matrix4_scale()`
//! and `matrix4_translate3v()` with a `struct vec3` whose `w` is 0), so its
//! floats are libobs's.
//!
//! The locks are the C's, taken where it takes them: `transition_mutex`
//! around the child reads of the size and of each matrix and around the
//! manual step of the tick, and a trylock of `transition_tex_mutex` around
//! the render target resets of the tick. As in the C, the actual size is
//! written, and the set size, scale type and alignment read, outside
//! `transition_mutex`. The seam takes no lock of its own.

use std::ffi::{c_int, c_void};
use std::marker::PhantomData;

use obs_source::transition_size::{self as core, Transition};

/// `struct vec2` (`graphics/vec2.h`): `x` and `y`. Its size, alignment and
/// offsets are checked against the C header in the parity tests
/// (`transition_size_layout`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    /// `x`.
    pub x: f32,
    /// `y`.
    pub y: f32,
}

/// `struct vec3` (`graphics/vec3.h`), an `__m128`: `x`, `y`, `z` and the
/// padding `w`.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    /// `x`.
    pub x: f32,
    /// `y`.
    pub y: f32,
    /// `z`.
    pub z: f32,
    /// `w`, 0 as `vec3_set()` leaves it.
    pub w: f32,
}

/// `struct matrix4` (`graphics/matrix4.h`): four `struct vec4` (`__m128`)
/// rows, `x`, `y`, `z` and `t`.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Matrix4 {
    /// `x`.
    pub x: [f32; 4],
    /// `y`.
    pub y: [f32; 4],
    /// `z`.
    pub z: [f32; 4],
    /// `t`.
    pub t: [f32; 4],
}

unsafe extern "C" {
    fn obs_source_transition_seam_get_sources(transition: *const c_void, idx: usize)
    -> *mut c_void;
    fn obs_source_transition_seam_get_cx(transition: *const c_void) -> u32;
    fn obs_source_transition_seam_get_cy(transition: *const c_void) -> u32;
    fn obs_source_transition_seam_get_actual_cx(transition: *const c_void) -> u32;
    fn obs_source_transition_seam_set_actual_cx(transition: *mut c_void, value: u32);
    fn obs_source_transition_seam_get_actual_cy(transition: *const c_void) -> u32;
    fn obs_source_transition_seam_set_actual_cy(transition: *mut c_void, value: u32);
    fn obs_source_transition_seam_get_scale_type(transition: *const c_void) -> c_int;
    fn obs_source_transition_seam_get_alignment(transition: *const c_void) -> u32;
    fn obs_source_transition_seam_get_mode(transition: *const c_void) -> c_int;
    fn obs_source_transition_seam_get_manual_val(transition: *const c_void) -> f32;
    fn obs_source_transition_seam_set_manual_val(transition: *mut c_void, value: f32);
    fn obs_source_transition_seam_get_manual_target(transition: *const c_void) -> f32;
    fn obs_source_transition_seam_get_manual_torque(transition: *const c_void) -> f32;
    fn obs_source_transition_seam_get_manual_clamp(transition: *const c_void) -> f32;
    fn obs_source_transition_seam_get_texrender(
        transition: *const c_void,
        idx: usize,
    ) -> *mut c_void;
    fn obs_source_transition_seam_set_matrix(
        transition: *mut c_void,
        idx: usize,
        value: *const Matrix4,
    );
    fn obs_source_transition_seam_lock(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_unlock(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_trylock_textures(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_unlock_textures(transition: *mut c_void) -> c_int;

    fn add_alignment(v: *mut Vec2, align: u32, cx: c_int, cy: c_int);
    fn calc_torquef(val1: f32, val2: f32, torque: f32, min_adjust: f32, t: f32) -> f32;
    fn matrix4_scale(dst: *mut Matrix4, m: *const Matrix4, v: *const Vec3);
    fn matrix4_translate3v(dst: *mut Matrix4, m: *const Matrix4, v: *const Vec3);
}

/// The calls the sizing makes into the rest of libobs that read a source or
/// touch a render target, one C function each.
///
/// # Safety
/// Each is called with the pointers the C passes: a live child in a slot,
/// and a render target of a live transition.
pub unsafe trait Calls {
    /// `obs_source_get_width(source)`.
    ///
    /// # Safety
    /// `source` is a live `obs_source_t`.
    unsafe fn get_width(source: *mut c_void) -> u32;
    /// `obs_source_get_height(source)`.
    ///
    /// # Safety
    /// `source` is a live `obs_source_t`.
    unsafe fn get_height(source: *mut c_void) -> u32;
    /// `gs_texrender_reset(texrender)`.
    ///
    /// # Safety
    /// `texrender` is NULL or a live `gs_texrender_t`.
    unsafe fn texrender_reset(texrender: *mut c_void);
}

/// [`Calls`] through libobs's own functions.
pub enum Libobs {}

unsafe extern "C" {
    fn obs_source_get_width(source: *mut c_void) -> u32;
    fn obs_source_get_height(source: *mut c_void) -> u32;
    fn gs_texrender_reset(texrender: *mut c_void);
}

// SAFETY: each method is the one libobs function of its name, called with
// the caller's pointers.
unsafe impl Calls for Libobs {
    unsafe fn get_width(source: *mut c_void) -> u32 {
        // SAFETY: see the trait.
        unsafe { obs_source_get_width(source) }
    }
    unsafe fn get_height(source: *mut c_void) -> u32 {
        // SAFETY: see the trait.
        unsafe { obs_source_get_height(source) }
    }
    unsafe fn texrender_reset(texrender: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { gs_texrender_reset(texrender) }
    }
}

/// A non-NULL child of a transition, read from one of its slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChildSource(pub(crate) *mut c_void);

/// A `gs_texrender_t *` of a transition, handed to `gs_texrender_reset()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Texrender(*mut c_void);

/// A live, non-NULL `obs_source_t` a shim was handed, with the libobs calls
/// `L` makes for it.
pub struct Seam<L>(pub(crate) *mut c_void, PhantomData<L>);

impl<L> Seam<L> {
    pub(crate) fn new(source: *mut c_void) -> Self {
        Self(source, PhantomData)
    }
}

/// `matrix4_identity()`: each row zeroed, then `x.x`, `y.y`, `z.z` and
/// `t.w` 1.
fn identity() -> Matrix4 {
    Matrix4 {
        x: [1.0, 0.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0, 0.0],
        z: [0.0, 0.0, 1.0, 0.0],
        t: [0.0, 0.0, 0.0, 1.0],
    }
}

// SAFETY (every method): `self.0` is the live source the C caller passed,
// each child one the seam read from its slot under the lock (live while the
// slot holds its reference), and each call is the one seam accessor, lock
// call or libobs function that names it. `idx` is a slot, 0 or 1.
impl<L: Calls> Transition for Seam<L> {
    type Child = ChildSource;
    type Texrender = Texrender;

    fn source(&self, idx: usize) -> Option<ChildSource> {
        // SAFETY: see the impl.
        let ptr = unsafe { obs_source_transition_seam_get_sources(self.0, idx) };
        (!ptr.is_null()).then_some(ChildSource(ptr))
    }
    fn cx(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_cx(self.0) }
    }
    fn cy(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_cy(self.0) }
    }
    fn actual_cx(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_actual_cx(self.0) }
    }
    fn set_actual_cx(&self, value: u32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_actual_cx(self.0, value) }
    }
    fn actual_cy(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_actual_cy(self.0) }
    }
    fn set_actual_cy(&self, value: u32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_actual_cy(self.0, value) }
    }
    fn scale_type(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_scale_type(self.0) }
    }
    fn alignment(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_alignment(self.0) }
    }
    fn mode(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_mode(self.0) }
    }
    fn manual_val(&self) -> f32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_manual_val(self.0) }
    }
    fn set_manual_val(&self, value: f32) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_manual_val(self.0, value) }
    }
    fn manual_target(&self) -> f32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_manual_target(self.0) }
    }
    fn manual_torque(&self) -> f32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_manual_torque(self.0) }
    }
    fn manual_clamp(&self) -> f32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_manual_clamp(self.0) }
    }
    fn texrender(&self, idx: usize) -> Texrender {
        // SAFETY: see the impl.
        Texrender(unsafe { obs_source_transition_seam_get_texrender(self.0, idx) })
    }
    fn lock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_lock(self.0) };
    }
    fn unlock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_unlock(self.0) };
    }
    fn trylock_textures(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_trylock_textures(self.0) }
    }
    fn unlock_textures(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_unlock_textures(self.0) };
    }
    fn child_width(&self, child: ChildSource) -> u32 {
        // SAFETY: see the impl.
        unsafe { L::get_width(child.0) }
    }
    fn child_height(&self, child: ChildSource) -> u32 {
        // SAFETY: see the impl.
        unsafe { L::get_height(child.0) }
    }
    fn alignment_position(&self, align: u32, cx: i32, cy: i32) -> (f32, f32) {
        let mut pos = Vec2::default();
        // SAFETY: a live vec2 of our own.
        unsafe { add_alignment(&raw mut pos, align, cx, cy) };
        (pos.x, pos.y)
    }
    fn set_matrix(&self, idx: usize, scale: (f32, f32), pos: (f32, f32)) {
        let mut mat = identity();
        let s = Vec3 {
            x: scale.0,
            y: scale.1,
            z: 1.0,
            w: 0.0,
        };
        let t = Vec3 {
            x: pos.0,
            y: pos.1,
            z: 0.0,
            w: 0.0,
        };
        let m: *mut Matrix4 = &raw mut mat;
        // SAFETY: a live, aligned matrix and vec3s of our own (libobs's
        // matrix calls allow dst == m, as the C passes them), then the live
        // source's slot.
        unsafe {
            matrix4_scale(m, m, &raw const s);
            matrix4_translate3v(m, m, &raw const t);
            obs_source_transition_seam_set_matrix(self.0, idx, m);
        }
    }
    fn calc_torquef(&self, val1: f32, val2: f32, torque: f32, min_adjust: f32, t: f32) -> f32 {
        // SAFETY: plain floats.
        unsafe { calc_torquef(val1, val2, torque, min_adjust, t) }
    }
    fn texrender_reset(&self, texrender: Texrender) {
        // SAFETY: see the impl; the render target is the transition's,
        // under its texture lock.
        unsafe { L::texrender_reset(texrender.0) }
    }
}

/// `transition_get_cx()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is a live transition `obs_source_t`.
pub unsafe fn get_cx<L: Calls>(transition: *mut c_void) -> u32 {
    core::get_cx(&Seam::<L>::new(transition))
}

/// `transition_get_cy()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is a live transition `obs_source_t`.
pub unsafe fn get_cy<L: Calls>(transition: *mut c_void) -> u32 {
    core::get_cy(&Seam::<L>::new(transition))
}

/// `transition_recalculate_matrices()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is a live transition `obs_source_t`.
pub unsafe fn recalculate_matrices<L: Calls>(transition: *mut c_void) {
    core::recalculate_matrices(&Seam::<L>::new(transition));
}

/// `transition_recalculate_size()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is a live transition `obs_source_t`.
pub unsafe fn recalculate_size<L: Calls>(transition: *mut c_void) {
    core::recalculate_size(&Seam::<L>::new(transition));
}

/// `obs_transition_tick()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is a live transition `obs_source_t`, on the graphics
/// thread (its render targets are reset).
pub unsafe fn tick<L: Calls>(transition: *mut c_void, t: f32) {
    core::tick(&Seam::<L>::new(transition), t);
}

/// `uint32_t transition_get_cx(obs_source_t *tr)`.
///
/// # Safety
/// `tr` is a live transition `obs_source_t`, as the C requires (it does not
/// check it).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn transition_get_cx(tr: *mut c_void) -> u32 {
    // SAFETY: the caller's contract.
    unsafe { get_cx::<Libobs>(tr) }
}

/// `uint32_t transition_get_cy(obs_source_t *tr)`.
///
/// # Safety
/// `tr` is a live transition `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn transition_get_cy(tr: *mut c_void) -> u32 {
    // SAFETY: the caller's contract.
    unsafe { get_cy::<Libobs>(tr) }
}

/// `void transition_recalculate_matrices(obs_source_t *transition)`.
///
/// # Safety
/// `transition` is a live transition `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn transition_recalculate_matrices(transition: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { recalculate_matrices::<Libobs>(transition) }
}

/// `void transition_recalculate_size(obs_source_t *transition)`.
///
/// # Safety
/// `transition` is a live transition `obs_source_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn transition_recalculate_size(transition: *mut c_void) {
    // SAFETY: the caller's contract.
    unsafe { recalculate_size::<Libobs>(transition) }
}

/// `void obs_transition_tick(obs_source_t *transition, float t)`.
///
/// # Safety
/// `transition` is a live transition `obs_source_t`, on the graphics
/// thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_tick(transition: *mut c_void, t: f32) {
    // SAFETY: the caller's contract.
    unsafe { tick::<Libobs>(transition, t) }
}
