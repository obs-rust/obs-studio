//! C ABI for the frontend (#2218), declared in
//! `frontend/utility/obs-fallback-warning.h`. It converts types and calls the
//! safe core; there is no logic here. The frontend calls it from its UI
//! thread; the session is one per process.

use std::ffi::{CStr, CString, c_char, c_int};
use std::sync::Mutex;

use crate::{End, Session, Show};

/// [`Show::Dialog`] at the C boundary.
pub const OBS_FALLBACK_WARNING_DIALOG: c_int = 0;
/// [`Show::AcceptNow`] at the C boundary.
pub const OBS_FALLBACK_WARNING_ACCEPT_NOW: c_int = 1;

/// [`End`] values at the C boundary; anything else is [`End::Closed`].
pub const OBS_FALLBACK_WARNING_END_COUNTDOWN: c_int = 0;
pub const OBS_FALLBACK_WARNING_END_ACCEPT: c_int = 1;
pub const OBS_FALLBACK_WARNING_END_CANCEL: c_int = 2;
pub const OBS_FALLBACK_WARNING_END_CLOSED: c_int = 3;

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

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

/// [`Session::begin`] on the process's session.
///
/// # Safety
/// `id` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_fallback_warning_begin(
    id: *const c_char,
    window_visible: bool,
) -> c_int {
    // SAFETY: as the caller promises.
    let id = unsafe { text(id) };
    let mut session = SESSION.lock().unwrap_or_else(|e| e.into_inner());
    match session
        .get_or_insert_with(Session::new)
        .begin(&id, window_visible)
    {
        Show::Dialog => OBS_FALLBACK_WARNING_DIALOG,
        Show::AcceptNow => OBS_FALLBACK_WARNING_ACCEPT_NOW,
    }
}

/// [`crate::accepted`].
#[unsafe(no_mangle)]
pub extern "C" fn obs_fallback_warning_accepted(end: c_int, can_cancel: bool) -> bool {
    let end = match end {
        OBS_FALLBACK_WARNING_END_COUNTDOWN => End::CountdownDone,
        OBS_FALLBACK_WARNING_END_ACCEPT => End::AcceptClicked,
        OBS_FALLBACK_WARNING_END_CANCEL => End::CancelClicked,
        _ => End::Closed,
    };
    crate::accepted(end, can_cancel)
}

/// [`crate::progress_percent`].
#[unsafe(no_mangle)]
pub extern "C" fn obs_fallback_warning_progress(elapsed_ms: u64) -> c_int {
    c_int::from(crate::progress_percent(elapsed_ms))
}

/// [`crate::COUNTDOWN_MS`].
#[unsafe(no_mangle)]
pub extern "C" fn obs_fallback_warning_countdown_ms() -> u64 {
    crate::COUNTDOWN_MS
}

/// [`crate::log_line`], as a string the caller frees with
/// [`obs_fallback_warning_free`]. Null `id` or `message` is empty.
///
/// # Safety
/// `id` and `message` are null or NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_fallback_warning_log_line(
    id: *const c_char,
    message: *const c_char,
) -> *mut c_char {
    // SAFETY: as the caller promises.
    let (id, message) = unsafe { (text(id), text(message)) };
    let line = crate::log_line(&id, &message).replace('\0', " ");
    CString::new(line).map_or(std::ptr::null_mut(), CString::into_raw)
}

/// Free a string from [`obs_fallback_warning_log_line`]. Null is ignored.
///
/// # Safety
/// `s` is null or from [`obs_fallback_warning_log_line`], freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn obs_fallback_warning_free(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: from CString::into_raw in obs_fallback_warning_log_line.
        drop(unsafe { CString::from_raw(s) });
    }
}
