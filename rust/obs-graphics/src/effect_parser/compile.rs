//! Safe core of `libobs/graphics/effect-parser-compile.c.h`: the effect
//! parser's compile step.
//!
//! Once `ep_parse()` has read the whole text with no error, it calls
//! `ep_compile()` ([`compile`]), which fills the effect from the parser:
//!
//! - [`compile`] (`ep_compile`) sizes the effect's params to the parser's
//!   params and its techniques to the parser's techniques, then compiles
//!   each param and each technique, in order;
//! - [`compile_param`] (`ep_compile_param`, with `ep_compile_param_annotations`
//!   and [`compile_annotations`], `ep_compile_annotations`) links the
//!   parser's param to its effect param, gives it a copy of its name, its
//!   section, its effect, the parser's default value (moved out of the
//!   parser) and its type (`obs_effect_param_type()`,
//!   [`crate::effect_param_types::param_type`]); a param named exactly
//!   `ViewProj` or `World` (case-sensitive) becomes the effect's
//!   `view_proj` or `world`; then its annotations the same way, with the
//!   annotation section;
//! - [`compile_technique`] (`ep_compile_technique`) gives each technique a
//!   copy of its name, its section and its effect, sizes its passes, and
//!   compiles each pass ([`compile_pass`], `ep_compile_pass`): a copy of its
//!   name, its section, then its vertex and then its pixel shader
//!   ([`compile_pass_shader`], `ep_compile_pass_shader`), each failure
//!   logged as a missing shader;
//! - [`compile_pass_shader`] builds the shader's location (`<file> (Vertex
//!   shader, technique <name>, pass <n>)`), the shader text the writer
//!   builds for the pass's call ([`super::writer`]), creates the shader with
//!   them, adds a non-empty error text the device hands back to the
//!   parser's error list, and, when there is a shader, pairs each param the
//!   text uses with the effect's and the shader's param of that name
//!   ([`compile_pass_shaderparams`], `ep_compile_pass_shaderparams`), which
//!   stops at the first the shader lacks.
//!
//! Everything goes through a [`CompileModel`], one call where the C makes
//! the step, in the C's order and under its conditions: the shim writes
//! the C structs in place, and the device calls are the C's. The core
//! never frees anything the effect holds: a failed pass keeps its shader
//! and the shader params it got, for `effect_free()`.
//!
//! Characterized, not endorsed, and kept (`test_effect_parser_compile`): a
//! second `ViewProj` or `World` becomes `view_proj` or `world` in place of
//! the first, while the shader-param lookups find the first; a shader
//! created together with an error text is kept, the result stays true, and
//! the error is in the parser's list; a shader lacking a param keeps the
//! params before it, that one with no shader param, and the rest empty.
//!
//! One intentional difference: the C's `_DEBUG_SHADERS` lines (a
//! developer-only define no build sets) are not written.

use crate::effect_param_types;

/// `EFFECT_PARAM` (`enum effect_section`, `graphics/effect.h`).
pub const EFFECT_PARAM: i32 = 0;
/// `EFFECT_TECHNIQUE`.
pub const EFFECT_TECHNIQUE: i32 = 1;
/// `EFFECT_SAMPLER`.
pub const EFFECT_SAMPLER: i32 = 2;
/// `EFFECT_PASS`.
pub const EFFECT_PASS: i32 = 3;
/// `EFFECT_ANNOTATION`.
pub const EFFECT_ANNOTATION: i32 = 4;

/// `GS_SHADER_VERTEX` or `GS_SHADER_PIXEL`: which shader of a pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShaderType {
    /// The vertex shader, from the pass's `vertex_program`.
    Vertex,
    /// The pixel shader, from the pass's `fragment_program`.
    Pixel,
}

/// A param or annotation of the effect, with the parser's param or
/// annotation it is compiled from (the same indices on both sides).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamSlot {
    /// `effect->params.array[i]`, from `ep->params.array[i]`.
    Param(usize),
    /// Annotation `index` of param `param`.
    Annotation {
        /// The param's index.
        param: usize,
        /// The annotation's index.
        index: usize,
    },
}

