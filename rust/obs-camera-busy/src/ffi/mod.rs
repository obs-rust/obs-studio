//! C ABI for the frontend (#3724), declared in
//! `frontend/utility/obs-camera-busy.h`. It converts types and calls the
//! safe core; there is no logic here. The warnings state is one per process
//! and may be used from any thread.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::{Allowlist, Answer, Cause, Closed, Holder, ProcessSignals, Show, Warnings, Watcher};

/// [`Show`] at the C boundary.
pub const OBS_CAMERA_BUSY_DIALOG: c_int = 0;
pub const OBS_CAMERA_BUSY_ALLOWED: c_int = 1;
pub const OBS_CAMERA_BUSY_DUPLICATE: c_int = 2;
pub const OBS_CAMERA_BUSY_RATE_LIMITED: c_int = 3;
pub const OBS_CAMERA_BUSY_NO_HOLDER: c_int = 4;

/// [`Cause`] at the C boundary; anything else is [`Cause::Busy`].
pub const OBS_CAMERA_BUSY_CAUSE_BUSY: c_int = 0;
pub const OBS_CAMERA_BUSY_CAUSE_OPENED: c_int = 1;

/// [`Answer`] at the C boundary; anything else is [`Answer::Ignore`].
pub const OBS_CAMERA_BUSY_CLOSE_IT: c_int = 0;
pub const OBS_CAMERA_BUSY_IGNORE: c_int = 1;
pub const OBS_CAMERA_BUSY_ALWAYS_ALLOW: c_int = 2;

/// [`Closed`] at the C boundary.
pub const OBS_CAMERA_BUSY_EXITED: c_int = 0;
pub const OBS_CAMERA_BUSY_KILLED: c_int = 1;
pub const OBS_CAMERA_BUSY_FAILED: c_int = 2;

static WARNINGS: Mutex<Option<Warnings>> = Mutex::new(None);

fn with_warnings<T>(f: impl FnOnce(&mut Warnings) -> T) -> T {
    let mut guard = WARNINGS.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Warnings::default))
}

/// A list of holders (`obs_camera_busy_holders_t`), with C copies of the
/// names.
pub struct Holders {
    list: Vec<Holder>,
    names: Vec<CString>,
}

impl Holders {
    fn new(list: Vec<Holder>) -> *mut Holders {
        let names = list
            .iter()
            .map(|h| CString::new(h.name.replace('\0', " ")).unwrap_or_default())
            .collect();
        Box::into_raw(Box::new(Holders { list, names }))
    }
}

/// The holders behind `h`; null is none.
///
/// # Safety
/// `h` is null or a live list from this module.
unsafe fn list<'a>(h: *const Holders) -> &'a [Holder] {
    // SAFETY: as the caller promises.
    unsafe { h.as_ref() }.map_or(&[], |h| h.list.as_slice())
}

/// A C string as `&str`; null is empty, invalid UTF-8 is replaced.
///
/// # Safety
/// `s` is null or a NUL-terminated string.
unsafe fn text<'a>(s: *const c_char) -> std::borrow::Cow<'a, str> {
    if s.is_null() {
        return std::borrow::Cow::Borrowed("");
    }
    // SAFETY: the caller passes a NUL-terminated string.
    unsafe { CStr::from_ptr(s) }.to_string_lossy()
}

fn owned(s: String) -> *mut c_char {
    CString::new(s.replace('\0', " ")).map_or(std::ptr::null_mut(), CString::into_raw)
}

fn cause(c: c_int) -> Cause {
    if c == OBS_CAMERA_BUSY_CAUSE_OPENED {
        Cause::Opened
    } else {
        Cause::Busy
    }
}

fn show_code(show: Show) -> c_int {
    match show {
        Show::Dialog => OBS_CAMERA_BUSY_DIALOG,
        Show::Allowed => OBS_CAMERA_BUSY_ALLOWED,
        Show::Duplicate => OBS_CAMERA_BUSY_DUPLICATE,
        Show::RateLimited => OBS_CAMERA_BUSY_RATE_LIMITED,
        Show::NoHolder => OBS_CAMERA_BUSY_NO_HOLDER,
    }
}

fn show(c: c_int) -> Show {
    match c {
        OBS_CAMERA_BUSY_DIALOG => Show::Dialog,
        OBS_CAMERA_BUSY_ALLOWED => Show::Allowed,
        OBS_CAMERA_BUSY_DUPLICATE => Show::Duplicate,
        OBS_CAMERA_BUSY_RATE_LIMITED => Show::RateLimited,
        _ => Show::NoHolder,
    }
}

fn answer(c: c_int) -> Answer {
    match c {
        OBS_CAMERA_BUSY_CLOSE_IT => Answer::CloseIt,
        OBS_CAMERA_BUSY_ALWAYS_ALLOW => Answer::AlwaysAllow,
        _ => Answer::Ignore,
    }
}

/// `device` with its symlinks resolved, as the `fd` links name it.
fn resolved(device: &str) -> PathBuf {
    std::fs::canonicalize(device).unwrap_or_else(|_| PathBuf::from(device))
}

