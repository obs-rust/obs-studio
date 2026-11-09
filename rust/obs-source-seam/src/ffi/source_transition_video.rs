//! C ABI for `libobs/obs-source-transition-video.c.h`, the video render of
//! a transition: `obs_transition_video_render()`,
//! `obs_transition_video_render2()`, `obs_transition_video_get_color_space()`
//! and `obs_transition_video_render_direct()`, all `EXPORT` through `obs.h`
//! (`rust-exports.d`).
//!
//! The functions are the safe core's (`obs_source::transition_video`), run
//! on [`Seam`], which implements its `Transition` over the transition seam
//! (`libobs/obs-source-transition-seam.h`, still C in `libobs.core-source`):
//! `transition_valid()`, one accessor per field (the slots, both
//! transitioning flags, the matrices and the texrenders), the lock, unlock
//! and trylock of `transition_mutex` and `transition_tex_mutex`, and the
//! transition's `context.data`, which the type's draw callback is handed
//! (read when the callback runs, as the C reads it). `struct obs_source` is
//! never mirrored. The calls into the rest of libobs go through [`Calls`]:
//! the clock read, stop and stop handler (the stop port's Rust,
//! [`super::source_transition_stop`]), the size (the size port's Rust,
//! [`super::source_transition_size`]), `obs_source_active()`,
//! `obs_source_get_ref()`, `obs_source_release()`, the signal through
//! `obs_source_transition_seam_dosignal()`, `obs_source_get_color_space()`,
//! `obs_source_video_render()`, the core's transparent texture through
//! `obs_source_transition_seam_get_transparent_texture()`, and the graphics
//! calls (`gs_texrender_*`, `gs_get_format_from_space()` (a header inline,
//! which [`Libobs`] runs as its Rust port),
//! `gs_get_color_space()`, `gs_clear()`, `gs_ortho()`, `gs_matrix_*` and
//! `gs_blend_*`). [`Libobs`] calls them, and the parity test hands in the
//! oracle's recording stand-ins, as the C oracle calls them. The draw
//! callback is called through the pointer the caller passed.
//!
//! [`Seam`] holds only the `obs_source_t *` it was handed, so the type's
//! draw callback and the child renders, which re-enter libobs, find nothing
//! of this call borrowed or cached. The locks are the C's: the clock read,
//! the stop and the state copy run under `transition_mutex`, which is let go
//! before the texture trylock; the texture lock, when it was taken, is let
//! go only after the draw and its callback. The video time takes the video
//! stats mutex under `transition_mutex`, with nothing in between.
//!
//! Intentional differences from C: `obs_transition_video_get_color_space()`
//! asks `transition_valid()` first, so no transition and a source that is
//! not one get `GS_CS_SRGB` (the C crashes on the first and mixes the
//! second's unused slots); `obs_transition_video_render_direct()` hands
//! `transition_valid()` its own name (the C hands it
//! `"obs_transition_video_render"`).

use std::ffi::{CStr, c_char, c_int, c_void};
use std::marker::PhantomData;
use std::ptr;

use obs_source::transition_video::{self as core, Matrix, Transition};

use super::{source_transition_size, source_transition_stop};

/// `obs_transition_video_render_callback_t`: the type's draw callback, on
/// its context data, both textures, the time and the size.
pub type RenderCallback = unsafe extern "C" fn(
    data: *mut c_void,
    a: *mut c_void,
    b: *mut c_void,
    t: f32,
    cx: u32,
    cy: u32,
);

/// `struct matrix4`: four `struct vec4` rows, each 16-byte aligned.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Matrix4(pub [f32; 16]);

/// `struct vec4`.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec4(pub [f32; 4]);