/// A shader of one pass: technique `tech`, pass `pass`, vertex or pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShaderSlot {
    /// The technique's index.
    pub tech: usize,
    /// The pass's index in its technique.
    pub pass: usize,
    /// Which shader of the pass.
    pub kind: ShaderType,
}

/// The `LOG_ERROR` lines of the compile step, with what they print.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompileLog<'a> {
    /// `Effect shader parameter not found`.
    ShaderParamNotFound,
    /// `Pass (<idx>) <<name>> missing vertex shader!`, the name empty for
    /// an unnamed pass.
    MissingVertexShader {
        /// The pass's index.
        idx: usize,
        /// The pass's name.
        name: &'a [u8],
    },
    /// `Pass (<idx>) <<name>> missing pixel shader!`.
    MissingPixelShader {
        /// The pass's index.
        idx: usize,
        /// The pass's name.
        name: &'a [u8],
    },
}

/// The parser, the effect it fills and the graphics device, as the compile
/// step reads and writes them.
pub trait CompileModel {
    /// A created shader (never `NULL`).
    type Shader: Copy;
    /// The shader text and used params `ep_makeshaderstring()` builds for
    /// one call.
    type Built;
    /// The error text a shader create hands back (`char *errors`).
    type Errors;
    /// An effect param a shader param is paired with (`NULL` allowed).
    type EParam: Copy;
    /// A shader's param.
    type SParam: Copy;

    /// `ep->params.num`.
    fn param_count(&self) -> usize;
    /// `ep->techniques.num`.
    fn technique_count(&self) -> usize;
    /// `da_resize(ep->effect->params, num)`: new params all zero.
    fn resize_params(&mut self, num: usize);
    /// `da_resize(ep->effect->techniques, num)`: new techniques all zero.
    fn resize_techniques(&mut self, num: usize);

    /// `param_in->param = param`: the parser's param `i` points at the
    /// effect's.
    fn link_param(&mut self, i: usize);
    /// The parser's type name of the slot (`param_in->type`).
    fn param_type_name(&self, slot: ParamSlot) -> &[u8];
    /// The slot's name (`bstrdup(param_in->name)`), `section`, effect
    /// (`ep->effect`), default value (`da_move` from the parser's) and
    /// `type_`, in the C's order.
    fn fill_param(&mut self, slot: ParamSlot, section: i32, type_: i32);
    /// The parser's name of param `i` (`param_in->name`), for the
    /// `ViewProj` and `World` checks.
    fn param_name(&self, i: usize) -> &[u8];
    /// `ep->effect->view_proj = param`.
    fn set_view_proj(&mut self, i: usize);
    /// `ep->effect->world = param`.
    fn set_world(&mut self, i: usize);
    /// The parser's annotation count of param `i`
    /// (`ep_annotations->num`).
    fn annotation_count(&self, i: usize) -> usize;
    /// `da_resize(*gsp_annotations, num)` for param `i`.
    fn resize_annotations(&mut self, i: usize, num: usize);

    /// Technique `t`'s name (`bstrdup(tech_in->name)`), section
    /// ([`EFFECT_TECHNIQUE`]) and effect.
    fn fill_technique(&mut self, t: usize);
    /// The parser's pass count of technique `t` (`tech_in->passes.num`).
    fn parser_pass_count(&self, t: usize) -> usize;
    /// `da_resize(tech->passes, num)`.
    fn resize_passes(&mut self, t: usize, num: usize);
    /// The effect's pass count of technique `t` (`tech->passes.num`).
    fn pass_count(&self, t: usize) -> usize;
    /// Pass `p` of technique `t`: its name (`bstrdup(pass_in->name)`) and
    /// section ([`EFFECT_PASS`]).
    fn fill_pass(&mut self, t: usize, p: usize);
    /// The effect's name of pass `p` of technique `t` (`pass->name`),
    /// `None` for `NULL`.
    fn pass_name(&self, t: usize, p: usize) -> Option<&[u8]>;
    /// The effect's name of technique `t` (`tech->name`), `None` for
    /// `NULL`.
    fn technique_name(&self, t: usize) -> Option<&[u8]>;
    /// The parser's file name (`ep->cfp.lex.file`), `None` for `NULL`.
    fn file(&self) -> Option<&[u8]>;

