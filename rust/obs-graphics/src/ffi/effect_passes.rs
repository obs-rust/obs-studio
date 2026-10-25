//! C ABI for `libobs/graphics/effect-passes.c.h`: the 7 exported functions
//! of the technique and pass loop.
//!
//! [`CModel`] implements the core's [`PassModel`] on the C structs (the
//! mirrors in [`super::effect_lookups`], plus `struct pass_shaderparam`)
//! and the C graphics functions. The technique lookup is the Rust
//! `gs_effect_get_technique`; the values are emptied and filled in place
//! with [`super::effect_params::DarrayStore`]. `graphics->cur_effect` is
//! written by `obs_graphics_set_cur_effect`, so `struct graphics_subsystem`
//! does not cross, and the two warnings go to `blog` with the C's literal
//! formats through `obs_graphics_effect_loop_warning`; both are adapters in
//! `obs-graphics-adapter.c.h`, which the executables that link the
//! staticlib next to libobs compile too.
//!
//! Intentional differences, where the C dereferences a NULL pointer and
//! libobs never gets there: a NULL technique name finds no technique (the
//! C passes it to `strcmp`) and a NULL pass name matches no name.

use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr;

use obs_util::ffi::darray::darray;

use super::effect_lookups::{
    gs_effect, gs_effect_get_technique, gs_effect_param, gs_effect_pass, gs_effect_technique,
};
use super::effect_params::DarrayStore;
use crate::effect_params::{ByteStore, GS_SHADER_PARAM_TEXTURE};
use crate::effect_passes::{
    LoopWarning, PassModel, Stage, effect_loop, effect_update_params, technique_begin,
    technique_begin_pass, technique_begin_pass_by_name, technique_end, technique_end_pass,
};

/// `struct pass_shaderparam` (`graphics/effect.h`).
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct pass_shaderparam {
    pub eparam: *mut gs_effect_param,
    pub sparam: *mut c_void,
}

/// `struct gs_shader_param_info` (`graphics/graphics.h`): the type first,
/// unlike `struct gs_effect_param_info`.
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct gs_shader_param_info {
    pub type_: i32,
    pub name: *const c_char,
}

unsafe extern "C" {
    fn gs_get_effect() -> *mut c_void;
    fn gs_load_vertexshader(vertshader: *mut c_void);
    fn gs_load_pixelshader(pixelshader: *mut c_void);
    fn gs_shader_set_val(param: *mut c_void, val: *const c_void, size: usize);
    fn gs_shader_set_next_sampler(param: *mut c_void, sampler: *mut c_void);
    fn gs_shader_get_param_info(param: *const c_void, info: *mut gs_shader_param_info);
    fn gs_shader_set_texture(param: *mut c_void, val: *mut c_void);
    /// `graphics->cur_effect = effect` (`obs-graphics-adapter.c.h`).
    fn obs_graphics_set_cur_effect(graphics: *mut c_void, effect: *mut gs_effect);
    /// `blog(LOG_WARNING, ...)` with the C's literal formats.
    fn obs_graphics_effect_loop_warning(which: c_int, name: *const c_char);
}

/// One effect, its technique name for the lookup, and the C device.
struct CModel {
    effect: *mut gs_effect,
    name: *const c_char,
}

impl CModel {
    /// # Safety
    ///
    /// `effect` is a valid `struct gs_effect` with valid techniques,
    /// passes and parameters, not accessed elsewhere during the call (the
    /// graphics thread); `name` is null or a C string.
    unsafe fn new(effect: *mut gs_effect, name: *const c_char) -> Self {
        Self { effect, name }
    }

    fn e(&self) -> &gs_effect {
        // SAFETY: a valid effect (the constructor's contract).
        unsafe { &*self.effect }
    }

    fn e_mut(&mut self) -> &mut gs_effect {
        // SAFETY: as e(), with exclusive access.
        unsafe { &mut *self.effect }
    }

    fn params_of(pass: *mut gs_effect_pass, stage: Stage) -> *mut darray {
        // SAFETY: a valid pass of the effect.
        unsafe {
            match stage {
                Stage::Vertex => &mut (*pass).vertshader_params,
                Stage::Pixel => &mut (*pass).pixelshader_params,
            }
        }
    }
}

/// The bytes of a C string, or none for NULL.
fn c_bytes<'a>(s: *const c_char) -> Option<&'a [u8]> {
    // SAFETY: null or a C string (the callers' contracts).
    (!s.is_null()).then(|| unsafe { CStr::from_ptr(s) }.to_bytes())
}

impl PassModel for CModel {
    type Technique = *mut gs_effect_technique;
    type Pass = *mut gs_effect_pass;
    type Param = *mut gs_effect_param;
    type SParam = *mut c_void;

