//! Internal video render orchestration. Graphics handles are opaque identity tokens;
//! every field lookup and graphics operation belongs to the supplied adapter.

/// Operations of the private render adapter, in C ABI order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Operation {
    /// Read the selected mix's render texture.
    MixTexture,
    /// Get the default effect.
    DefaultEffect,
    /// Get the effect's image parameter.
    ImageParameter,
    /// Bind an sRGB texture.
    BindTexture,
    /// Save blending state.
    PushBlend,
    /// Disable blending.
    DisableBlend,
    /// Enable framebuffer sRGB.
    EnableSrgb,
    /// Advance the Draw effect loop.
    EffectLoop,
    /// Draw the whole texture with default dimensions and flags.
    Sprite,
    /// Reset framebuffer sRGB to false.
    DisableSrgb,
    /// Restore blending state.
    PopBlend,
    /// Look up a conversion technique.
    Technique,
    /// Read texture width.
    Width,
    /// Read texture height.
    Height,
    /// Bind the render target without depth.
    Target,
    /// Delegate render size to the existing helper.
    Size,
    /// Begin the technique, returning its pass count.
    Begin,
    /// Begin the indexed pass; its result is deliberately ignored.
    BeginPass,
    /// Draw three triangle vertices, starting at zero.
    Triangle,
    /// End the pass.
    EndPass,
    /// End the technique.
    End,
}

/// Copy another mix's complete canvas, including alpha, without blending.
pub fn draw_mix_texture(
    _index: usize,
    _call: impl FnMut(Operation, usize, usize) -> usize,
) {
    todo!()
}

/// Render every conversion pass, including passes whose begin call returns false.
pub fn render_convert_plane(
    _effect: usize,
    _target: usize,
    _name: usize,
    _call: impl FnMut(Operation, usize, usize) -> usize,
) {
    todo!()
}
