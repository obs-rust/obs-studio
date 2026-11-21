//! Public Linux/BSD platform state. There is deliberately no synchronization.
//! The application must serialize setters against every other access, as in C.
#![cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]

// Keep the Unix export marker discoverable by the ABI inventory; the file's
// host boundary above also excludes macOS, where these APIs do not exist.
#[cfg(unix)]
mod unix {
    use crate::platform::nix_platform::{PlatformState, checked_platform};
    use core::ffi::{c_uint, c_void};

    static mut STATE: PlatformState = PlatformState::new();

    /// Select a platform; INVALID asserts only in debug builds.
    ///
    /// # Safety
    /// The caller must serialize writes and reads of the selected platform.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn obs_set_nix_platform(platform: c_uint) {
        // SAFETY: the caller excludes conflicting accesses to this field. Raw
        // field access avoids borrowing the independently accessed display.
        unsafe { (*(&raw mut STATE)).platform = checked_platform(platform) };
    }

    /// Read the selected platform.
    ///
    /// # Safety
    /// The caller must exclude concurrent writes to the selected platform.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn obs_get_nix_platform() -> c_uint {
        // SAFETY: the caller excludes concurrent mutation of the process state.
        unsafe { (*(&raw const STATE)).platform }
    }

    /// Store a borrowed display pointer, including NULL, without dereferencing it.
    ///
    /// # Safety
    /// The caller must serialize writes and reads of the display pointer.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn obs_set_nix_platform_display(display: *mut c_void) {
        // SAFETY: exclusive state access is the caller's responsibility. The
        // pointer is stored unchanged; this API never acquires ownership.
        unsafe { (*(&raw mut STATE)).display = display };
    }

    /// Return the exact borrowed display pointer last stored, initially NULL.
    ///
    /// # Safety
    /// The caller must exclude concurrent writes to the display pointer.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn obs_get_nix_platform_display() -> *mut c_void {
        // SAFETY: the caller excludes concurrent mutation of the process state.
        unsafe { (*(&raw const STATE)).display }
    }
}

#[cfg(unix)]
pub use unix::*;
