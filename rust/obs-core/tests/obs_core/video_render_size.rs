//! Tier 1: every assertion of test_video_render_size.c, without FFI.
use obs_core::video_render_size::{Graphics, set_render_size};

#[derive(Debug, PartialEq)]
pub enum Call {
    Depth(bool),
    Cull(i32),
    Ortho([f32; 6]),
    Viewport([i32; 4]),
}
#[derive(Default)]
pub struct Recorder(pub Vec<Call>);
impl Graphics for Recorder {
    fn depth(&mut self, value: bool) { self.0.push(Call::Depth(value)); }
    fn cull(&mut self, value: i32) { self.0.push(Call::Cull(value)); }
    fn ortho(&mut self, value: [f32; 6]) { self.0.push(Call::Ortho(value)); }
    fn viewport(&mut self, value: [i32; 4]) { self.0.push(Call::Viewport(value)); }
}

#[test]
fn c_dimensions_and_call_order() {
    let mut recorder = Recorder::default();
    for (width, height) in [(0,0),(0,1),(1,0),(1,1),(1920,1080),(1080,1920),(16777215,16777217),(i32::MAX as u32,i32::MAX as u32),(u32::MAX,u32::MAX)] {
        recorder.0.clear();
        set_render_size(width, height, &mut recorder);
        assert_eq!(recorder.0, [Call::Depth(false), Call::Cull(0), Call::Ortho([0.0,width as f32,0.0,height as f32,-100.0,100.0]),Call::Viewport([0,0,width as i32,height as i32])]);
    }
}
