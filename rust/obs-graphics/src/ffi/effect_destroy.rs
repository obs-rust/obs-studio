//! C ABI for `libobs/graphics/effect.c.h`: `gs_effect_destroy` (`EXPORT`
//! in `graphics.h`) and `gs_effect_actually_destroy` (no `EXPORT`: hidden;
//! `gs_destroy` in `graphics.c` calls it for every cached effect, and
//! `gs_effect_create` reaches it through `gs_effect_destroy` when the parse
//! fails).
//!
//! `CEffect` (private, so only the entry points below build one)
//! implements the core's [`EffectModel`] on the C structs (the
//! mirrors in [`super::effect_lookups`]) and frees each item in place, as
//! the header-inline frees of `graphics/effect.h` do: `bfree` for a name,
//! a path or the effect, `darray_free` (`bfree` of the array, then the
//! array, count and capacity zeroed) for an array, `gs_shader_destroy` for
//! a shader. The two C calls go through [`Calls`]: [`Libobs`] is libobs's
//! (`bfree` through the private `obs_graphics_bfree` adapter, as every
//! obs-graphics shim allocates), which the exported shims use; the parity
//! test passes a recorder.
//!
//! Like the C, the shims take no graphics context of their own
//! (`gs_destroy` enters the device's first) and clear nothing that points
//! at the effect.

use core::ffi::c_void;
use core::ptr;

use obs_util::ffi::darray::darray;

use super::effect_lookups::{gs_effect, gs_effect_param, gs_effect_pass, gs_effect_technique};
use crate::effect_destroy::{EffectModel, Free, effect_actually_destroy, effect_destroy};

unsafe extern "C" {
    /// `bfree`, through the private adapter in `obs-graphics-adapter.c.h`
    /// (an executable that links the staticlib next to libobs must not pull
    /// in the staticlib's own allocator).
    #[link_name = "obs_graphics_bfree"]
    fn bfree(ptr: *mut c_void);
    fn gs_shader_destroy(shader: *mut c_void);
}

/// The two C calls the destroy makes.
pub trait Calls {
    /// `bfree(ptr)`.
    ///
    /// # Safety
    /// `ptr` is null or a live `bmalloc` block nothing uses afterwards.
    unsafe fn bfree(&mut self, ptr: *mut c_void);

    /// `gs_shader_destroy(shader)`.
    ///
    /// # Safety
    /// `shader` is null or a live shader of the current device.
    unsafe fn shader_destroy(&mut self, shader: *mut c_void);
}

/// The libobs symbols.
pub struct Libobs;

impl Calls for Libobs {
    unsafe fn bfree(&mut self, ptr: *mut c_void) {
        // SAFETY: the caller's contract.
        unsafe { bfree(ptr) }
    }

    unsafe fn shader_destroy(&mut self, shader: *mut c_void) {
        // SAFETY: the caller's contract.
        unsafe { gs_shader_destroy(shader) }
    }
}

/// An item of the effect being destroyed (a param or annotation, a
/// technique or a pass): only `CEffect` makes one, from the effect's own
/// arrays, and only it reads one.
struct Item<T>(*mut T);

impl<T> Clone for Item<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Item<T> {}

/// One `struct gs_effect` being destroyed, and the calls that free it.
struct CEffect<'a, C: Calls> {
    effect: *mut gs_effect,
    calls: &'a mut C,
}

impl<'a, C: Calls> CEffect<'a, C> {
    /// # Safety
    ///
    /// `effect` is a valid `struct gs_effect` whose params (with their
    /// annotations), techniques (with their passes), names, paths and
    /// arrays are null or live `bmalloc` blocks and whose shaders are null
    /// or live shaders, as `gs_effect_create` builds it; nothing else uses
    /// it during the call, nor after a destroy that frees it.
    unsafe fn new(effect: *mut gs_effect, calls: &'a mut C) -> Self {
        Self { effect, calls }
    }

    /// `darray_free`.
    ///
    /// # Safety
    /// `array` is a valid `struct darray` of the effect.
    unsafe fn da_free(&mut self, array: *mut darray) {
        // SAFETY: `array` is valid and its block is the effect's to free.
        unsafe {
            self.calls.bfree((*array).array);
            (*array).array = ptr::null_mut();
            (*array).num = 0;
            (*array).capacity = 0;
        }
    }
}