unsafe extern "C" {
    fn transition_valid(transition: *const c_void, func: *const c_char) -> bool;

    fn obs_source_transition_seam_get_sources(transition: *const c_void, idx: usize)
    -> *mut c_void;
    fn obs_source_transition_seam_get_transitioning_video(transition: *const c_void) -> bool;
    fn obs_source_transition_seam_set_transitioning_video(transition: *mut c_void, value: bool);
    fn obs_source_transition_seam_get_transitioning_audio(transition: *const c_void) -> bool;
    fn obs_source_transition_seam_set_transitioning_audio(transition: *mut c_void, value: bool);
    fn obs_source_transition_seam_get_matrix(
        transition: *const c_void,
        idx: usize,
        out: *mut Matrix4,
    );
    fn obs_source_transition_seam_get_texrender(
        transition: *const c_void,
        idx: usize,
    ) -> *mut c_void;
    fn obs_source_transition_seam_set_texrender(
        transition: *mut c_void,
        idx: usize,
        value: *mut c_void,
    );
    fn obs_source_transition_seam_lock(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_unlock(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_trylock_textures(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_unlock_textures(transition: *mut c_void) -> c_int;
    fn obs_source_transition_seam_get_context_data(transition: *const c_void) -> *mut c_void;
}

/// The calls the video render makes into the rest of libobs, one C (or
/// ported) function each.
///
/// # Safety
/// Each is called with the pointers the C passes: live sources (a child or
/// NULL where the C allows NULL), texrenders the transition holds (or
/// NULL), and the graphics context entered, as the render callers have it.
pub unsafe trait Calls {
    /// `transition_get_video_time(transition)`.
    ///
    /// # Safety
    /// `transition` is a live transition, its `transition_mutex` held.
    unsafe fn video_time(transition: *mut c_void) -> f32;
    /// `transition_stop(transition)`.
    ///
    /// # Safety
    /// As for [`Calls::video_time`].
    unsafe fn stop(transition: *mut c_void);
    /// `transition_handle_stop(transition)`.
    ///
    /// # Safety
    /// `transition` is a live transition.
    unsafe fn handle_stop(transition: *mut c_void);
    /// `transition_get_cx(transition)`.
    ///
    /// # Safety
    /// `transition` is a live transition.
    unsafe fn cx(transition: *mut c_void) -> u32;
    /// `transition_get_cy(transition)`.
    ///
    /// # Safety
    /// `transition` is a live transition.
    unsafe fn cy(transition: *mut c_void) -> u32;
    /// `obs_source_active(source)`.
    ///
    /// # Safety
    /// `source` is a live `obs_source_t`.
    unsafe fn active(source: *mut c_void) -> bool;
    /// `obs_source_get_ref(source)`.
    ///
    /// # Safety
    /// `source` is NULL or a live `obs_source_t`.
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void;
    /// `obs_source_release(source)`.
    ///
    /// # Safety
    /// `source` is NULL or a live `obs_source_t` the caller holds a
    /// reference on.
    unsafe fn release(source: *mut c_void);
    /// `obs_source_transition_seam_dosignal(transition, signal_obs,
    /// signal_source)`.
    ///
    /// # Safety
    /// `transition` is a live `obs_source_t`, the names NUL-terminated.
    unsafe fn dosignal(
        transition: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    );
    /// `obs_source_get_color_space(source, count, preferred_spaces)`.
    ///
    /// # Safety
    /// `source` is a live `obs_source_t`, `preferred_spaces` `count`
    /// readable spaces.
    unsafe fn color_space(
        source: *mut c_void,
        count: usize,
        preferred_spaces: *const c_int,
    ) -> c_int;
    /// `obs_source_video_render(source)`.
    ///
    /// # Safety
    /// `source` is a live `obs_source_t`; the graphics context is entered.
    unsafe fn video_render(source: *mut c_void);
    /// `obs_source_transition_seam_get_transparent_texture()`:
    /// `obs->video.transparent_texture`.
    ///
    /// # Safety
    /// libobs is started.
    unsafe fn transparent_texture() -> *mut c_void;

    /// `gs_texrender_get_texture(texrender)`.
    ///
    /// # Safety
    /// `texrender` is NULL or a live `gs_texrender_t`.
    unsafe fn texrender_get_texture(texrender: *mut c_void) -> *mut c_void;
    /// `gs_texrender_get_format(texrender)`.
    ///
    /// # Safety
    /// As for [`Calls::texrender_get_texture`].
    unsafe fn texrender_get_format(texrender: *mut c_void) -> c_int;
    /// `gs_texrender_destroy(texrender)`.
    ///
    /// # Safety
    /// As for [`Calls::texrender_get_texture`]; the caller drops its
    /// pointer.
    unsafe fn texrender_destroy(texrender: *mut c_void);
    /// `gs_texrender_create(format, zsformat)`.
    ///
    /// # Safety
    /// The graphics context is entered.
    unsafe fn texrender_create(format: c_int, zsformat: c_int) -> *mut c_void;
    /// `gs_texrender_begin_with_color_space(texrender, cx, cy, space)`.
    ///
    /// # Safety
    /// As for [`Calls::texrender_get_texture`].
    unsafe fn texrender_begin(texrender: *mut c_void, cx: u32, cy: u32, space: c_int) -> bool;
    /// `gs_texrender_end(texrender)`.
    ///
    /// # Safety
    /// As for [`Calls::texrender_get_texture`].
    unsafe fn texrender_end(texrender: *mut c_void);
    /// `gs_get_format_from_space(space)`, a `static inline` of
    /// `graphics.h` ([`Libobs`] runs its port,
    /// `obs_source::transition_video::format_from_space()`).
    ///
    /// # Safety
    /// None beyond the call.
    unsafe fn format_from_space(space: c_int) -> c_int;
    /// `gs_get_color_space()`.
    ///
    /// # Safety
    /// The graphics context is entered.
    unsafe fn current_color_space() -> c_int;
    /// `gs_clear(clear_flags, color, depth, stencil)`.
    ///
    /// # Safety
    /// `color` is a readable `struct vec4`; the graphics context is
    /// entered.
    unsafe fn clear(clear_flags: u32, color: *const Vec4, depth: f32, stencil: u8);
    /// `gs_ortho(left, right, top, bottom, znear, zfar)`.
    ///
    /// # Safety
    /// The graphics context is entered.
    unsafe fn ortho(left: f32, right: f32, top: f32, bottom: f32, znear: f32, zfar: f32);
    /// `gs_matrix_push()`.
    ///
    /// # Safety
    /// The graphics context is entered.
    unsafe fn matrix_push();
    /// `gs_matrix_mul(matrix)`.
    ///
    /// # Safety
    /// `matrix` is a readable `struct matrix4`; the graphics context is
    /// entered.
    unsafe fn matrix_mul(matrix: *const Matrix4);
    /// `gs_matrix_pop()`.
    ///
    /// # Safety
    /// The graphics context is entered.
    unsafe fn matrix_pop();
    /// `gs_blend_state_push()`.
    ///
    /// # Safety
    /// The graphics context is entered.
    unsafe fn blend_state_push();
    /// `gs_blend_function(src, dest)`.
    ///
    /// # Safety
    /// The graphics context is entered.
    unsafe fn blend_function(src: c_int, dest: c_int);
    /// `gs_blend_state_pop()`.
    ///
    /// # Safety
    /// The graphics context is entered.
    unsafe fn blend_state_pop();
}

/// [`Calls`] through libobs's own functions, and the stop and size ports'
/// Rust.
pub enum Libobs {}

unsafe extern "C" {
    fn obs_source_active(source: *const c_void) -> bool;
    fn obs_source_get_ref(source: *mut c_void) -> *mut c_void;
    fn obs_source_release(source: *mut c_void);
    fn obs_source_transition_seam_dosignal(
        transition: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    );
    fn obs_source_get_color_space(
        source: *mut c_void,
        count: usize,
        preferred_spaces: *const c_int,
    ) -> c_int;
    fn obs_source_video_render(source: *mut c_void);
    fn obs_source_transition_seam_get_transparent_texture() -> *mut c_void;

    fn gs_texrender_get_texture(texrender: *const c_void) -> *mut c_void;
    fn gs_texrender_get_format(texrender: *const c_void) -> c_int;
    fn gs_texrender_destroy(texrender: *mut c_void);
    fn gs_texrender_create(format: c_int, zsformat: c_int) -> *mut c_void;
    fn gs_texrender_begin_with_color_space(
        texrender: *mut c_void,
        cx: u32,
        cy: u32,
        space: c_int,
    ) -> bool;
    fn gs_texrender_end(texrender: *mut c_void);
    fn gs_get_color_space() -> c_int;
    fn gs_clear(clear_flags: u32, color: *const Vec4, depth: f32, stencil: u8);
    fn gs_ortho(left: f32, right: f32, top: f32, bottom: f32, znear: f32, zfar: f32);
    fn gs_matrix_push();
    fn gs_matrix_mul(matrix: *const Matrix4);
    fn gs_matrix_pop();
    fn gs_blend_state_push();
    fn gs_blend_function(src: c_int, dest: c_int);
    fn gs_blend_state_pop();
}

// SAFETY: each method is the one libobs function of its name, or the
// ported helper of the C's call, called with the caller's pointers.
unsafe impl Calls for Libobs {
    unsafe fn video_time(transition: *mut c_void) -> f32 {
        // SAFETY: see the trait.
        unsafe {
            source_transition_stop::get_video_time::<source_transition_stop::Libobs>(transition)
        }
    }
    unsafe fn stop(transition: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { source_transition_stop::stop::<source_transition_stop::Libobs>(transition) }
    }
    unsafe fn handle_stop(transition: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { source_transition_stop::handle_stop::<source_transition_stop::Libobs>(transition) }
    }
    unsafe fn cx(transition: *mut c_void) -> u32 {
        // SAFETY: see the trait.
        unsafe { source_transition_size::get_cx::<source_transition_size::Libobs>(transition) }
    }
    unsafe fn cy(transition: *mut c_void) -> u32 {
        // SAFETY: see the trait.
        unsafe { source_transition_size::get_cy::<source_transition_size::Libobs>(transition) }
    }
    unsafe fn active(source: *mut c_void) -> bool {
        // SAFETY: see the trait.
        unsafe { obs_source_active(source) }
    }
    unsafe fn get_ref(source: *mut c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { obs_source_get_ref(source) }
    }
    unsafe fn release(source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { obs_source_release(source) }
    }
    unsafe fn dosignal(
        transition: *mut c_void,
        signal_obs: *const c_char,
        signal_source: *const c_char,
    ) {
        // SAFETY: see the trait.
        unsafe { obs_source_transition_seam_dosignal(transition, signal_obs, signal_source) }
    }
    unsafe fn color_space(
        source: *mut c_void,
        count: usize,
        preferred_spaces: *const c_int,
    ) -> c_int {
        // SAFETY: see the trait.
        unsafe { obs_source_get_color_space(source, count, preferred_spaces) }
    }
    unsafe fn video_render(source: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { obs_source_video_render(source) }
    }
    unsafe fn transparent_texture() -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { obs_source_transition_seam_get_transparent_texture() }
    }
    unsafe fn texrender_get_texture(texrender: *mut c_void) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { gs_texrender_get_texture(texrender) }
    }
    unsafe fn texrender_get_format(texrender: *mut c_void) -> c_int {
        // SAFETY: see the trait.
        unsafe { gs_texrender_get_format(texrender) }
    }
    unsafe fn texrender_destroy(texrender: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { gs_texrender_destroy(texrender) }
    }
    unsafe fn texrender_create(format: c_int, zsformat: c_int) -> *mut c_void {
        // SAFETY: see the trait.
        unsafe { gs_texrender_create(format, zsformat) }
    }
    unsafe fn texrender_begin(texrender: *mut c_void, cx: u32, cy: u32, space: c_int) -> bool {
        // SAFETY: see the trait.
        unsafe { gs_texrender_begin_with_color_space(texrender, cx, cy, space) }
    }
    unsafe fn texrender_end(texrender: *mut c_void) {
        // SAFETY: see the trait.
        unsafe { gs_texrender_end(texrender) }
    }
    unsafe fn format_from_space(space: c_int) -> c_int {
        // The header inline, ported.
        core::format_from_space(space)
    }
    unsafe fn current_color_space() -> c_int {
        // SAFETY: see the trait.
        unsafe { gs_get_color_space() }
    }
    unsafe fn clear(clear_flags: u32, color: *const Vec4, depth: f32, stencil: u8) {
        // SAFETY: see the trait.
        unsafe { gs_clear(clear_flags, color, depth, stencil) }
    }
    unsafe fn ortho(left: f32, right: f32, top: f32, bottom: f32, znear: f32, zfar: f32) {
        // SAFETY: see the trait.
        unsafe { gs_ortho(left, right, top, bottom, znear, zfar) }
    }
    unsafe fn matrix_push() {
        // SAFETY: see the trait.
        unsafe { gs_matrix_push() }
    }
    unsafe fn matrix_mul(matrix: *const Matrix4) {
        // SAFETY: see the trait.
        unsafe { gs_matrix_mul(matrix) }
    }
    unsafe fn matrix_pop() {
        // SAFETY: see the trait.
        unsafe { gs_matrix_pop() }
    }
    unsafe fn blend_state_push() {
        // SAFETY: see the trait.
        unsafe { gs_blend_state_push() }
    }
    unsafe fn blend_function(src: c_int, dest: c_int) {
        // SAFETY: see the trait.
        unsafe { gs_blend_function(src, dest) }
    }
    unsafe fn blend_state_pop() {
        // SAFETY: see the trait.
        unsafe { gs_blend_state_pop() }
    }
}

/// A pointer libobs handed out (a source, texrender or texture), never
/// dereferenced here, only passed back and compared; never NULL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle(*mut c_void);

fn handle(ptr: *mut c_void) -> Option<Handle> {
    (!ptr.is_null()).then_some(Handle(ptr))
}

fn handle_ptr(handle: Option<Handle>) -> *mut c_void {
    handle.map_or(ptr::null_mut(), |h| h.0)
}

/// A live `obs_source_t` a shim was handed, with the libobs calls `L` makes
/// for it.
pub struct Seam<L>(*mut c_void, PhantomData<L>);

impl<L> Seam<L> {
    fn new(source: *mut c_void) -> Self {
        Self(source, PhantomData)
    }
}

// SAFETY (every method): `self.0` is the live source the C caller passed,
// each child one the seam read from its slot and referenced, each
// texrender one the seam read from the transition, and each call is the
// one seam accessor, lock call or libobs function that names it, under the
// caller's graphics context. `idx` is a slot, 0 or 1.
impl<L: Calls> Transition for Seam<L> {
    type Child = Handle;
    type Texrender = Handle;
    type Texture = Handle;
    type Callback = RenderCallback;

    fn valid(source: Option<&Self>, func: &'static CStr) -> bool {
        let ptr = source.map_or(ptr::null(), |s| s.0.cast_const());
        // SAFETY: NULL or the caller's live source, and a static name.
        unsafe { transition_valid(ptr, func.as_ptr()) }
    }
    fn source(&self, idx: usize) -> Option<Handle> {
        // SAFETY: see the impl.
        handle(unsafe { obs_source_transition_seam_get_sources(self.0, idx) })
    }
    fn transitioning_video(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_transitioning_video(self.0) }
    }
    fn set_transitioning_video(&self, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_transitioning_video(self.0, value) }
    }
    fn transitioning_audio(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_get_transitioning_audio(self.0) }
    }
    fn set_transitioning_audio(&self, value: bool) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_transitioning_audio(self.0, value) }
    }
    fn matrix(&self, idx: usize) -> Matrix {
        let mut out = Matrix4::default();
        // SAFETY: see the impl; `out` is a writable `struct matrix4`.
        unsafe { obs_source_transition_seam_get_matrix(self.0, idx, &raw mut out) };
        out.0
    }
    fn texrender(&self, idx: usize) -> Option<Handle> {
        // SAFETY: see the impl.
        handle(unsafe { obs_source_transition_seam_get_texrender(self.0, idx) })
    }
    fn set_texrender(&self, idx: usize, value: Option<Handle>) {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_set_texrender(self.0, idx, handle_ptr(value)) }
    }
    fn lock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_lock(self.0) };
    }
    fn unlock(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_unlock(self.0) };
    }
    fn trylock_textures(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { obs_source_transition_seam_trylock_textures(self.0) == 0 }
    }
    fn unlock_textures(&self) {
        // SAFETY: see the impl. The C ignores the result too.
        unsafe { obs_source_transition_seam_unlock_textures(self.0) };
    }
    fn video_time(&self) -> f32 {
        // SAFETY: see the impl; the caller holds `transition_mutex`.
        unsafe { L::video_time(self.0) }
    }
    fn stop(&self) {
        // SAFETY: see the impl; the caller holds `transition_mutex`.
        unsafe { L::stop(self.0) }
    }
    fn handle_stop(&self) {
        // SAFETY: see the impl.
        unsafe { L::handle_stop(self.0) }
    }
    fn cx(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { L::cx(self.0) }
    }
    fn cy(&self) -> u32 {
        // SAFETY: see the impl.
        unsafe { L::cy(self.0) }
    }
    fn active(&self) -> bool {
        // SAFETY: see the impl.
        unsafe { L::active(self.0) }
    }
    fn get_ref(&self, child: Option<Handle>) -> Option<Handle> {
        // SAFETY: see the impl; NULL as the C passes it.
        handle(unsafe { L::get_ref(handle_ptr(child)) })
    }
    fn release(&self, child: Option<Handle>) {
        // SAFETY: see the impl; NULL as the C passes it.
        unsafe { L::release(handle_ptr(child)) }
    }
    fn dosignal(&self, signal_obs: &'static CStr, signal_source: &'static CStr) {
        // SAFETY: see the impl; static names.
        unsafe { L::dosignal(self.0, signal_obs.as_ptr(), signal_source.as_ptr()) }
    }
    fn color_space(&self, preferred: &[i32]) -> i32 {
        // SAFETY: see the impl; `preferred` is the slice's spaces.
        unsafe { L::color_space(self.0, preferred.len(), preferred.as_ptr()) }
    }
    fn child_color_space(&self, child: Handle, preferred: &[i32]) -> i32 {
        // SAFETY: see the impl; `preferred` is the slice's spaces.
        unsafe { L::color_space(child.0, preferred.len(), preferred.as_ptr()) }
    }
    fn video_render(&self, child: Handle) {
        // SAFETY: see the impl.
        unsafe { L::video_render(child.0) }
    }
    fn call_render(
        &self,
        callback: RenderCallback,
        a: Option<Handle>,
        b: Option<Handle>,
        t: f32,
        cx: u32,
        cy: u32,
    ) {
        // SAFETY: see the impl; the context data is read now, as the C
        // reads it, and the callback is the caller's, on the arguments the
        // C hands it.
        unsafe {
            let data = obs_source_transition_seam_get_context_data(self.0);
            callback(data, handle_ptr(a), handle_ptr(b), t, cx, cy);
        }
    }
    fn transparent_texture() -> Option<Handle> {
        // SAFETY: libobs is up when a render runs.
        handle(unsafe { L::transparent_texture() })
    }
    fn texrender_texture(&self, texrender: Option<Handle>) -> Option<Handle> {
        // SAFETY: see the impl.
        handle(unsafe { L::texrender_get_texture(handle_ptr(texrender)) })
    }
    fn texrender_format(&self, texrender: Option<Handle>) -> i32 {
        // SAFETY: see the impl.
        unsafe { L::texrender_get_format(handle_ptr(texrender)) }
    }
    fn texrender_destroy(&self, texrender: Option<Handle>) {
        // SAFETY: see the impl; the core replaces the field next.
        unsafe { L::texrender_destroy(handle_ptr(texrender)) }
    }
    fn texrender_create(&self, format: i32, zsformat: i32) -> Option<Handle> {
        // SAFETY: see the impl.
        handle(unsafe { L::texrender_create(format, zsformat) })
    }
    fn texrender_begin(&self, texrender: Option<Handle>, cx: u32, cy: u32, space: i32) -> bool {
        // SAFETY: see the impl.
        unsafe { L::texrender_begin(handle_ptr(texrender), cx, cy, space) }
    }
    fn texrender_end(&self, texrender: Option<Handle>) {
        // SAFETY: see the impl.
        unsafe { L::texrender_end(handle_ptr(texrender)) }
    }
    fn format_from_space(&self, space: i32) -> i32 {
        // SAFETY: see the impl.
        unsafe { L::format_from_space(space) }
    }
    fn current_color_space(&self) -> i32 {
        // SAFETY: see the impl.
        unsafe { L::current_color_space() }
    }
    fn clear(&self, flags: u32, color: [f32; 4], depth: f32, stencil: u8) {
        let color = Vec4(color);
        // SAFETY: see the impl; `color` is a readable `struct vec4`.
        unsafe { L::clear(flags, &raw const color, depth, stencil) }
    }
    fn ortho(&self, left: f32, right: f32, top: f32, bottom: f32, znear: f32, zfar: f32) {
        // SAFETY: see the impl.
        unsafe { L::ortho(left, right, top, bottom, znear, zfar) }
    }
    fn matrix_push(&self) {
        // SAFETY: see the impl.
        unsafe { L::matrix_push() }
    }
    fn matrix_mul(&self, matrix: &Matrix) {
        let matrix = Matrix4(*matrix);
        // SAFETY: see the impl; `matrix` is a readable `struct matrix4`.
        unsafe { L::matrix_mul(&raw const matrix) }
    }
    fn matrix_pop(&self) {
        // SAFETY: see the impl.
        unsafe { L::matrix_pop() }
    }
    fn blend_state_push(&self) {
        // SAFETY: see the impl.
        unsafe { L::blend_state_push() }
    }
    fn blend_function(&self, src: i32, dest: i32) {
        // SAFETY: see the impl.
        unsafe { L::blend_function(src, dest) }
    }
    fn blend_state_pop(&self) {
        // SAFETY: see the impl.
        unsafe { L::blend_state_pop() }
    }
}

