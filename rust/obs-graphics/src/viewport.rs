//! Viewport and maximum texture capability policy over an opaque backend.
pub trait Backend {
    fn valid(&mut self, function: &'static str) -> bool;
    fn size(&mut self) -> (u32, u32);
    fn viewport(&mut self, value: [i32; 4]);
    fn ortho(&mut self, value: [f32; 6]);
    fn maximum(&mut self) -> Option<u32>;
    fn device_type(&mut self) -> i32;
}
pub fn reset_viewport(_backend: &mut impl Backend) { todo!() }
pub fn set_2d_mode(_backend: &mut impl Backend) { todo!() }
pub fn max_texture_size(_backend: &mut impl Backend) -> u32 { todo!() }