impl<C: Calls> EffectModel for CEffect<'_, C> {
    type Param = Item<gs_effect_param>;
    type Technique = Item<gs_effect_technique>;
    type Pass = Item<gs_effect_pass>;

    fn cached(&self) -> bool {
        // SAFETY: `new`'s contract: the effect is valid.
        unsafe { (*self.effect).cached }
    }

    fn param_count(&self) -> usize {
        // SAFETY: the effect is valid.
        unsafe { (*self.effect).params.num }
    }

    fn param(&self, index: usize) -> Self::Param {
        // SAFETY: the effect is valid; the core asks only for `index < num`.
        Item(unsafe { ((*self.effect).params.array as *mut gs_effect_param).wrapping_add(index) })
    }

    fn annotation_count(&self, param: Self::Param) -> usize {
        // SAFETY: `param` is one of the effect's params or annotations.
        unsafe { (*param.0).annotations.num }
    }

    fn annotation(&self, param: Self::Param, index: usize) -> Self::Param {
        // SAFETY: `param` is valid; the core asks only for `index < num`.
        Item(unsafe { ((*param.0).annotations.array as *mut gs_effect_param).wrapping_add(index) })
    }

    fn technique_count(&self) -> usize {
        // SAFETY: the effect is valid.
        unsafe { (*self.effect).techniques.num }
    }

    fn technique(&self, index: usize) -> Self::Technique {
        // SAFETY: the effect is valid; the core asks only for `index < num`.
        Item(unsafe {
            ((*self.effect).techniques.array as *mut gs_effect_technique).wrapping_add(index)
        })
    }

    fn pass_count(&self, technique: Self::Technique) -> usize {
        // SAFETY: `technique` is one of the effect's techniques.
        unsafe { (*technique.0).passes.num }
    }

    fn pass(&self, technique: Self::Technique, index: usize) -> Self::Pass {
        // SAFETY: `technique` is valid; the core asks only for `index < num`.
        Item(unsafe { ((*technique.0).passes.array as *mut gs_effect_pass).wrapping_add(index) })
    }

    fn free(&mut self, step: Free<Self::Param, Self::Technique, Self::Pass>) {
        let effect = self.effect;
        // SAFETY: every item the core names is one of the effect's, freed
        // once and in the C's order, so no block is used after its free:
        // each param's and technique's own frees come before the array that
        // holds it, and the effect's last.
        unsafe {
            match step {
                Free::ParamName(param) => self.calls.bfree((*param.0).name.cast()),
                Free::ParamCurVal(param) => self.da_free(&raw mut (*param.0).cur_val),
                Free::ParamDefaultVal(param) => self.da_free(&raw mut (*param.0).default_val),
                Free::ParamAnnotations(param) => self.da_free(&raw mut (*param.0).annotations),
                Free::PassName(pass) => self.calls.bfree((*pass.0).name.cast()),
                Free::PassVertParams(pass) => self.da_free(&raw mut (*pass.0).vertshader_params),
                Free::PassPixelParams(pass) => self.da_free(&raw mut (*pass.0).pixelshader_params),
                Free::PassVertShader(pass) => self.calls.shader_destroy((*pass.0).vertshader),
                Free::PassPixelShader(pass) => self.calls.shader_destroy((*pass.0).pixelshader),
                Free::TechniquePasses(technique) => self.da_free(&raw mut (*technique.0).passes),
                Free::TechniqueName(technique) => self.calls.bfree((*technique.0).name.cast()),
                Free::Params => self.da_free(&raw mut (*effect).params),
                Free::Techniques => self.da_free(&raw mut (*effect).techniques),
                Free::EffectPath => self.calls.bfree((*effect).effect_path.cast()),
                Free::EffectDir => self.calls.bfree((*effect).effect_dir.cast()),
                Free::ClearPaths => {
                    (*effect).effect_path = ptr::null_mut();
                    (*effect).effect_dir = ptr::null_mut();
                }
                Free::Effect => self.calls.bfree(effect.cast()),
            }
        }
    }
}

/// `gs_effect_destroy` over `calls`.
///
/// # Safety
/// `effect` is null, or:
/// `effect` is a valid `struct gs_effect` whose params (with their
/// annotations), techniques (with their passes), names, paths and arrays
/// are null or live `bmalloc` blocks and whose shaders are null or live
/// shaders, as `gs_effect_create` builds it; nothing else uses it during
/// the call, nor after a destroy that frees it.
pub unsafe fn destroy_with<C: Calls>(effect: *mut gs_effect, calls: &mut C) {
    if effect.is_null() {
        effect_destroy::<CEffect<'_, C>>(None);
        return;
    }
    // SAFETY: the caller's contract.
    let mut model = unsafe { CEffect::new(effect, calls) };
    effect_destroy(Some(&mut model));
}

/// `gs_effect_actually_destroy` over `calls`.
///
/// # Safety
/// `effect` is a valid `struct gs_effect` whose params (with their
/// annotations), techniques (with their passes), names, paths and arrays
/// are null or live `bmalloc` blocks and whose shaders are null or live
/// shaders, as `gs_effect_create` builds it; nothing else uses it during
/// the call, nor after a destroy that frees it.
pub unsafe fn actually_destroy_with<C: Calls>(effect: *mut gs_effect, calls: &mut C) {
    // SAFETY: the caller's contract.
    let mut model = unsafe { CEffect::new(effect, calls) };
    effect_actually_destroy(&mut model);
}

/// `effect_free` (`graphics/effect.h`) over `calls`: everything the effect
/// holds, not the effect itself.
///
/// # Safety
/// `effect` is a valid `struct gs_effect` whose params (with their
/// annotations), techniques (with their passes), names, paths and arrays
/// are null or live `bmalloc` blocks and whose shaders are null or live
/// shaders, as `gs_effect_create` builds it; nothing else uses it during
/// the call, nor after a destroy that frees it.
pub unsafe fn free_with<C: Calls>(effect: *mut gs_effect, calls: &mut C) {
    // SAFETY: the caller's contract.
    let mut model = unsafe { CEffect::new(effect, calls) };
    crate::effect_destroy::effect_free(&mut model);
}

/// `gs_effect_destroy`: frees `effect` unless it is null or cached.
///
/// # Safety
/// `effect` is null or a live effect from `gs_effect_create`, on the
/// thread whose graphics context made its shaders.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_destroy(effect: *mut gs_effect) {
    // SAFETY: the caller's contract.
    unsafe { destroy_with(effect, &mut Libobs) }
}

/// `gs_effect_actually_destroy`: frees `effect`, cached or not.
///
/// # Safety
/// `effect` is a live effect from `gs_effect_create`, in its device's
/// graphics context, and nothing uses it afterwards (`gs_destroy` frees the
/// cached ones last).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_actually_destroy(effect: *mut gs_effect) {
    // SAFETY: the caller's contract.
    unsafe { actually_destroy_with(effect, &mut Libobs) }
}