/// [`crate::find_holders`] under `/proc` for this process and user.
/// Free the result with [`obs_camera_busy_holders_free`].
///
/// # Safety
/// `device` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_scan(device: *const c_char) -> *mut Holders {
    // SAFETY: as the caller promises.
    let device = resolved(&unsafe { text(device) });
    Holders::new(crate::find_holders(
        Path::new("/proc"),
        &device,
        std::process::id(),
        crate::current_uid(),
    ))
}

/// [`crate::find_holders`] under `proc_root`, with `device` as given.
///
/// # Safety
/// `proc_root` and `device` are null or NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_scan_root(
    proc_root: *const c_char,
    device: *const c_char,
    self_pid: u32,
    self_uid: u32,
) -> *mut Holders {
    // SAFETY: as the caller promises.
    let (root, device) = unsafe { (text(proc_root), text(device)) };
    Holders::new(crate::find_holders(
        Path::new(&*root),
        Path::new(&*device),
        self_pid,
        Some(self_uid),
    ))
}

/// The number of holders; null is 0.
///
/// # Safety
/// `h` is null or a live list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_holders_count(h: *const Holders) -> usize {
    // SAFETY: as the caller promises.
    unsafe { list(h) }.len()
}

/// The PID of holder `i`; 0 out of range.
///
/// # Safety
/// `h` is null or a live list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_holder_pid(h: *const Holders, i: usize) -> u32 {
    // SAFETY: as the caller promises.
    unsafe { list(h) }.get(i).map_or(0, |h| h.pid)
}

/// The name of holder `i`, owned by the list; "" out of range.
///
/// # Safety
/// `h` is null or a live list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_holder_name(h: *const Holders, i: usize) -> *const c_char {
    // SAFETY: as the caller promises.
    match unsafe { h.as_ref() }.and_then(|h| h.names.get(i)) {
        Some(name) => name.as_ptr(),
        None => c"".as_ptr(),
    }
}

/// Whether Close it may signal holder `i`.
///
/// # Safety
/// `h` is null or a live list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_holder_can_close(h: *const Holders, i: usize) -> bool {
    // SAFETY: as the caller promises.
    unsafe { list(h) }.get(i).is_some_and(|h| h.can_close)
}

/// Free a list. Null is ignored.
///
/// # Safety
/// `h` is null or a list from this module, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_holders_free(h: *mut Holders) {
    if !h.is_null() {
        // SAFETY: from Box::into_raw in Holders::new.
        drop(unsafe { Box::from_raw(h) });
    }
}

/// Replace the allowlist with the config string `config`.
///
/// # Safety
/// `config` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_set_allowlist(config: *const c_char) {
    // SAFETY: as the caller promises.
    let list = Allowlist::parse(&unsafe { text(config) });
    with_warnings(|w| w.set_allowlist(list));
}

/// The allowlist as a config string; free it with [`obs_camera_busy_free`].
#[unsafe(no_mangle)]
pub extern "C" fn obs_camera_busy_allowlist() -> *mut c_char {
    owned(with_warnings(|w| w.allowlist().to_config()))
}

/// [`Warnings::begin`]. Returns a list of the holders to name, which the
/// caller passes to [`obs_camera_busy_end`] and frees; `*show` gets the
/// `OBS_CAMERA_BUSY_*` decision.
///
/// # Safety
/// `device` is null or a NUL-terminated string, `holders` null or a live
/// list, and `show` null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_begin(
    cause_code: c_int,
    device: *const c_char,
    holders: *const Holders,
    now_ms: u64,
    show: *mut c_int,
) -> *mut Holders {
    // SAFETY: as the caller promises.
    let (device, holders) = unsafe { (text(device), list(holders)) };
    let begin = with_warnings(|w| w.begin(cause(cause_code), &device, holders, now_ms));
    // SAFETY: as the caller promises.
    if let Some(out) = unsafe { show.as_mut() } {
        *out = show_code(begin.show);
    }
    Holders::new(begin.holders)
}

/// [`Warnings::end`]. Returns whether the allowlist changed (save
/// [`obs_camera_busy_allowlist`]). The PIDs to close are those of
/// `holders` that [`obs_camera_busy_holder_can_close`].
///
/// # Safety
/// `device` is null or a NUL-terminated string, `holders` null or a live
/// list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_end(
    device: *const c_char,
    holders: *const Holders,
    answer_code: c_int,
    now_ms: u64,
) -> bool {
    // SAFETY: as the caller promises.
    let (device, holders) = unsafe { (text(device), list(holders)) };
    with_warnings(|w| w.end(&device, holders, answer(answer_code), now_ms)).allowlist_changed
}

/// [`crate::holder_list`]; free it with [`obs_camera_busy_free`].
///
/// # Safety
/// `holders` is null or a live list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_holder_list(holders: *const Holders) -> *mut c_char {
    // SAFETY: as the caller promises.
    owned(crate::holder_list(unsafe { list(holders) }))
}

