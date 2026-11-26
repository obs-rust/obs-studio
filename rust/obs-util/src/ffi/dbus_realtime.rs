//! GIO calls for real-time thread priority (obs-rust/obs-studio#5770):
//! reading a property of the Realtime portal or rtkit, and the two
//! `...WithPID` requests they share. The signatures follow PipeWire's
//! `module-rt`: `MakeThreadRealtimeWithPID(t, t, u)` and
//! `MakeThreadHighPriorityWithPID(t, t, i)`; the limits are the
//! `MaxRealtimePriority` (i), `MinNiceLevel` (i) and `RTTimeUSecMax` (x)
//! properties.
//!
//! Every call has a short timeout, and a bus whose socket is missing is not
//! connected at all: without a session bus, GIO would otherwise try to
//! autolaunch one.

use core::ffi::{CStr, c_char, c_int, c_void};
use core::ptr::{self, NonNull};

#[repr(C)]
struct GError {
    domain: u32,
    code: c_int,
    message: *mut c_char,
}

unsafe extern "C" {
    fn g_bus_get_sync(
        bus_type: c_int,
        cancellable: *mut c_void,
        error: *mut *mut GError,
    ) -> *mut c_void;
    fn g_dbus_connection_call_sync(
        connection: *mut c_void,
        destination: *const c_char,
        object_path: *const c_char,
        interface_name: *const c_char,
        method_name: *const c_char,
        parameters: *mut c_void,
        reply_type: *const c_void,
        flags: c_int,
        timeout_msec: c_int,
        cancellable: *mut c_void,
        error: *mut *mut GError,
    ) -> *mut c_void;
    fn g_variant_new(format: *const c_char, ...) -> *mut c_void;
    fn g_variant_get(value: *mut c_void, format: *const c_char, ...);
    fn g_variant_get_type_string(value: *mut c_void) -> *const c_char;
    fn g_variant_get_int32(value: *mut c_void) -> i32;
    fn g_variant_get_int64(value: *mut c_void) -> i64;
    fn g_variant_get_uint32(value: *mut c_void) -> u32;
    fn g_variant_unref(value: *mut c_void);
    fn g_object_unref(object: *mut c_void);
    fn g_error_free(error: *mut GError);
}

/// The timeout of every call, in milliseconds.
pub const TIMEOUT_MS: c_int = 500;

/// A message bus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bus {
    Session,
    System,
}

/// A service object: bus, well-known name, object path and interface.
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub bus: Bus,
    pub name: &'static CStr,
    pub path: &'static CStr,
    pub interface: &'static CStr,
}

/// `org.freedesktop.portal.Realtime` on the session bus.
pub const PORTAL: Target = Target {
    bus: Bus::Session,
    name: c"org.freedesktop.portal.Desktop",
    path: c"/org/freedesktop/portal/desktop",
    interface: c"org.freedesktop.portal.Realtime",
};

/// `org.freedesktop.RealtimeKit1` on the system bus.
pub const RTKIT: Target = Target {
    bus: Bus::System,
    name: c"org.freedesktop.RealtimeKit1",
    path: c"/org/freedesktop/RealtimeKit1",
    interface: c"org.freedesktop.RealtimeKit1",
};

/// The connection reference `g_bus_get_sync` returns (GIO shares one
/// connection per bus in the process and keeps it alive).
struct Connection(NonNull<c_void>);

impl Drop for Connection {
    fn drop(&mut self) {
        // SAFETY: `self.0` came from `g_bus_get_sync` and owns one reference.
        unsafe { g_object_unref(self.0.as_ptr()) };
    }
}