    fn looping(&self) -> bool {
        self.e().looping
    }
    fn set_looping(&mut self, looping: bool) {
        self.e_mut().looping = looping;
    }
    fn loop_pass(&self) -> usize {
        self.e().loop_pass
    }
    fn set_loop_pass(&mut self, pass: usize) {
        self.e_mut().loop_pass = pass;
    }
    fn cur_technique(&self) -> Option<Self::Technique> {
        let t = self.e().cur_technique;
        (!t.is_null()).then_some(t)
    }
    fn set_cur_technique(&mut self, tech: Option<Self::Technique>) {
        self.e_mut().cur_technique = tech.unwrap_or(ptr::null_mut());
    }
    fn cur_pass(&self) -> Option<Self::Pass> {
        let p = self.e().cur_pass;
        (!p.is_null()).then_some(p)
    }
    fn set_cur_pass(&mut self, pass: Option<Self::Pass>) {
        self.e_mut().cur_pass = pass.unwrap_or(ptr::null_mut());
    }
    fn set_current_effect(&mut self, current: bool) {
        let effect = if current {
            self.effect
        } else {
            ptr::null_mut()
        };
        // SAFETY: the effect's graphics subsystem.
        unsafe { obs_graphics_set_cur_effect(self.e().graphics, effect) }
    }

    fn find_technique(&self, _name: &[u8]) -> Option<Self::Technique> {
        if self.name.is_null() {
            return None;
        }
        // SAFETY: a valid effect and a C string.
        let t = unsafe { gs_effect_get_technique(self.effect, self.name) };
        (!t.is_null()).then_some(t)
    }
    fn num_passes(&self, tech: Self::Technique) -> usize {
        // SAFETY: a valid technique.
        unsafe { (*tech).passes.num }
    }
    fn pass(&self, tech: Self::Technique, idx: usize) -> Self::Pass {
        // SAFETY: `idx` < the pass count (the core checks it).
        unsafe { (*tech).passes.array.cast::<gs_effect_pass>().add(idx) }
    }
    fn pass_name_is(&self, pass: Self::Pass, name: &[u8]) -> bool {
        // SAFETY: a valid pass.
        c_bytes(unsafe { (*pass).name }) == Some(name)
    }
    fn num_shader_params(&self, pass: Self::Pass, stage: Stage) -> usize {
        // SAFETY: a valid darray of the pass.
        unsafe { (*Self::params_of(pass, stage)).num }
    }
    fn shader_param(
        &self,
        pass: Self::Pass,
        stage: Stage,
        idx: usize,
    ) -> (Self::Param, Self::SParam) {
        // SAFETY: `idx` < the count (the core loops below it).
        let sp = unsafe {
            *(*Self::params_of(pass, stage))
                .array
                .cast::<pass_shaderparam>()
                .add(idx)
        };
        (sp.eparam, sp.sparam)
    }
    fn num_params(&self) -> usize {
        self.e().params.num
    }
    fn param(&self, idx: usize) -> Self::Param {
        // SAFETY: `idx` < the count (the core loops below it).
        unsafe { self.e().params.array.cast::<gs_effect_param>().add(idx) }
    }

    fn changed(&self, param: Self::Param) -> bool {
        // SAFETY: a valid parameter.
        unsafe { (*param).changed }
    }
    fn set_changed(&mut self, param: Self::Param, changed: bool) {
        // SAFETY: a valid parameter.
        unsafe { (*param).changed = changed }
    }
    fn has_next_sampler(&self, param: Self::Param) -> bool {
        // SAFETY: a valid parameter.
        unsafe { !(*param).next_sampler.is_null() }
    }
    fn clear_next_sampler(&mut self, param: Self::Param) {
        // SAFETY: a valid parameter.
        unsafe { (*param).next_sampler = ptr::null_mut() }
    }
    fn value_len(&self, param: Self::Param) -> usize {
        // SAFETY: a valid parameter.
        unsafe { (*param).cur_val.num }
    }
    fn default_len(&self, param: Self::Param) -> usize {
        // SAFETY: a valid parameter.
        unsafe { (*param).default_val.num }
    }
    fn copy_default_to_value(&mut self, param: Self::Param) {
        // SAFETY: a valid parameter; its two darrays are separate.
        unsafe {
            let default = DarrayStore::new(&mut (*param).default_val);
            let mut cur = DarrayStore::new(&mut (*param).cur_val);
            cur.resize(default.bytes().len());
            cur.bytes_mut().copy_from_slice(default.bytes());
        }
    }
    fn empty_value(&mut self, param: Self::Param) {
        // SAFETY: a valid parameter.
        unsafe { DarrayStore::new(&mut (*param).cur_val) }.resize(0);
    }

