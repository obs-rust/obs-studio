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

/// Check before the host can access any device fields.
fn valid(host: &mut impl Host, entry: Entry) -> bool {
    if host.has_context() {
        return true;
    }
    host.debug(entry);
    false
}

pub fn load_vertexbuffer<H: Host>(host: &mut H, buffer: H::Vertexbuffer) {
    if valid(host, Entry::Vertexbuffer) {
        host.load_vertexbuffer(buffer);
    }
}

pub fn load_indexbuffer<H: Host>(host: &mut H, buffer: H::Indexbuffer) {
    if valid(host, Entry::Indexbuffer) {
        host.load_indexbuffer(buffer);
    }
}

pub fn load_texture<H: Host>(host: &mut H, texture: H::Texture, unit: i32) {
    if valid(host, Entry::Texture) {
        host.load_texture(texture, unit);
    }
}

pub fn load_samplerstate<H: Host>(host: &mut H, sampler: H::Samplerstate, unit: i32) {
    if valid(host, Entry::Samplerstate) {
        host.load_samplerstate(sampler, unit);
    }
}

pub fn load_default_samplerstate(host: &mut impl Host, b_3d: bool, unit: i32) {
    if valid(host, Entry::DefaultSamplerstate) {
        host.load_default_samplerstate(b_3d, unit);
    }
}
