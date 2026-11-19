//! Shader matrix-parameter query decisions without graphics layout knowledge.
use std::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    ViewProj,
    World,
}

impl Entry {
    pub const fn name(self) -> &'static str {
        match self {
            Self::ViewProj => "gs_shader_get_viewproj_matrix",
            Self::World => "gs_shader_get_world_matrix",
        }
    }
    pub const fn c_name(self) -> &'static CStr {
        match self {
            Self::ViewProj => c"gs_shader_get_viewproj_matrix",
            Self::World => c"gs_shader_get_world_matrix",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    Context,
    Shader,
}

impl Rejection {
    pub const fn message(self) -> &'static CStr {
        match self {
            Self::Context => c"called while not in a graphics context",
            Self::Shader => c"Null 'shader' parameter",
        }
    }
}

/// Backend queries run only after the context and shader guards succeed.
/// Results are borrowed backend handles, returned unchanged (including none).
pub trait Host {
    type Shader: Copy;
    type Param: Copy;
    fn has_context(&self) -> bool;
    fn debug(&mut self, entry: Entry, reason: Rejection);
    fn viewproj(&mut self, shader: Self::Shader) -> Option<Self::Param>;
    fn world(&mut self, shader: Self::Shader) -> Option<Self::Param>;
}

pub fn get_viewproj_matrix<H: Host>(host: &mut H, shader: Option<H::Shader>) -> Option<H::Param> {
    let _ = (host, shader);
    todo!("gs_shader_get_viewproj_matrix")
}

pub fn get_world_matrix<H: Host>(host: &mut H, shader: Option<H::Shader>) -> Option<H::Param> {
    let _ = (host, shader);
    todo!("gs_shader_get_world_matrix")
}