/// [`Warnings::unknown_open`].
///
/// # Safety
/// `device` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_unknown_open(device: *const c_char) -> bool {
    // SAFETY: as the caller promises.
    let device = unsafe { text(device) };
    with_warnings(|w| w.unknown_open(&device))
}

/// [`crate::closable_names`]; free it with [`obs_camera_busy_free`].
///
/// # Safety
/// `holders` is null or a live list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_closable_names(holders: *const Holders) -> *mut c_char {
    // SAFETY: as the caller promises.
    owned(crate::closable_names(unsafe { list(holders) }))
}

/// [`crate::log_line`]; free it with [`obs_camera_busy_free`].
///
/// # Safety
/// `device` is null or a NUL-terminated string, `holders` null or a live
/// list.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_log_line(
    cause_code: c_int,
    device: *const c_char,
    holders: *const Holders,
    show_value: c_int,
) -> *mut c_char {
    // SAFETY: as the caller promises.
    let (device, holders) = unsafe { (text(device), list(holders)) };
    owned(crate::log_line(
        cause(cause_code),
        &device,
        holders,
        show(show_value),
    ))
}

/// [`crate::answer_log_line`]; free it with [`obs_camera_busy_free`].
///
/// # Safety
/// As [`obs_camera_busy_log_line`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_answer_log_line(
    device: *const c_char,
    holders: *const Holders,
    answer_code: c_int,
) -> *mut c_char {
    // SAFETY: as the caller promises.
    let (device, holders) = unsafe { (text(device), list(holders)) };
    owned(crate::answer_log_line(
        &device,
        holders,
        answer(answer_code),
    ))
}

/// Close it for `pid` with [`crate::KILL_GRACE_MS`]: blocks up to twice
/// that, so not on the UI thread. Returns `OBS_CAMERA_BUSY_EXITED`,
/// `_KILLED` or `_FAILED`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_camera_busy_close(pid: u32) -> c_int {
    match crate::close_with(&mut ProcessSignals, pid, crate::KILL_GRACE_MS) {
        Closed::Exited => OBS_CAMERA_BUSY_EXITED,
        Closed::Killed => OBS_CAMERA_BUSY_KILLED,
        Closed::Failed => OBS_CAMERA_BUSY_FAILED,
    }
}

/// [`crate::close_log_line`]; free it with [`obs_camera_busy_free`].
///
/// # Safety
/// `name` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_close_log_line(
    name: *const c_char,
    pid: u32,
    closed: c_int,
) -> *mut c_char {
    let closed = match closed {
        OBS_CAMERA_BUSY_EXITED => Closed::Exited,
        OBS_CAMERA_BUSY_KILLED => Closed::Killed,
        _ => Closed::Failed,
    };
    // SAFETY: as the caller promises.
    owned(crate::close_log_line(&unsafe { text(name) }, pid, closed))
}

/// Called on the watcher thread with the holders (a list the callee frees
/// with [`obs_camera_busy_holders_free`]).
pub type WatchCallback = unsafe extern "C" fn(param: *mut c_void, holders: *mut Holders);

/// The C pointer, sent to the watcher thread. The caller promises that
/// `param` may be used from it.
struct Param(*mut c_void);
// SAFETY: obs_camera_busy_watch's contract.
unsafe impl Send for Param {}

/// [`crate::watch_holders`] on `device` (resolved) under `/proc` for this
/// process. Null when the watch cannot start. Stop it with
/// [`obs_camera_busy_unwatch`], which waits for a callback in progress.
///
/// # Safety
/// `device` is null or a NUL-terminated string; `cb` and `param` may be
/// called from another thread until [`obs_camera_busy_unwatch`] returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_watch(
    device: *const c_char,
    cb: WatchCallback,
    param: *mut c_void,
) -> *mut Watcher {
    // SAFETY: as the caller promises.
    let device = resolved(&unsafe { text(device) });
    let param = Param(param);
    let on_holders = Box::new(move |holders: Vec<Holder>| {
        let param = &param;
        // SAFETY: obs_camera_busy_watch's contract; the callee frees the list.
        unsafe { cb(param.0, Holders::new(holders)) };
    });
    crate::watch_holders(
        &device,
        Path::new("/proc"),
        std::process::id(),
        crate::current_uid(),
        crate::OPEN_SETTLE_MS,
        on_holders,
    )
    .map_or(std::ptr::null_mut(), |w| Box::into_raw(Box::new(w)))
}

/// Stop a watch and wait for its thread. Null is ignored.
///
/// # Safety
/// `w` is null or from [`obs_camera_busy_watch`], stopped once, and not
/// from inside its own callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_unwatch(w: *mut Watcher) {
    if !w.is_null() {
        // SAFETY: from Box::into_raw in obs_camera_busy_watch.
        drop(unsafe { Box::from_raw(w) });
    }
}

/// Free a string from this module. Null is ignored.
///
/// # Safety
/// `s` is null or a string from this module, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_camera_busy_free(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: from CString::into_raw in owned().
        drop(unsafe { CString::from_raw(s) });
    }
}
