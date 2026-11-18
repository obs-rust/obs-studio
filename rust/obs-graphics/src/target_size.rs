//! Target-size dispatch independent of native graphics layouts.
use std::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Size,
    Width,
    Height,
}

impl Entry {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Size => "gs_get_size",
            Self::Width => "gs_get_width",
            Self::Height => "gs_get_height",
        }
    }
    pub const fn c_name(self) -> &'static CStr {
        match self {
            Self::Size => c"gs_get_size",
            Self::Width => c"gs_get_width",
            Self::Height => c"gs_get_height",
        }
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

pub fn get_size<H: Host>(host: &mut H, x: H::Output, y: H::Output) {
    if !host.has_context() {
        host.debug(Entry::Size);
        return;
    }
    host.size(x, y);
}
pub fn get_width<H: Host>(host: &mut H) -> u32 {
    if !host.has_context() {
        host.debug(Entry::Width);
        return 0;
    }
    host.width()
}
pub fn get_height<H: Host>(host: &mut H) -> u32 {
    if !host.has_context() {
        host.debug(Entry::Height);
        return 0;
    }
    host.height()
}
