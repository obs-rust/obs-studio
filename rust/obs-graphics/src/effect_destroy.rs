//! Safe core for `libobs/graphics/effect.c.h`: the effect destroy,
//! `gs_effect_destroy` and `gs_effect_actually_destroy`, with the walk of
//! the header-inline frees of `graphics/effect.h` they run (`effect_free`,
//! `effect_param_free`, `effect_technique_free` and `effect_pass_free`).
//!
//! The core runs over an [`EffectModel`]: one read where the C reads a
//! field (the cached flag, each count and each item) and one
//! [`EffectModel::free`] call where the C frees something, in the C's order,
//! so a shim frees the C structs in place. A count is read again before each
//! item, as the C's `for` loops do.
//!
//! Kept as the C has it: `gs_effect_destroy` reads only the cached flag of
//! a non-null effect and leaves a cached one alone (it stays on the graphics
//! subsystem's list until `gs_destroy` frees it with
//! `gs_effect_actually_destroy`, which does not look at the flag); both
//! shaders of every pass are destroyed, a null one included; nothing that
//! points at the effect is cleared.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

/// One free of the effect destroy, on the item it names: `P` a parameter or
/// annotation, `T` a technique, `S` a pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Free<P, T, S> {
    /// `bfree(param->name)`.
    ParamName(P),
    /// `da_free(param->cur_val)`.
    ParamCurVal(P),
    /// `da_free(param->default_val)`.
    ParamDefaultVal(P),
    /// `da_free(param->annotations)`, after each annotation's own frees.
    ParamAnnotations(P),
    /// `bfree(pass->name)`.
    PassName(S),
    /// `da_free(pass->vertshader_params)`.
    PassVertParams(S),
    /// `da_free(pass->pixelshader_params)`.
    PassPixelParams(S),
    /// `gs_shader_destroy(pass->vertshader)`.
    PassVertShader(S),
    /// `gs_shader_destroy(pass->pixelshader)`.
    PassPixelShader(S),
    /// `da_free(technique->passes)`, after each pass's frees.
    TechniquePasses(T),
    /// `bfree(technique->name)`.
    TechniqueName(T),
    /// `da_free(effect->params)`.
    Params,
    /// `da_free(effect->techniques)`.
    Techniques,
    /// `bfree(effect->effect_path)`.
    EffectPath,
    /// `bfree(effect->effect_dir)`.
    EffectDir,
    /// `effect->effect_path = NULL; effect->effect_dir = NULL;`.
    ClearPaths,
    /// `bfree(effect)`.
    Effect,
}

/// The effect being destroyed, as the C reads and frees it.
pub trait EffectModel {
    /// A parameter or annotation (`struct gs_effect_param`).
    type Param: Copy;
    /// A technique (`struct gs_effect_technique`).
    type Technique: Copy;
    /// A pass (`struct gs_effect_pass`).
    type Pass: Copy;

    /// `effect->cached`.
    fn cached(&self) -> bool;
    /// `effect->params.num`.
    fn param_count(&self) -> usize;
    /// `effect->params.array + index`.
    fn param(&self, index: usize) -> Self::Param;
    /// `param->annotations.num`.
    fn annotation_count(&self, param: Self::Param) -> usize;
    /// `param->annotations.array + index`.
    fn annotation(&self, param: Self::Param, index: usize) -> Self::Param;
    /// `effect->techniques.num`.
    fn technique_count(&self) -> usize;
    /// `effect->techniques.array + index`.
    fn technique(&self, index: usize) -> Self::Technique;
    /// `technique->passes.num`.
    fn pass_count(&self, technique: Self::Technique) -> usize;
    /// `technique->passes.array + index`.
    fn pass(&self, technique: Self::Technique, index: usize) -> Self::Pass;
    /// One free, in place.
    fn free(&mut self, step: Free<Self::Param, Self::Technique, Self::Pass>);
}

/// `gs_effect_destroy`: nothing for a null or cached effect, otherwise
/// [`effect_actually_destroy`].
pub fn effect_destroy<M: EffectModel>(effect: Option<&mut M>) {
    if let Some(effect) = effect
        && !effect.cached()
    {
        effect_actually_destroy(effect);
    }
}

/// `gs_effect_actually_destroy`: [`effect_free`], then the effect itself,
/// cached or not.
pub fn effect_actually_destroy<M: EffectModel>(effect: &mut M) {
    effect_free(effect);
    effect.free(Free::Effect);
}

/// `effect_free`: each parameter, each technique, the two arrays, then the
/// two paths, which it then clears.
pub fn effect_free<M: EffectModel>(effect: &mut M) {
    let mut i = 0;
    while i < effect.param_count() {
        let param = effect.param(i);
        effect_param_free(effect, param);
        i = i.wrapping_add(1);
    }
    let mut i = 0;
    while i < effect.technique_count() {
        let technique = effect.technique(i);
        effect_technique_free(effect, technique);
        i = i.wrapping_add(1);
    }

    effect.free(Free::Params);
    effect.free(Free::Techniques);

    effect.free(Free::EffectPath);
    effect.free(Free::EffectDir);
    effect.free(Free::ClearPaths);
}

/// `effect_param_free`: the name, the current and default values, each
/// annotation the same way, then the annotation array.
pub fn effect_param_free<M: EffectModel>(effect: &mut M, param: M::Param) {
    effect.free(Free::ParamName(param));
    effect.free(Free::ParamCurVal(param));
    effect.free(Free::ParamDefaultVal(param));

    let mut i = 0;
    while i < effect.annotation_count(param) {
        let annotation = effect.annotation(param, i);
        effect_param_free(effect, annotation);
        i = i.wrapping_add(1);
    }

    effect.free(Free::ParamAnnotations(param));
}

/// `effect_technique_free`: each pass, the pass array, then the name.
pub fn effect_technique_free<M: EffectModel>(effect: &mut M, technique: M::Technique) {
    let mut i = 0;
    while i < effect.pass_count(technique) {
        let pass = effect.pass(technique, i);
        effect_pass_free(effect, pass);
        i = i.wrapping_add(1);
    }

    effect.free(Free::TechniquePasses(technique));
    effect.free(Free::TechniqueName(technique));
}

/// `effect_pass_free`: the name, the two shader parameter arrays, then the
/// vertex and the pixel shader.
pub fn effect_pass_free<M: EffectModel>(effect: &mut M, pass: M::Pass) {
    effect.free(Free::PassName(pass));
    effect.free(Free::PassVertParams(pass));
    effect.free(Free::PassPixelParams(pass));

    effect.free(Free::PassVertShader(pass));
    effect.free(Free::PassPixelShader(pass));
}
