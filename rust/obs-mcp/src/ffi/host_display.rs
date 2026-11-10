//! Display ownership for a self-hosted libobs; attached GUI hosts do not use this.

#[cfg(all(unix, not(target_os = "macos")))]
mod unix {
    use std::ffi::c_void;

    const X11: i32 = 1;
    const WAYLAND: i32 = 2;
    type Connect = unsafe extern "C" fn(*const std::ffi::c_char) -> *mut c_void;
    type Disconnect = unsafe extern "C" fn(*mut c_void);

    unsafe extern "C" {
        fn obs_set_nix_platform(platform: i32);
        fn obs_get_nix_platform() -> i32;
        fn obs_set_nix_platform_display(display: *mut c_void);
        fn obs_get_nix_platform_display() -> *mut c_void;
    }

    pub struct Display {
        connection: *mut c_void,
        disconnect: Option<Disconnect>,
    }

    impl Display {
        pub fn open() -> Self {
            // SAFETY: resolve optional functions from libobs's already loaded
            // display libraries. Each non-null symbol has its documented C ABI.
            unsafe {
                let connect = libc::dlsym(libc::RTLD_DEFAULT, c"wl_display_connect".as_ptr());
                let disconnect = libc::dlsym(libc::RTLD_DEFAULT, c"wl_display_disconnect".as_ptr());
                if !connect.is_null() && !disconnect.is_null() {
                    let connect: Connect = std::mem::transmute(connect);
                    let disconnect: Disconnect = std::mem::transmute(disconnect);
                    let connection = connect(std::ptr::null());
                    if !connection.is_null() {
                        obs_set_nix_platform(WAYLAND);
                        obs_set_nix_platform_display(connection);
                        return Self {
                            connection,
                            disconnect: Some(disconnect),
                        };
                    }
                }
                // Preserve the existing X11 video path when an X server really
                // is available; a stale DISPLAY must not make startup fail.
                let open = libc::dlsym(libc::RTLD_DEFAULT, c"XOpenDisplay".as_ptr());
                let close = libc::dlsym(libc::RTLD_DEFAULT, c"XCloseDisplay".as_ptr());
                if !open.is_null() && !close.is_null() {
                    let open: Connect = std::mem::transmute(open);
                    let close: unsafe extern "C" fn(*mut c_void) -> i32 =
                        std::mem::transmute(close);
                    let connection = open(std::ptr::null());
                    if !connection.is_null() {
                        close(connection);
                        obs_set_nix_platform(X11);
                        return Self {
                            connection: std::ptr::null_mut(),
                            disconnect: None,
                        };
                    }
                }
                obs_set_nix_platform(WAYLAND);
                obs_set_nix_platform_display(std::ptr::null_mut());
                Self {
                    connection: std::ptr::null_mut(),
                    disconnect: None,
                }
            }
        }
    }

    impl Drop for Display {
        fn drop(&mut self) {
            // SAFETY: Libobs drops this lease after obs_shutdown, and this is
            // the sole owner of the connection created by wl_display_connect.
            unsafe {
                obs_set_nix_platform_display(std::ptr::null_mut());
                if let Some(disconnect) = self.disconnect {
                    disconnect(self.connection);
                }
            }
        }
    }

    pub fn require_video_display() -> Result<(), String> {
        // SAFETY: called after startup while the display lease is alive.
        if unsafe { obs_get_nix_platform() == WAYLAND && obs_get_nix_platform_display().is_null() }
        {
            Err("video is unavailable: no reachable X11 or Wayland display".to_owned())
        } else {
            Ok(())
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
pub use unix::{Display, require_video_display};

#[cfg(not(all(unix, not(target_os = "macos"))))]
pub struct Display;

#[cfg(not(all(unix, not(target_os = "macos"))))]
impl Display {
    pub fn open() -> Self {
        Self
    }
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
pub fn require_video_display() -> Result<(), String> {
    Ok(())
}
