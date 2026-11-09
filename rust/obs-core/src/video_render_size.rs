//! Graphics setup for the private video render-size helper.

/// Graphics operations in the order required before rendering a video frame.
pub trait Graphics {
    fn depth(&mut self, enabled: bool);
    fn cull(&mut self, mode: i32);
    fn ortho(&mut self, planes: [f32; 6]);
    fn viewport(&mut self, rect: [i32; 4]);
}

/// Preserve the C helper's unsigned-to-float and unsigned-to-int conversions.
pub fn set_render_size(_width: u32, _height: u32, _graphics: &mut impl Graphics) {
    todo!()
}