    fn effect_active(&self) -> bool {
        // SAFETY: a plain graphics call on the graphics thread.
        unsafe { !gs_get_effect().is_null() }
    }
    fn load_shaders(&mut self, pass: Option<Self::Pass>, stage: Stage) {
        // SAFETY: a valid pass or NULL; plain graphics calls.
        unsafe {
            match (stage, pass) {
                (Stage::Vertex, p) => {
                    gs_load_vertexshader(p.map_or(ptr::null_mut(), |p| (*p).vertshader))
                }
                (Stage::Pixel, p) => {
                    gs_load_pixelshader(p.map_or(ptr::null_mut(), |p| (*p).pixelshader))
                }
            }
        }
    }
    fn send_next_sampler(&mut self, sparam: Self::SParam, param: Self::Param) {
        // SAFETY: a valid parameter and shader parameter.
        unsafe { gs_shader_set_next_sampler(sparam, (*param).next_sampler) }
    }
    fn send_value(&mut self, sparam: Self::SParam, param: Self::Param) {
        // SAFETY: a valid parameter: `num` bytes at `array`.
        unsafe { gs_shader_set_val(sparam, (*param).cur_val.array, (*param).cur_val.num) }
    }
    fn is_texture(&mut self, sparam: Self::SParam) -> bool {
        let mut info = gs_shader_param_info {
            type_: 0,
            name: ptr::null(),
        };
        // SAFETY: a valid shader parameter; `info` is a local.
        unsafe { gs_shader_get_param_info(sparam, &mut info) };
        info.type_ == GS_SHADER_PARAM_TEXTURE
    }
    fn clear_texture(&mut self, sparam: Self::SParam) {
        // SAFETY: a valid shader parameter.
        unsafe { gs_shader_set_texture(sparam, ptr::null_mut()) }
    }
    fn warn(&mut self, warning: LoopWarning, _name: &[u8]) {
        let which = match warning {
            LoopWarning::AlreadyActive => 0,
            LoopWarning::TechniqueNotFound => 1,
        };
        // SAFETY: the name is null or a C string.
        unsafe { obs_graphics_effect_loop_warning(which, self.name) }
    }
}

/// The model of `tech`'s effect, or none for a NULL technique.
///
/// # Safety
///
/// `tech` is null or a valid technique of a valid effect.
unsafe fn tech_model(tech: *mut gs_effect_technique) -> Option<(CModel, *mut gs_effect_technique)> {
    // SAFETY: the caller's contract.
    (!tech.is_null()).then(|| (unsafe { CModel::new((*tech).effect, ptr::null()) }, tech))
}

/// `gs_effect_loop`.
///
/// # Safety
///
/// `effect` is null or a valid effect (see [`CModel::new`]); `name` is
/// null or a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_loop(effect: *mut gs_effect, name: *const c_char) -> bool {
    if effect.is_null() {
        return false;
    }
    // SAFETY: the caller's contract.
    let mut m = unsafe { CModel::new(effect, name) };
    effect_loop(&mut m, c_bytes(name).unwrap_or_default())
}

/// `gs_technique_begin`.
///
/// # Safety
///
/// `tech` is null or a valid technique of a valid effect.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_technique_begin(tech: *mut gs_effect_technique) -> usize {
    // SAFETY: the caller's contract.
    unsafe { tech_model(tech) }.map_or(0, |(mut m, t)| technique_begin(&mut m, Some(t)))
}

/// `gs_technique_end`.
///
/// # Safety
///
/// As [`gs_technique_begin`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_technique_end(tech: *mut gs_effect_technique) {
    // SAFETY: the caller's contract.
    if let Some((mut m, t)) = unsafe { tech_model(tech) } {
        technique_end(&mut m, Some(t));
    }
}

/// `gs_effect_update_params`.
///
/// # Safety
///
/// `effect` is null or a valid effect.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_effect_update_params(effect: *mut gs_effect) {
    if effect.is_null() {
        return;
    }
    // SAFETY: the caller's contract.
    let mut m = unsafe { CModel::new(effect, ptr::null()) };
    effect_update_params(&mut m);
}

/// `gs_technique_begin_pass`.
///
/// # Safety
///
/// As [`gs_technique_begin`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_technique_begin_pass(
    tech: *mut gs_effect_technique,
    idx: usize,
) -> bool {
    // SAFETY: the caller's contract.
    unsafe { tech_model(tech) }.is_some_and(|(mut m, t)| technique_begin_pass(&mut m, Some(t), idx))
}

/// `gs_technique_begin_pass_by_name`.
///
/// # Safety
///
/// As [`gs_technique_begin`], and `name` is null or a C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_technique_begin_pass_by_name(
    tech: *mut gs_effect_technique,
    name: *const c_char,
) -> bool {
    // SAFETY: the caller's contract.
    let Some((mut m, t)) = (unsafe { tech_model(tech) }) else {
        return false;
    };
    let Some(name) = c_bytes(name) else {
        return false;
    };
    technique_begin_pass_by_name(&mut m, Some(t), name)
}

/// `gs_technique_end_pass`.
///
/// # Safety
///
/// As [`gs_technique_begin`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gs_technique_end_pass(tech: *mut gs_effect_technique) {
    // SAFETY: the caller's contract.
    if let Some((mut m, t)) = unsafe { tech_model(tech) } {
        technique_end_pass(&mut m, Some(t));
    }
}
