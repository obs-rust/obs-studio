//! Device-load dispatch from `graphics-device-load.c.h`.
//! The context guard belongs to Rust; the device receives each argument unchanged.
use std::ffi::CStr;

pub const NOT_IN_CONTEXT: &str = "called while not in a graphics context";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Vertexbuffer,
    Indexbuffer,
    Texture,
    Samplerstate,
    DefaultSamplerstate,
}

impl Entry {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Vertexbuffer => "gs_load_vertexbuffer",
            Self::Indexbuffer => "gs_load_indexbuffer",
            Self::Texture => "gs_load_texture",
            Self::Samplerstate => "gs_load_samplerstate",
            Self::DefaultSamplerstate => "gs_load_default_samplerstate",
        }
    }
    pub const fn c_name(self) -> &'static CStr {
        match self {
            Self::Vertexbuffer => c"gs_load_vertexbuffer",
            Self::Indexbuffer => c"gs_load_indexbuffer",
            Self::Texture => c"gs_load_texture",
            Self::Samplerstate => c"gs_load_samplerstate",
            Self::DefaultSamplerstate => c"gs_load_default_samplerstate",
        }
    }
}

/// A thread's current context. Object handles are forwarded even when absent.
pub trait Host {
    type Vertexbuffer: Copy;
    type Indexbuffer: Copy;
    type Texture: Copy;
    type Samplerstate: Copy;
    fn has_context(&self) -> bool;
    fn debug(&mut self, entry: Entry);
    fn load_vertexbuffer(&mut self, buffer: Self::Vertexbuffer);
    fn load_indexbuffer(&mut self, buffer: Self::Indexbuffer);
    fn load_texture(&mut self, texture: Self::Texture, unit: i32);
    fn load_samplerstate(&mut self, sampler: Self::Samplerstate, unit: i32);
    fn load_default_samplerstate(&mut self, b_3d: bool, unit: i32);
}

// RED: no-op stubs let both safe and ABI tests report assertion failures.
pub fn load_vertexbuffer<H: Host>(_host: &mut H, _buffer: H::Vertexbuffer) {}
pub fn load_indexbuffer<H: Host>(_host: &mut H, _buffer: H::Indexbuffer) {}
pub fn load_texture<H: Host>(_host: &mut H, _texture: H::Texture, _unit: i32) {}
pub fn load_samplerstate<H: Host>(_host: &mut H, _sampler: H::Samplerstate, _unit: i32) {}
pub fn load_default_samplerstate(_host: &mut impl Host, _b_3d: bool, _unit: i32) {}
