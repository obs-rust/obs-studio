//! Viewport and maximum texture capability policy over an opaque backend.
pub trait Backend {
    fn valid(&mut self, function: &'static str) -> bool;
    fn size(&mut self) -> (u32, u32);
    fn viewport(&mut self, value: [i32; 4]);
    fn ortho(&mut self, value: [f32; 6]);
    fn maximum(&mut self) -> Option<u32>;
    fn device_type(&mut self) -> i32;
}
pub fn reset_viewport(backend: &mut impl Backend) {
    if !backend.valid("gs_reset_viewport") {
        return;
    }
    let (width, height) = backend.size();
    backend.viewport([0, 0, width as i32, height as i32]);
}
pub fn set_2d_mode(backend: &mut impl Backend) {
    if !backend.valid("gs_set_2d_mode") {
        return;
    }
    let (width, height) = backend.size();
    backend.ortho([0., width as f32, 0., height as f32, -1., -1024.]);
}
pub fn max_texture_size(backend: &mut impl Backend) -> u32 {
    if let Some(maximum) = backend.maximum() {
        return maximum;
    }
    match backend.device_type() {
        2 | 3 => 16384,
        _ => 0,
    }
}