    /// `ep_makeshaderstring()` on the slot's call, into a fresh shader
    /// string and used-params list.
    fn make_shader(&mut self, slot: ShaderSlot) -> Self::Built;
    /// `gs_vertexshader_create()` or `gs_pixelshader_create()` with the
    /// built text and `location`, stored as the pass's `vertshader` or
    /// `pixelshader`; the shader (`None` for `NULL`) and the error text it
    /// handed back.
    fn create_shader(
        &mut self,
        slot: ShaderSlot,
        built: &Self::Built,
        location: &[u8],
    ) -> (Option<Self::Shader>, Self::Errors);
    /// `errors && strlen(errors)`.
    fn errors_nonempty(&self, errors: &Self::Errors) -> bool;
    /// `cf_adderror(&ep->cfp, "Error creating shader: $1", LEX_ERROR,
    /// errors, NULL, NULL)`.
    fn add_shader_error(&mut self, errors: &Self::Errors);
    /// `bfree(errors)`.
    fn free_errors(&mut self, errors: Self::Errors);

    /// `used_params->num` of the built call.
    fn used_count(&self, built: &Self::Built) -> usize;
    /// `da_resize(*pass_params, num)` of the slot's shader params.
    fn resize_shader_params(&mut self, slot: ShaderSlot, num: usize);
    /// `pass_params->num` of the slot.
    fn shader_param_count(&self, slot: ShaderSlot) -> usize;
    /// `gs_effect_get_param_by_name(ep->effect, used[i])`.
    fn effect_param(&mut self, built: &Self::Built, i: usize) -> Self::EParam;
    /// `gs_shader_get_param_by_name(shader, used[i])`, `None` for `NULL`.
    fn shader_param(
        &mut self,
        shader: Self::Shader,
        built: &Self::Built,
        i: usize,
    ) -> Option<Self::SParam>;
    /// Shader param `i` of the slot: `eparam` and `sparam`.
    fn set_shader_param(
        &mut self,
        slot: ShaderSlot,
        i: usize,
        eparam: Self::EParam,
        sparam: Option<Self::SParam>,
    );
    /// Frees the built text and used params.
    fn free_built(&mut self, built: Self::Built);

