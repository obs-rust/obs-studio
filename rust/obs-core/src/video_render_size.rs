//! Graphics setup for the private video render-size helper.

/// Graphics operations in the order required before rendering a video frame.
pub trait Graphics {
    fn depth(&mut self, enabled: bool);
    fn cull(&mut self, mode: i32);
    fn ortho(&mut self, planes: [f32; 6]);
    fn viewport(&mut self, rect: [i32; 4]);
}

/// Preserve the C helper's unsigned-to-float and unsigned-to-int conversions.
pub fn set_render_size(width: u32, height: u32, graphics: &mut impl Graphics) {
    graphics.depth(false);
    graphics.cull(2); // GS_NEITHER, checked against the original C by Tier 3.
    graphics.ortho([0.0, width as f32, 0.0, height as f32, -100.0, 100.0]);
    graphics.viewport([0, 0, width as i32, height as i32]);
}
