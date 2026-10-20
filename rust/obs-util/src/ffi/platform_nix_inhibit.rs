//! C ABI shim for `libobs/util/platform-nix-inhibit.c` (Linux and FreeBSD).
//! The core is [`crate::platform::unix::nix_inhibit`]; macOS has its own in
//! `platform_cocoa`, Windows in `platform_windows`.
//!
//! The portal and session D-Bus inhibitors stay C
//! (`util/platform-nix-portal.c`, `util/platform-nix-dbus.c`, unity group
//! `libobs.nix-dbus`). With the `gio` feature, which CMake sets when GIO is
//! found (`GIO_FOUND`), [`CHost`] calls them; without it libobs has no such
//! symbols, and the core only uses the xdg-screensaver fallback, as the C
//! compiled without GIO does.

use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::{self, NonNull};
use std::ffi::CString;
use std::sync::Arc;

use crate::platform::unix::nix_inhibit::{self, Host, Inhibit};

unsafe extern "C" {
    fn blog(log_level: c_int, format: *const c_char, ...);
}

/// The C inhibitors. Without GIO libobs has no such symbols; the core never
/// calls these then (`CHost::GIO` is false), and they do nothing.
#[cfg(feature = "gio")]
mod gio {
    use core::ffi::{c_char, c_void};

    unsafe extern "C" {
        pub fn portal_inhibit_info_create() -> *mut c_void;
        pub fn portal_inhibit(portal: *mut c_void, reason: *const c_char, active: bool);
        pub fn portal_inhibit_info_destroy(portal: *mut c_void);
        pub fn dbus_sleep_info_create() -> *mut c_void;
        pub fn dbus_inhibit_sleep(dbus: *mut c_void, sleep: *const c_char, active: bool);
        pub fn dbus_sleep_info_destroy(dbus: *mut c_void);
    }
}

#[cfg(not(feature = "gio"))]
mod gio {
    use core::ffi::{c_char, c_void};
    use core::ptr;

    pub unsafe fn portal_inhibit_info_create() -> *mut c_void {
        ptr::null_mut()
    }
    pub unsafe fn portal_inhibit(_: *mut c_void, _: *const c_char, _: bool) {}
    pub unsafe fn portal_inhibit_info_destroy(_: *mut c_void) {}
    pub unsafe fn dbus_sleep_info_create() -> *mut c_void {
        ptr::null_mut()
    }
    pub unsafe fn dbus_inhibit_sleep(_: *mut c_void, _: *const c_char, _: bool) {}
    pub unsafe fn dbus_sleep_info_destroy(_: *mut c_void) {}
}

/// A `struct portal_inhibit_info *` or `struct dbus_sleep_info *` from the C.
pub struct Handle(NonNull<c_void>);

fn reason_ptr(reason: Option<&CStr>) -> *const c_char {
    reason.map_or(ptr::null(), CStr::as_ptr)
}

/// The libobs host: the C inhibitors and `blog`.
pub struct CHost;

impl Host for CHost {
    const GIO: bool = cfg!(feature = "gio");
    type Portal = Handle;
    type Dbus = Handle;

    fn portal_create(&self) -> Option<Handle> {
        // SAFETY: no arguments; returns an owned handle or NULL.
        NonNull::new(unsafe { gio::portal_inhibit_info_create() }).map(Handle)
    }

    fn dbus_create(&self) -> Option<Handle> {
        // SAFETY: no arguments; returns an owned handle or NULL.
        NonNull::new(unsafe { gio::dbus_sleep_info_create() }).map(Handle)
    }

    fn portal_inhibit(&self, portal: &mut Handle, reason: Option<&CStr>, active: bool) {
        // SAFETY: `portal` came from `portal_inhibit_info_create` and is not
        // destroyed yet; `reason` is NULL or a C string that outlives the call.
        unsafe { gio::portal_inhibit(portal.0.as_ptr(), reason_ptr(reason), active) };
    }

    fn dbus_inhibit(&self, dbus: &mut Handle, reason: Option<&CStr>, active: bool) {
        // SAFETY: as in `portal_inhibit`, for a `dbus_sleep_info_create` handle.
        unsafe { gio::dbus_inhibit_sleep(dbus.0.as_ptr(), reason_ptr(reason), active) };
    }

    fn portal_destroy(&self, portal: Handle) {
        // SAFETY: the handle is owned and given back here, once.
        unsafe { gio::portal_inhibit_info_destroy(portal.0.as_ptr()) };
    }

    fn dbus_destroy(&self, dbus: Handle) {
        // SAFETY: the handle is owned and given back here, once.
        unsafe { gio::dbus_sleep_info_destroy(dbus.0.as_ptr()) };
    }

    fn log(&self, level: i32, message: &str) {
        let message = CString::new(message).unwrap_or_default();
        // SAFETY: "%s" with one C string argument.
        unsafe { blog(level, c"%s".as_ptr(), message.as_ptr()) };
    }
}

/// `os_inhibit_sleep_create`.
///
/// # Safety
/// `reason` is NULL or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_inhibit_sleep_create(reason: *const c_char) -> *mut c_void {
    // SAFETY: the caller passes NULL or a C string.
    let reason = (!reason.is_null()).then(|| unsafe { CStr::from_ptr(reason) });
    match Inhibit::create(Arc::new(CHost), reason) {
        Some(info) => Box::into_raw(Box::new(info)).cast(),
        None => ptr::null_mut(),
    }
}

/// `os_inhibit_sleep_set_active`.
///
/// # Safety
/// `info` is NULL or came from `os_inhibit_sleep_create` and is not
/// destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_inhibit_sleep_set_active(info: *mut c_void, active: bool) -> bool {
    // SAFETY: NULL or a live `Inhibit<CHost>` from `Box::into_raw`.
    nix_inhibit::set_active(unsafe { info.cast::<Inhibit<CHost>>().as_mut() }, active)
}

/// `os_inhibit_sleep_destroy`.
///
/// # Safety
/// As `os_inhibit_sleep_set_active`; `info` is not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn os_inhibit_sleep_destroy(info: *mut c_void) {
    if info.is_null() {
        return;
    }
    // SAFETY: a live `Inhibit<CHost>` from `Box::into_raw`, taken back once.
    let info = unsafe { Box::from_raw(info.cast::<Inhibit<CHost>>()) };
    nix_inhibit::destroy(Some(*info));
}
