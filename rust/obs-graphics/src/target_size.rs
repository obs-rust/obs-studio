//! Target-size dispatch independent of native graphics layouts.
use std::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry { Size, Width, Height }

impl Entry {
    pub const fn name(self) -> &'static str {
        match self { Self::Size => "gs_get_size", Self::Width => "gs_get_width", Self::Height => "gs_get_height" }
    }
    pub const fn c_name(self) -> &'static CStr {
        match self { Self::Size => c"gs_get_size", Self::Width => c"gs_get_width", Self::Height => c"gs_get_height" }
    }
}

/// Outputs are opaque handles: nullable and aliased handles reach the backend unchanged.
pub trait Host {
    type Output: Copy;
    fn has_context(&self) -> bool;
    fn debug(&mut self, entry: Entry);
    fn size(&mut self, x: Self::Output, y: Self::Output);
    fn width(&mut self) -> u32;
    fn height(&mut self) -> u32;
}

pub fn get_size<H: Host>(_host: &mut H, _x: H::Output, _y: H::Output) { todo!() }
pub fn get_width<H: Host>(_host: &mut H) -> u32 { todo!() }
pub fn get_height<H: Host>(_host: &mut H) -> u32 { todo!() }
