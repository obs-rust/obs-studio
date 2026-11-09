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
pub fn draw_mix_texture(index: usize, mut call: impl FnMut(Operation, usize, usize) -> usize) {
    let texture = call(Operation::MixTexture, index, 0);
    let effect = call(Operation::DefaultEffect, 0, 0);
    let parameter = call(Operation::ImageParameter, effect, 0);
    call(Operation::BindTexture, parameter, texture);
    call(Operation::PushBlend, 0, 0);
    call(Operation::DisableBlend, 0, 0);
    call(Operation::EnableSrgb, 0, 0);
    while call(Operation::EffectLoop, effect, 0) != 0 {
        call(Operation::Sprite, texture, 0);
    }
    call(Operation::DisableSrgb, 0, 0);
    call(Operation::PopBlend, 0, 0);
}

/// Render every conversion pass, including passes whose begin call returns false.
pub fn render_convert_plane(
    effect: usize,
    target: usize,
    name: usize,
    mut call: impl FnMut(Operation, usize, usize) -> usize,
) {
    let technique = call(Operation::Technique, effect, name);
    let width = call(Operation::Width, target, 0);
    let height = call(Operation::Height, target, 0);
    call(Operation::Target, target, 0);
    call(Operation::Size, width, height);
    let passes = call(Operation::Begin, technique, 0);
    for index in 0..passes {
        // Characterized, not endorsed: C draws even if begin_pass returns false.
        call(Operation::BeginPass, technique, index);
        call(Operation::Triangle, 0, 0);
        call(Operation::EndPass, technique, 0);
    }
    call(Operation::End, technique, 0);
}