fn seam<L>(transition: *mut c_void) -> Option<Seam<L>> {
    (!transition.is_null()).then(|| Seam::new(transition))
}

/// `obs_transition_video_render()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`, whose locks the caller
/// does not hold; `callback`, when set, takes the transition's context
/// data; the graphics context is entered.
pub unsafe fn render<L: Calls>(transition: *mut c_void, callback: Option<RenderCallback>) {
    core::render(seam::<L>(transition).as_ref(), callback);
}

/// `obs_transition_video_render2()` with the libobs calls of `L`.
///
/// # Safety
/// As for [`render`]; `placeholder` is NULL or a live `gs_texture_t`.
pub unsafe fn render2<L: Calls>(
    transition: *mut c_void,
    callback: Option<RenderCallback>,
    placeholder: *mut c_void,
) {
    core::render2(
        seam::<L>(transition).as_ref(),
        callback,
        handle(placeholder),
    );
}

/// `obs_transition_video_get_color_space()` with the libobs calls of `L`.
///
/// # Safety
/// `transition` is NULL or a live `obs_source_t`.
pub unsafe fn get_color_space<L: Calls>(transition: *mut c_void) -> c_int {
    core::get_color_space(seam::<L>(transition).as_ref())
}

/// `obs_transition_video_render_direct()` with the libobs calls of `L`.
///
/// # Safety
/// As for [`render`].
pub unsafe fn render_direct<L: Calls>(transition: *mut c_void, target: c_int) -> bool {
    core::render_direct(seam::<L>(transition).as_ref(), target)
}

