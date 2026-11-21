//! Unsynchronized Linux/BSD display selection from `obs-nix-platform.c.h`.

use core::ffi::{c_uint, c_void};

/// State owned by the application; the display connection is always borrowed.
/// Access to the process-wide instance must be externally serialized, as in C.
#[derive(Debug)]
pub struct PlatformState {
    pub(crate) platform: c_uint,
    pub(crate) display: *mut c_void,
}

impl Default for PlatformState {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformState {
    /// The C defaults are X11_EGL (1) and a NULL display.
    pub const fn new() -> Self {
        Self {
            platform: 1,
            display: core::ptr::null_mut(),
        }
    }

    pub fn platform(&self) -> c_uint {
        self.platform
    }

    pub fn set_platform(&mut self, platform: c_uint) {
        self.platform = checked_platform(platform);
    }

    pub fn display(&self) -> *mut c_void {
        self.display
    }

    pub fn set_display(&mut self, display: *mut c_void) {
        self.display = display;
    }
}

/// Preserve C's debug-only INVALID assertion; other bit patterns pass through.
pub(crate) fn checked_platform(platform: c_uint) -> c_uint {
    debug_assert_ne!(platform, 0);
    platform
}
