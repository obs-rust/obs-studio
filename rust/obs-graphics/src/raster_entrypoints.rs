//! The cull and colour calls from `raster-state-entrypoints.c.h`:
//! `gs_set_cull_mode`, `gs_get_cull_mode` and `gs_enable_color`.
//!
//! Each call checks for a graphics context first. Without one it writes the
//! context's debug line under its own name and returns (`GS_NEITHER` for
//! `gs_get_cull_mode`); with one it makes exactly one device call with its
//! arguments unchanged. The cull mode is not checked either way, and
//! `gs_get_cull_mode` returns the device's answer, never a stored one.

use std::ffi::CStr;

/// The debug line's reason, after the call's name.
pub const NOT_IN_CONTEXT: &str = "called while not in a graphics context";

/// `GS_NEITHER`, `gs_get_cull_mode`'s answer without a context.
pub const GS_NEITHER: i32 = 2;

/// The three calls, for the name in their debug line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    SetCullMode,
    GetCullMode,
    EnableColor,
}

impl Entry {
    /// The exported C name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::SetCullMode => "gs_set_cull_mode",
            Self::GetCullMode => "gs_get_cull_mode",
            Self::EnableColor => "gs_enable_color",
        }
    }

    /// The exported C name, terminated, for the C debug line.
    pub const fn c_name(self) -> &'static CStr {
        match self {
            Self::SetCullMode => c"gs_set_cull_mode",
            Self::GetCullMode => c"gs_get_cull_mode",
            Self::EnableColor => c"gs_enable_color",
        }
    }
}

/// The current thread's graphics context and its device.
///
/// `has_context` is asked once per call, before anything else; the device
/// calls are made only when it said yes.
pub trait Host {
    fn has_context(&self) -> bool;
    /// Write `"<entry>: called while not in a graphics context"` at debug level.
    fn debug(&mut self, entry: Entry);

    fn set_cull_mode(&mut self, mode: i32);
    fn get_cull_mode(&mut self) -> i32;
    fn enable_color(&mut self, red: bool, green: bool, blue: bool, alpha: bool);
}

/// The context check: without a context, the debug line and no device call.
fn valid(host: &mut impl Host, entry: Entry) -> bool {
    if host.has_context() {
        return true;
    }
    host.debug(entry);
    false
}

pub fn set_cull_mode(host: &mut impl Host, mode: i32) {
    if valid(host, Entry::SetCullMode) {
        host.set_cull_mode(mode);
    }
}

/// `GS_NEITHER` without a context; otherwise the device's answer.
pub fn get_cull_mode(host: &mut impl Host) -> i32 {
    if !valid(host, Entry::GetCullMode) {
        return GS_NEITHER;
    }
    host.get_cull_mode()
}

pub fn enable_color(host: &mut impl Host, red: bool, green: bool, blue: bool, alpha: bool) {
    if valid(host, Entry::EnableColor) {
        host.enable_color(red, green, blue, alpha);
    }
}
