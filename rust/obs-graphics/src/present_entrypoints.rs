//! The frame and present calls from `present-entrypoints.c.h`:
//! `gs_begin_frame`, `gs_begin_scene`, `gs_draw`, `gs_end_scene`,
//! `gs_load_swapchain`, `gs_clear`, `gs_is_present_ready`, `gs_present` and
//! `gs_flush`.
//!
//! Each call checks for a graphics context first. Without one it writes the
//! context's debug line under its own name and returns (`false` for
//! `gs_is_present_ready`); with one it makes exactly one device call with its
//! arguments unchanged. The draw mode is not checked, the swapchain and the
//! clear colour are passed by identity (NULL included), and the clear depth
//! keeps its exact bits.

use std::ffi::CStr;

/// The debug line's reason, after the call's name.
pub const NOT_IN_CONTEXT: &str = "called while not in a graphics context";

/// The nine calls, for the name in their debug line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    BeginFrame,
    BeginScene,
    Draw,
    EndScene,
    LoadSwapchain,
    Clear,
    IsPresentReady,
    Present,
    Flush,
}

impl Entry {
    /// The exported C name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::BeginFrame => "gs_begin_frame",
            Self::BeginScene => "gs_begin_scene",
            Self::Draw => "gs_draw",
            Self::EndScene => "gs_end_scene",
            Self::LoadSwapchain => "gs_load_swapchain",
            Self::Clear => "gs_clear",
            Self::IsPresentReady => "gs_is_present_ready",
            Self::Present => "gs_present",
            Self::Flush => "gs_flush",
        }
    }

    /// The exported C name, terminated, for the C debug line.
    pub const fn c_name(self) -> &'static CStr {
        match self {
            Self::BeginFrame => c"gs_begin_frame",
            Self::BeginScene => c"gs_begin_scene",
            Self::Draw => c"gs_draw",
            Self::EndScene => c"gs_end_scene",
            Self::LoadSwapchain => c"gs_load_swapchain",
            Self::Clear => c"gs_clear",
            Self::IsPresentReady => c"gs_is_present_ready",
            Self::Present => c"gs_present",
            Self::Flush => c"gs_flush",
        }
    }
}

/// The current thread's graphics context and its device.
///
/// `has_context` is asked once per call, before anything else; the device
/// calls are made only when it said yes.
pub trait Host {
    /// A swapchain, passed by identity.
    type Swapchain: Copy;
    /// A clear colour, passed by identity.
    type Color: Copy;

    fn has_context(&self) -> bool;
    /// Write `"<entry>: called while not in a graphics context"` at debug level.
    fn debug(&mut self, entry: Entry);

    fn begin_frame(&mut self);
    fn begin_scene(&mut self);
    fn draw(&mut self, mode: i32, start_vert: u32, num_verts: u32);
    fn end_scene(&mut self);
    fn load_swapchain(&mut self, swapchain: Self::Swapchain);
    fn clear(&mut self, flags: u32, color: Self::Color, depth: f32, stencil: u8);
    fn is_present_ready(&mut self) -> bool;
    fn present(&mut self);
    fn flush(&mut self);
}

/// The context check: without a context, the debug line and no device call.
fn valid(host: &mut impl Host, entry: Entry) -> bool {
    if host.has_context() {
        return true;
    }
    host.debug(entry);
    false
}

pub fn begin_frame(host: &mut impl Host) {
    if valid(host, Entry::BeginFrame) {
        host.begin_frame();
    }
}

pub fn begin_scene(host: &mut impl Host) {
    if valid(host, Entry::BeginScene) {
        host.begin_scene();
    }
}

pub fn draw(host: &mut impl Host, mode: i32, start_vert: u32, num_verts: u32) {
    if valid(host, Entry::Draw) {
        host.draw(mode, start_vert, num_verts);
    }
}

pub fn end_scene(host: &mut impl Host) {
    if valid(host, Entry::EndScene) {
        host.end_scene();
    }
}

pub fn load_swapchain<H: Host>(host: &mut H, swapchain: H::Swapchain) {
    if valid(host, Entry::LoadSwapchain) {
        host.load_swapchain(swapchain);
    }
}

pub fn clear<H: Host>(host: &mut H, flags: u32, color: H::Color, depth: f32, stencil: u8) {
    if valid(host, Entry::Clear) {
        host.clear(flags, color, depth, stencil);
    }
}

/// `false` without a context; otherwise the device's answer.
pub fn is_present_ready(host: &mut impl Host) -> bool {
    valid(host, Entry::IsPresentReady) && host.is_present_ready()
}

pub fn present(host: &mut impl Host) {
    if valid(host, Entry::Present) {
        host.present();
    }
}

pub fn flush(host: &mut impl Host) {
    if valid(host, Entry::Flush) {
        host.flush();
    }
}
