//! Safe core of `libobs/graphics/effect-passes.c.h`: the technique and
//! pass loop of an effect, with the upload of its parameters.
//!
//! Every draw runs `while (gs_effect_loop(effect, "Draw"))`: the first call
//! begins the technique and its first pass, each later call ends the
//! current pass and begins the next, and the call after the last pass ends
//! the technique ([`effect_loop`]). Beginning a pass loads its shaders and
//! uploads every parameter that has a value or a default
//! ([`technique_begin_pass`]); [`effect_update_params`] uploads only the
//! changed ones. Ending a pass sets its texture parameters to NULL
//! ([`technique_end_pass`]); ending the technique unloads the shaders and
//! empties every parameter's value ([`technique_end`]).
//!
//! The order of the device calls and state changes is the behaviour, so the
//! whole loop is here, written once against [`PassModel`]: the effect's
//! state and the graphics device. The shim (`crate::ffi::effect_passes`)
//! implements it on the C structs and functions; the Tier 1 tests on a
//! plain model with a recording device. Nothing here keeps state or locks.
//!
//! Characterized (`test_effect_pass_loop`) and kept as the C has them:
//! `gs_effect_loop` begins pass `loop_pass` and then increments it, also
//! when there is no such pass; a next sampler is sent before the changed
//! check, so `gs_effect_update_params` sends it even for an unchanged
//! parameter; a parameter without a value gets its default copied in
//! before the upload; `gs_technique_begin_pass_by_name` returns true on a
//! name match whatever `gs_technique_begin_pass` returns.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]
#![allow(unused_variables, dead_code)]

/// The shader stage of a pass's parameter list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Vertex,
    Pixel,
}

/// The two warnings of `gs_effect_loop`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopWarning {
    /// "gs_effect_loop: An effect is already active"
    AlreadyActive,
    /// "gs_effect_loop: Technique '%s' not found."
    TechniqueNotFound,
}

/// One effect and the graphics device, as the loop sees them.
///
/// Handles are opaque and `Copy`; `Param` is a parameter of the effect,
/// `SParam` a shader parameter. The pass parameter lists pair a `Param`
/// with the `SParam` it is uploaded to.
pub trait PassModel {
    type Technique: Copy + PartialEq;
    type Pass: Copy + PartialEq;
    type Param: Copy;
    type SParam: Copy;

    // The effect's loop state.
    fn looping(&self) -> bool;
    fn set_looping(&mut self, looping: bool);
    fn loop_pass(&self) -> usize;
    fn set_loop_pass(&mut self, pass: usize);
    fn cur_technique(&self) -> Option<Self::Technique>;
    fn set_cur_technique(&mut self, tech: Option<Self::Technique>);
    fn cur_pass(&self) -> Option<Self::Pass>;
    fn set_cur_pass(&mut self, pass: Option<Self::Pass>);
    /// `graphics->cur_effect` of the effect's graphics subsystem: this
    /// effect or none.
    fn set_current_effect(&mut self, current: bool);

    // Lookups.
    /// `gs_effect_get_technique`.
    fn find_technique(&self, name: &[u8]) -> Option<Self::Technique>;
    fn num_passes(&self, tech: Self::Technique) -> usize;
    fn pass(&self, tech: Self::Technique, idx: usize) -> Self::Pass;
    fn pass_name_is(&self, pass: Self::Pass, name: &[u8]) -> bool;
    fn num_shader_params(&self, pass: Self::Pass, stage: Stage) -> usize;
    fn shader_param(
        &self,
        pass: Self::Pass,
        stage: Stage,
        idx: usize,
    ) -> (Self::Param, Self::SParam);
    fn num_params(&self) -> usize;
    fn param(&self, idx: usize) -> Self::Param;

    // A parameter's state.
    fn changed(&self, param: Self::Param) -> bool;
    fn set_changed(&mut self, param: Self::Param, changed: bool);
    fn has_next_sampler(&self, param: Self::Param) -> bool;
    fn clear_next_sampler(&mut self, param: Self::Param);
    fn value_len(&self, param: Self::Param) -> usize;
    fn default_len(&self, param: Self::Param) -> usize;
    /// `da_copy(cur_val, default_val)` with a default present.
    fn copy_default_to_value(&mut self, param: Self::Param);
    /// `da_resize(cur_val, 0)`.
    fn empty_value(&mut self, param: Self::Param);

