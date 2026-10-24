//! The C ABI the frontend calls (#2218).

use std::ffi::{CStr, CString};

use obs_fallback_warning::ffi::{
    OBS_FALLBACK_WARNING_ACCEPT_NOW, OBS_FALLBACK_WARNING_DIALOG, OBS_FALLBACK_WARNING_END_ACCEPT,
    OBS_FALLBACK_WARNING_END_CANCEL, OBS_FALLBACK_WARNING_END_CLOSED,
    OBS_FALLBACK_WARNING_END_COUNTDOWN, obs_fallback_warning_accepted, obs_fallback_warning_begin,
    obs_fallback_warning_countdown_ms, obs_fallback_warning_free, obs_fallback_warning_log_line,
    obs_fallback_warning_progress,
};

/// The log line through the C ABI, freed.
fn line(id: Option<&CStr>, message: Option<&CStr>) -> String {
    let ptr = |s: Option<&CStr>| s.map_or(std::ptr::null(), CStr::as_ptr);
    // SAFETY: null or C strings.
    let raw = unsafe { obs_fallback_warning_log_line(ptr(id), ptr(message)) };
    assert!(!raw.is_null());
    // SAFETY: NUL-terminated and still ours.
    let text = unsafe { CStr::from_ptr(raw) }.to_str().unwrap().to_owned();
    // SAFETY: from obs_fallback_warning_log_line, freed once.
    unsafe { obs_fallback_warning_free(raw) };
    text
}

#[test]
fn begin_uses_one_session_per_process() {
    // A unique id: the tests in this binary share the process session.
    let id = CString::new("ffi-begin-once").unwrap();
    // SAFETY: a C string.
    assert_eq!(
        unsafe { obs_fallback_warning_begin(id.as_ptr(), true) },
        OBS_FALLBACK_WARNING_DIALOG
    );
    // SAFETY: as above.
    assert_eq!(
        unsafe { obs_fallback_warning_begin(id.as_ptr(), true) },
        OBS_FALLBACK_WARNING_ACCEPT_NOW
    );
}

#[test]
fn begin_without_a_window_accepts_now() {
    let id = CString::new("ffi-begin-no-window").unwrap();
    // SAFETY: a C string.
    assert_eq!(
        unsafe { obs_fallback_warning_begin(id.as_ptr(), false) },
        OBS_FALLBACK_WARNING_ACCEPT_NOW
    );
}

#[test]
fn a_null_id_is_an_empty_id() {
    // SAFETY: null is allowed.
    assert_eq!(
        unsafe { obs_fallback_warning_begin(std::ptr::null(), true) },
        OBS_FALLBACK_WARNING_DIALOG
    );
    // SAFETY: as above.
    assert_eq!(
        unsafe { obs_fallback_warning_begin(std::ptr::null(), true) },
        OBS_FALLBACK_WARNING_DIALOG
    );
}

#[test]
fn accepted_maps_every_end() {
    assert!(obs_fallback_warning_accepted(
        OBS_FALLBACK_WARNING_END_COUNTDOWN,
        true
    ));
    assert!(obs_fallback_warning_accepted(
        OBS_FALLBACK_WARNING_END_ACCEPT,
        true
    ));
    assert!(!obs_fallback_warning_accepted(
        OBS_FALLBACK_WARNING_END_CANCEL,
        true
    ));
    assert!(!obs_fallback_warning_accepted(
        OBS_FALLBACK_WARNING_END_CLOSED,
        true
    ));
    // An unknown value is a close.
    assert!(!obs_fallback_warning_accepted(42, true));
    assert!(obs_fallback_warning_accepted(42, false));
}

#[test]
fn progress_and_countdown() {
    assert_eq!(obs_fallback_warning_countdown_ms(), 3_000);
    assert_eq!(obs_fallback_warning_progress(1_500), 50);
    assert_eq!(obs_fallback_warning_progress(u64::MAX), 100);
}

#[test]
fn log_line_through_the_c_abi() {
    let id = CString::new("browser-sandbox").unwrap();
    let message = CString::new("Browser sources run without the sandbox.").unwrap();
    assert_eq!(
        line(Some(&id), Some(&message)),
        "[fallback] browser-sandbox: Browser sources run without the sandbox. This may become an error in a \
         future version of OBS."
    );
}

#[test]
fn log_line_with_null_strings() {
    assert_eq!(
        line(None, None),
        "[fallback] : This may become an error in a future version of OBS."
    );
}
