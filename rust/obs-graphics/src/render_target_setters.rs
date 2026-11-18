//! Render-target dispatch decisions, independent of private graphics layouts.
use std::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    RenderTarget,
    CubeRenderTarget,
}

impl Entry {
    pub const fn name(self) -> &'static str {
        match self {
            Self::RenderTarget => "gs_set_render_target",
            Self::CubeRenderTarget => "gs_set_cube_render_target",
        }
    }

    pub const fn c_name(self) -> &'static CStr {
        match self {
            Self::RenderTarget => c"gs_set_render_target",
            Self::CubeRenderTarget => c"gs_set_cube_render_target",
        }
    }
}

pub const NO_CONTEXT: &CStr = c"called while not in a graphics context";

/// The backend is reached only with a current context. Both handles are nullable.
pub trait Host {
    type Texture: Copy;
    type Depth: Copy;
    fn has_context(&self) -> bool;
    fn debug(&mut self, entry: Entry);
    fn target(&mut self, texture: Option<Self::Texture>, depth: Option<Self::Depth>);
    fn cube(&mut self, texture: Option<Self::Texture>, side: i32, depth: Option<Self::Depth>);
}

pub fn set_render_target<H: Host>(
    _host: &mut H,
    _texture: Option<H::Texture>,
    _depth: Option<H::Depth>,
) {
    todo!("render-target port")
}

pub fn set_cube_render_target<H: Host>(
    _host: &mut H,
    _texture: Option<H::Texture>,
    _side: i32,
    _depth: Option<H::Depth>,
) {
    todo!("cube render-target port")
}
