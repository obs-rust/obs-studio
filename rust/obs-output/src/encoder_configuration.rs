//! Public encoder configuration orchestration; private layout stays in C.
use core::ffi::CStr;

#[derive(Clone, Copy, Debug)]
pub struct Configuration {
    pub kind: i32,
    pub active: bool,
    pub initialized: bool,
    pub has_override: bool,
    pub has_media: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Request {
    ScaledSize(u32, u32),
    GpuScale(i32),
    Divisor(u32),
}

/// Effects are synchronous and operate on the caller's exclusively held encoder.
pub trait Effects {
    fn log(&mut self, level: i32, format: &'static CStr);
    fn output_size(&mut self) -> Option<(u32, u32)>;
    fn scaled_size(&mut self, width: u32, height: u32);
    fn gpu_scale(&mut self, scale: i32);
    fn divisor(&mut self, divisor: u32);
    fn free_override(&mut self);
    fn create_override(&mut self, divisor: u32);
}

pub fn configure(
    state: Option<Configuration>,
    request: Request,
    effects: &mut impl Effects,
) -> bool {
    let _ = (state, request, effects);
    todo!("port encoder configuration setters")
}
