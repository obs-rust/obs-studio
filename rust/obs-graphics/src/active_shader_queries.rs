//! Borrowed active shader queries over a caller-owned graphics context.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Vertex,
    Pixel,
}

impl Entry {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Vertex => "gs_get_vertex_shader",
            Self::Pixel => "gs_get_pixel_shader",
        }
    }
}

/// Dispatch projects the context only after the core has checked its presence.
/// Shader values are borrowed identities; their default represents NULL.
pub trait Host {
    type Shader: Default;
    fn has_context(&self) -> bool;
    fn debug(&mut self, entry: Entry);
    fn dispatch(&mut self, entry: Entry) -> Self::Shader;
}

pub fn shader<H: Host>(_host: &mut H, _entry: Entry) -> H::Shader {
    todo!("active shader queries")
}