    // The device.
    /// `gs_get_effect() != NULL`.
    fn effect_active(&self) -> bool;
    /// `gs_load_vertexshader` / `gs_load_pixelshader` with the pass's
    /// shader, or NULL with `None`.
    fn load_shaders(&mut self, pass: Option<Self::Pass>, stage: Stage);
    /// `gs_shader_set_next_sampler(sparam, param->next_sampler)`.
    fn send_next_sampler(&mut self, sparam: Self::SParam, param: Self::Param);
    /// `gs_shader_set_val(sparam, cur_val.array, cur_val.num)`.
    fn send_value(&mut self, sparam: Self::SParam, param: Self::Param);
    /// `gs_shader_get_param_info(sparam).type == GS_SHADER_PARAM_TEXTURE`.
    fn is_texture(&mut self, sparam: Self::SParam) -> bool;
    /// `gs_shader_set_texture(sparam, NULL)`.
    fn clear_texture(&mut self, sparam: Self::SParam);
    fn warn(&mut self, warning: LoopWarning, name: &[u8]);
}

/// `gs_effect_loop`.
pub fn effect_loop<M: PassModel>(m: &mut M, name: &[u8]) -> bool {
    todo!()
}

/// `gs_technique_begin`: the number of passes.
pub fn technique_begin<M: PassModel>(m: &mut M, tech: Option<M::Technique>) -> usize {
    todo!()
}

/// `gs_technique_end`.
pub fn technique_end<M: PassModel>(m: &mut M, tech: Option<M::Technique>) {
    todo!()
}

/// `upload_shader_params`.
fn upload_shader_params<M: PassModel>(m: &mut M, pass: M::Pass, stage: Stage, changed_only: bool) {
    for i in 0..m.num_shader_params(pass, stage) {
        let (param, sparam) = m.shader_param(pass, stage, i);
        if m.has_next_sampler(param) {
            m.send_next_sampler(sparam, param);
        }
        if changed_only && !m.changed(param) {
            continue;
        }
        if m.value_len(param) == 0 {
            if m.default_len(param) != 0 {
                m.copy_default_to_value(param);
            } else {
                continue;
            }
        }
        m.send_value(sparam, param);
    }
}

/// `reset_params`.
fn reset_params<M: PassModel>(m: &mut M, pass: M::Pass, stage: Stage) {
    for i in 0..m.num_shader_params(pass, stage) {
        let (param, _) = m.shader_param(pass, stage, i);
        m.set_changed(param, false);
    }
}

/// `upload_parameters`.
fn upload_parameters<M: PassModel>(m: &mut M, changed_only: bool) {
    let Some(pass) = m.cur_pass() else {
        return;
    };
    upload_shader_params(m, pass, Stage::Vertex, changed_only);
    upload_shader_params(m, pass, Stage::Pixel, changed_only);
    reset_params(m, pass, Stage::Vertex);
    reset_params(m, pass, Stage::Pixel);
}

/// `gs_effect_update_params`: only the changed parameters.
pub fn effect_update_params<M: PassModel>(m: &mut M) {
    todo!()
}

/// `gs_technique_begin_pass`.
pub fn technique_begin_pass<M: PassModel>(
    m: &mut M,
    tech: Option<M::Technique>,
    idx: usize,
) -> bool {
    todo!()
}

/// `gs_technique_begin_pass_by_name`.
pub fn technique_begin_pass_by_name<M: PassModel>(
    m: &mut M,
    tech: Option<M::Technique>,
    name: &[u8],
) -> bool {
    todo!()
}

/// `clear_tex_params`.
fn clear_tex_params<M: PassModel>(m: &mut M, pass: M::Pass, stage: Stage) {
    for i in 0..m.num_shader_params(pass, stage) {
        let (_, sparam) = m.shader_param(pass, stage, i);
        if m.is_texture(sparam) {
            m.clear_texture(sparam);
        }
    }
}

/// `gs_technique_end_pass`.
pub fn technique_end_pass<M: PassModel>(m: &mut M, tech: Option<M::Technique>) {
    todo!()
}