/// Reads and frees the GError of one failed GIO call.
unsafe fn take_error(error: *mut GError) -> String {
    if error.is_null() {
        return "unknown D-Bus error".into();
    }
    // SAFETY: GIO returns a live GError with a NUL-terminated message.
    let message = unsafe { CStr::from_ptr((*error).message) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: the GError is ours after the call.
    unsafe { g_error_free(error) };
    message
}

/// True when the bus can be reached without autolaunching one.
fn bus_present(bus: Bus) -> bool {
    match bus {
        Bus::System => {
            std::env::var_os("DBUS_SYSTEM_BUS_ADDRESS").is_some()
                || std::path::Path::new("/run/dbus/system_bus_socket").exists()
                || std::path::Path::new("/var/run/dbus/system_bus_socket").exists()
        }
        Bus::Session => {
            std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some()
                || std::env::var_os("XDG_RUNTIME_DIR")
                    .is_some_and(|dir| std::path::Path::new(&dir).join("bus").exists())
        }
    }
}

fn connect(bus: Bus) -> Result<Connection, String> {
    if !bus_present(bus) {
        return Err(match bus {
            Bus::System => "no system bus".into(),
            Bus::Session => "no session bus".into(),
        });
    }
    let bus_type = match bus {
        Bus::System => 1,  // G_BUS_TYPE_SYSTEM
        Bus::Session => 2, // G_BUS_TYPE_SESSION
    };
    let mut error = ptr::null_mut();
    // SAFETY: out-pointer and NULL cancellable follow the GIO API.
    let connection = unsafe { g_bus_get_sync(bus_type, ptr::null_mut(), &raw mut error) };
    match NonNull::new(connection) {
        Some(connection) => Ok(Connection(connection)),
        // SAFETY: GIO owns the error returned on failure.
        None => Err(unsafe { take_error(error) }),
    }
}

/// One synchronous call on a connected bus; GIO consumes the floating
/// `params`. The reply is owned by the caller.
fn call(
    connection: &Connection,
    target: &Target,
    interface: &CStr,
    method: &CStr,
    params: *mut c_void,
) -> Result<NonNull<c_void>, String> {
    let mut error = ptr::null_mut();
    // SAFETY: the connection, the static names and `params` are live.
    let reply = unsafe {
        g_dbus_connection_call_sync(
            connection.0.as_ptr(),
            target.name.as_ptr(),
            target.path.as_ptr(),
            interface.as_ptr(),
            method.as_ptr(),
            params,
            ptr::null(),
            0,
            TIMEOUT_MS,
            ptr::null_mut(),
            &raw mut error,
        )
    };
    match NonNull::new(reply) {
        Some(reply) => Ok(reply),
        // SAFETY: GIO returned this error to us.
        None => Err(unsafe { take_error(error) }),
    }
}

/// An integer property (`i`, `u` or `x`) of `target`.
pub fn property(target: &Target, name: &CStr) -> Result<i64, String> {
    let connection = connect(target.bus)?;
    // SAFETY: two NUL-terminated strings match `(ss)`.
    let params =
        unsafe { g_variant_new(c"(ss)".as_ptr(), target.interface.as_ptr(), name.as_ptr()) };
    let reply = call(
        &connection,
        target,
        c"org.freedesktop.DBus.Properties",
        c"Get",
        params,
    )?;
    let mut inner: *mut c_void = ptr::null_mut();
    // SAFETY: `Get` returns `(v)`; `inner` receives one owned reference.
    unsafe {
        g_variant_get(reply.as_ptr(), c"(v)".as_ptr(), &raw mut inner);
        g_variant_unref(reply.as_ptr());
    }
    if inner.is_null() {
        return Err(format!("{}: empty reply", name.to_string_lossy()));
    }
    // SAFETY: `inner` is a live GVariant; its type string is static.
    let kind = unsafe { CStr::from_ptr(g_variant_get_type_string(inner)) };
    // SAFETY: each getter matches the checked type.
    let value = unsafe {
        match kind.to_bytes() {
            b"i" => Ok(i64::from(g_variant_get_int32(inner))),
            b"u" => Ok(i64::from(g_variant_get_uint32(inner))),
            b"x" => Ok(g_variant_get_int64(inner)),
            other => Err(format!(
                "{}: unexpected type {}",
                name.to_string_lossy(),
                String::from_utf8_lossy(other)
            )),
        }
    };
    // SAFETY: the reference from `g_variant_get` is ours.
    unsafe { g_variant_unref(inner) };
    value
}

/// `MakeThreadRealtimeWithPID(pid, tid, priority)` on `target`.
pub fn make_thread_realtime(
    target: &Target,
    pid: u64,
    tid: u64,
    priority: u32,
) -> Result<(), String> {
    let connection = connect(target.bus)?;
    // SAFETY: two guint64 and one guint32 match `(ttu)`.
    let params = unsafe { g_variant_new(c"(ttu)".as_ptr(), pid, tid, priority) };
    let reply = call(
        &connection,
        target,
        target.interface,
        c"MakeThreadRealtimeWithPID",
        params,
    )?;
    // SAFETY: the reply is ours.
    unsafe { g_variant_unref(reply.as_ptr()) };
    Ok(())
}

/// `MakeThreadHighPriorityWithPID(pid, tid, nice)` on `target`.
pub fn make_thread_high_priority(
    target: &Target,
    pid: u64,
    tid: u64,
    nice: i32,
) -> Result<(), String> {
    let connection = connect(target.bus)?;
    // SAFETY: two guint64 and one gint32 match `(tti)`.
    let params = unsafe { g_variant_new(c"(tti)".as_ptr(), pid, tid, nice) };
    let reply = call(
        &connection,
        target,
        target.interface,
        c"MakeThreadHighPriorityWithPID",
        params,
    )?;
    // SAFETY: the reply is ours.
    unsafe { g_variant_unref(reply.as_ptr()) };
    Ok(())
}