/// `void obs_transition_video_render(obs_source_t *transition,
/// obs_transition_video_render_callback_t callback)`.
///
/// # Safety
/// As for [`render`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_video_render(
    transition: *mut c_void,
    callback: Option<RenderCallback>,
) {
    // SAFETY: the caller's contract.
    unsafe { render::<Libobs>(transition, callback) }
}

/// `void obs_transition_video_render2(obs_source_t *transition,
/// obs_transition_video_render_callback_t callback,
/// gs_texture_t *placeholder_texture)`.
///
/// # Safety
/// As for [`render2`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_video_render2(
    transition: *mut c_void,
    callback: Option<RenderCallback>,
    placeholder_texture: *mut c_void,
) {
    // SAFETY: the caller's contract.
    unsafe { render2::<Libobs>(transition, callback, placeholder_texture) }
}

/// `enum gs_color_space obs_transition_video_get_color_space(obs_source_t
/// *transition)`.
///
/// # Safety
/// As for [`get_color_space`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_video_get_color_space(transition: *mut c_void) -> c_int {
    // SAFETY: the caller's contract.
    unsafe { get_color_space::<Libobs>(transition) }
}

/// `bool obs_transition_video_render_direct(obs_source_t *transition,
/// enum obs_transition_target target)`.
///
/// # Safety
/// As for [`render_direct`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_transition_video_render_direct(
    transition: *mut c_void,
    target: c_int,
) -> bool {
    // SAFETY: the caller's contract.
    unsafe { render_direct::<Libobs>(transition, target) }
}
