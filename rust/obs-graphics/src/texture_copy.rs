//! Texture-copy decisions without graphics layout knowledge.
use std::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Whole,
    Region,
}

impl Entry {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Whole => "gs_copy_texture",
            Self::Region => "gs_copy_texture_region",
        }
    }
    pub const fn c_name(self) -> &'static CStr {
        match self {
            Self::Whole => c"gs_copy_texture",
            Self::Region => c"gs_copy_texture_region",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    Context,
    Destination,
    Source,
}

impl Rejection {
    pub const fn message(self) -> &'static CStr {
        match self {
            Self::Context => c"called while not in a graphics context",
            Self::Destination => c"Null 'dst' parameter",
            Self::Source => c"Null 'src' parameter",
        }
    }
}

/// Backend methods run only after the corresponding Rust guards succeed.
pub trait Host {
    type Texture: Copy;
    fn has_context(&self) -> bool;
    fn debug(&mut self, entry: Entry, reason: Rejection);
    fn whole(&mut self, dst: Self::Texture, src: Self::Texture);
    /// Values are dst_x, dst_y, src_x, src_y, src_w, src_h, unchanged.
    fn region(&mut self, dst: Self::Texture, src: Option<Self::Texture>, values: [u32; 6]);
}

pub fn copy_texture<H: Host>(_host: &mut H, _dst: Option<H::Texture>, _src: Option<H::Texture>) {
    todo!("#5062: texture-copy Rust swap")
}

pub fn copy_texture_region<H: Host>(
    _host: &mut H,
    _dst: Option<H::Texture>,
    _src: Option<H::Texture>,
    _values: [u32; 6],
) {
    todo!("#5062: texture-copy Rust swap")
}
