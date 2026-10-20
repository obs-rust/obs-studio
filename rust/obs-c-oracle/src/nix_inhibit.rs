//! Oracle entry points for `libobs/util/platform-nix-inhibit.c` (Linux and
//! FreeBSD): the original built without GIO (`oracle_os_inhibit_sleep_*`)
//! and with it (`oracle_gio_os_inhibit_sleep_*`), and the counted stand-ins
//! in `oracle/nix_inhibit_stubs_posix.c` for what it calls outside itself.

use core::ffi::{c_char, c_int, c_ulong, c_void};

unsafe extern "C" {
    pub fn oracle_os_inhibit_sleep_create(reason: *const c_char) -> *mut c_void;
    pub fn oracle_os_inhibit_sleep_set_active(info: *mut c_void, active: bool) -> bool;
    pub fn oracle_os_inhibit_sleep_destroy(info: *mut c_void);

    pub fn oracle_gio_os_inhibit_sleep_create(reason: *const c_char) -> *mut c_void;
    pub fn oracle_gio_os_inhibit_sleep_set_active(info: *mut c_void, active: bool) -> bool;
    pub fn oracle_gio_os_inhibit_sleep_destroy(info: *mut c_void);

    /// Stand-ins for `util/platform-nix-portal.c` and `util/platform-nix-dbus.c`,
    /// under their real names.
    pub fn portal_inhibit_info_create() -> *mut c_void;
    pub fn portal_inhibit(portal: *mut c_void, reason: *const c_char, active: bool);
    pub fn portal_inhibit_info_destroy(portal: *mut c_void);
    pub fn dbus_sleep_info_create() -> *mut c_void;
    pub fn dbus_inhibit_sleep(dbus: *mut c_void, sleep: *const c_char, active: bool);
    pub fn dbus_sleep_info_destroy(dbus: *mut c_void);

    /// `access(path, mode)` as the oracle sees it.
    pub fn oracle_stub_inhibit_access(path: *const c_char, mode: c_int) -> c_int;

    /// Sets what the desktop has and the errno a spawn returns (0 is
    /// `ENOENT`), and clears the log.
    pub fn oracle_inhibit_stub_reset(
        portal_ok: c_int,
        dbus_ok: c_int,
        flatpak: c_int,
        spawn_error: c_int,
    );
    /// Waits that time out before the next reset thread parks.
    pub fn oracle_inhibit_stub_set_timeouts(timeouts: c_int);
    /// Copies the log (one call per line) into `out` and clears it; returns
    /// its full length.
    pub fn oracle_inhibit_stub_take_log(out: *mut c_char, cap: usize) -> usize;
    /// 1 once `threads` reset threads are parked in their wait, 0 after 10 s.
    pub fn oracle_inhibit_stub_await_parked(threads: c_int) -> c_int;
    /// The reset thread's wait, for a test host: 1 when it times out (logged),
    /// else 0 and the thread counts as parked until `wait_leave`.
    pub fn oracle_inhibit_stub_wait_enter(milliseconds: c_ulong) -> c_int;
    pub fn oracle_inhibit_stub_wait_leave();
    /// A spawn as the oracle's `posix_spawnp` logs it; returns the errno.
    pub fn oracle_inhibit_stub_spawn(
        file: *const c_char,
        argv0: *const c_char,
        argv1: *const c_char,
    ) -> c_int;
    /// A `blog` line as the oracle logs it.
    pub fn oracle_inhibit_stub_log_line(level: c_int, message: *const c_char);
}