    /// `blog(LOG_ERROR, ...)`.
    fn log(&mut self, line: CompileLog<'_>);
}

/// `ep_compile_annotations()`: param `i`'s annotations.
pub fn compile_annotations(m: &mut impl CompileModel, i: usize) {
    let num = m.annotation_count(i);
    m.resize_annotations(i, num);
    for index in 0..num {
        let slot = ParamSlot::Annotation { param: i, index };
        let type_ = effect_param_types::param_type(m.param_type_name(slot));
        m.fill_param(slot, EFFECT_ANNOTATION, type_);
    }
}

/// `ep_compile_param()`: param `i`, then its annotations.
pub fn compile_param(m: &mut impl CompileModel, i: usize) {
    m.link_param(i);
    let slot = ParamSlot::Param(i);
    let type_ = effect_param_types::param_type(m.param_type_name(slot));
    m.fill_param(slot, EFFECT_PARAM, type_);

    let name = m.param_name(i);
    let (view_proj, world) = (name == b"ViewProj", name == b"World");
    if view_proj {
        m.set_view_proj(i);
    } else if world {
        m.set_world(i);
    }

    compile_annotations(m, i);
}

/// `ep_compile_pass_shaderparams()`: pairs each param the built text uses
/// with the effect's and the shader's param of that name; false at the
/// first one the shader lacks.
pub fn compile_pass_shaderparams<M: CompileModel>(
    m: &mut M,
    slot: ShaderSlot,
    built: &M::Built,
    shader: M::Shader,
) -> bool {
    let num = m.used_count(built);
    m.resize_shader_params(slot, num);

    let mut i = 0;
    while i < m.shader_param_count(slot) {
        let eparam = m.effect_param(built, i);
        let sparam = m.shader_param(shader, built, i);
        m.set_shader_param(slot, i, eparam, sparam);

        if sparam.is_none() {
            m.log(CompileLog::ShaderParamNotFound);
            return false;
        }
        i += 1;
    }

    true
}

/// The location `ep_compile_pass_shader()` names a shader with: `<file>
/// (Vertex shader, technique <name>, pass <n>)` or `(Pixel ...`, a `NULL`
/// file empty and a `NULL` technique name `(null)`, as `printf` writes it.
#[must_use]
pub fn location(
    file: Option<&[u8]>,
    kind: ShaderType,
    tech_name: Option<&[u8]>,
    pass: usize,
) -> Vec<u8> {
    let mut out = file.unwrap_or_default().to_vec();
    out.extend_from_slice(match kind {
        ShaderType::Vertex => b" (Vertex ",
        ShaderType::Pixel => b" (Pixel ",
    });
    out.extend_from_slice(b"shader, technique ");
    out.extend_from_slice(tech_name.unwrap_or(b"(null)"));
    out.extend_from_slice(b", pass ");
    // (unsigned)pass_idx, past the C's assert that it fits.
    #[allow(clippy::cast_possible_truncation)]
    let pass = pass as u32;
    out.extend_from_slice(pass.to_string().as_bytes());
    out.push(b')');
    out
}

/// `ep_compile_pass_shader()`: one shader of a pass; false when it was not
/// created or lacks a param.
pub fn compile_pass_shader(m: &mut impl CompileModel, slot: ShaderSlot) -> bool {
    let location = location(m.file(), slot.kind, m.technique_name(slot.tech), slot.pass);

    let built = m.make_shader(slot);
    let (shader, errors) = m.create_shader(slot, &built, &location);

    if m.errors_nonempty(&errors) {
        m.add_shader_error(&errors);
    }
    m.free_errors(errors);

    let success = match shader {
        Some(shader) => compile_pass_shaderparams(m, slot, &built, shader),
        None => false,
    };

    m.free_built(built);
    success
}

/// `ep_compile_pass()`: pass `p` of technique `t`; false when either
/// shader failed.
pub fn compile_pass(m: &mut impl CompileModel, t: usize, p: usize) -> bool {
    let mut success = true;

    m.fill_pass(t, p);

    for kind in [ShaderType::Vertex, ShaderType::Pixel] {
        let slot = ShaderSlot {
            tech: t,
            pass: p,
            kind,
        };
        if !compile_pass_shader(m, slot) {
            success = false;
            let name = m.pass_name(t, p).unwrap_or_default().to_vec();
            let name = name.as_slice();
            m.log(match kind {
                ShaderType::Vertex => CompileLog::MissingVertexShader { idx: p, name },
                ShaderType::Pixel => CompileLog::MissingPixelShader { idx: p, name },
            });
        }
    }
    success
}

/// `ep_compile_technique()`: technique `t` and its passes; false when any
/// pass failed (every pass is still compiled).
pub fn compile_technique(m: &mut impl CompileModel, t: usize) -> bool {
    let mut success = true;

    m.fill_technique(t);

    let num = m.parser_pass_count(t);
    m.resize_passes(t, num);

    let mut p = 0;
    while p < m.pass_count(t) {
        if !compile_pass(m, t, p) {
            success = false;
        }
        p += 1;
    }

    success
}

/// `ep_compile()`: the whole effect; false when any technique failed.
pub fn compile(m: &mut impl CompileModel) -> bool {
    let mut success = true;

    let params = m.param_count();
    m.resize_params(params);
    let techniques = m.technique_count();
    m.resize_techniques(techniques);

    let mut i = 0;
    while i < m.param_count() {
        compile_param(m, i);
        i += 1;
    }

    let mut t = 0;
    while t < m.technique_count() {
        if !compile_technique(m, t) {
            success = false;
        }
        t += 1;
    }

    success
}
