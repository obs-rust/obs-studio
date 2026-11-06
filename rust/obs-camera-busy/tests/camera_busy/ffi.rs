//! The C ABI the frontend calls (#3724). The warnings state is one per
//! process, so each test uses its own device name.

use std::ffi::{CStr, CString, c_char};

use obs_camera_busy::ffi::{
    OBS_CAMERA_BUSY_ALWAYS_ALLOW, OBS_CAMERA_BUSY_CAUSE_BUSY, OBS_CAMERA_BUSY_DIALOG,
    OBS_CAMERA_BUSY_DUPLICATE, OBS_CAMERA_BUSY_EXITED, OBS_CAMERA_BUSY_FAILED,
    OBS_CAMERA_BUSY_IGNORE, OBS_CAMERA_BUSY_KILLED, OBS_CAMERA_BUSY_RATE_LIMITED,
    obs_camera_busy_allowlist, obs_camera_busy_answer_log_line, obs_camera_busy_begin,
    obs_camera_busy_close_log_line, obs_camera_busy_end, obs_camera_busy_free,
    obs_camera_busy_holder_can_close, obs_camera_busy_holder_list, obs_camera_busy_holder_name,
    obs_camera_busy_holder_pid, obs_camera_busy_holders_count, obs_camera_busy_holders_free,
    obs_camera_busy_log_line, obs_camera_busy_scan, obs_camera_busy_set_allowlist,
};

/// A string from the C ABI, freed.
fn take(raw: *mut c_char) -> String {
    assert!(!raw.is_null());
    // SAFETY: NUL-terminated and still ours.
    let s = unsafe { CStr::from_ptr(raw) }.to_str().unwrap().to_owned();
    // SAFETY: from this C ABI, freed once.
    unsafe { obs_camera_busy_free(raw) };
    s
}

#[test]
fn null_lists_are_empty() {
    // SAFETY: null is allowed everywhere.
    unsafe {
        assert_eq!(obs_camera_busy_holders_count(std::ptr::null()), 0);
        assert_eq!(obs_camera_busy_holder_pid(std::ptr::null(), 0), 0);
        assert!(!obs_camera_busy_holder_can_close(std::ptr::null(), 0));
        let name = obs_camera_busy_holder_name(std::ptr::null(), 3);
        assert_eq!(CStr::from_ptr(name).to_bytes(), b"");
        assert_eq!(take(obs_camera_busy_holder_list(std::ptr::null())), "");
        obs_camera_busy_holders_free(std::ptr::null_mut());
        obs_camera_busy_free(std::ptr::null_mut());
    }
}

#[test]
fn a_device_nobody_holds() {
    let dev = CString::new("/nonexistent/obs-camera-busy-ffi").unwrap();
    // SAFETY: a C string; the list is freed once.
    unsafe {
        let h = obs_camera_busy_scan(dev.as_ptr());
        assert_eq!(obs_camera_busy_holders_count(h), 0);
        obs_camera_busy_holders_free(h);
    }
}

#[test]
fn begin_end_and_log_through_the_c_abi() {
    let dev = CString::new("/dev/obs-camera-busy-ffi-begin").unwrap();
    let mut show = -1;
    // SAFETY: C strings, live lists, each freed once.
    unsafe {
        let shown = obs_camera_busy_begin(
            OBS_CAMERA_BUSY_CAUSE_BUSY,
            dev.as_ptr(),
            std::ptr::null(),
            0,
            &raw mut show,
        );
        assert_eq!(show, OBS_CAMERA_BUSY_DIALOG);
        assert_eq!(obs_camera_busy_holders_count(shown), 0);
        assert_eq!(
            take(obs_camera_busy_log_line(
                OBS_CAMERA_BUSY_CAUSE_BUSY,
                dev.as_ptr(),
                shown,
                show
            )),
            "[camera-busy] /dev/obs-camera-busy-ffi-begin is busy, in use by another program: warning shown"
        );

        let again = obs_camera_busy_begin(
            OBS_CAMERA_BUSY_CAUSE_BUSY,
            dev.as_ptr(),
            std::ptr::null(),
            1,
            &raw mut show,
        );
        assert_eq!(show, OBS_CAMERA_BUSY_DUPLICATE);
        obs_camera_busy_holders_free(again);

        assert!(!obs_camera_busy_end(
            dev.as_ptr(),
            shown,
            OBS_CAMERA_BUSY_IGNORE,
            2
        ));
        assert_eq!(
            take(obs_camera_busy_answer_log_line(
                dev.as_ptr(),
                shown,
                OBS_CAMERA_BUSY_IGNORE
            )),
            "[camera-busy] /dev/obs-camera-busy-ffi-begin: Ignore"
        );
        obs_camera_busy_holders_free(shown);

        let later = obs_camera_busy_begin(
            OBS_CAMERA_BUSY_CAUSE_BUSY,
            dev.as_ptr(),
            std::ptr::null(),
            3,
            std::ptr::null_mut(),
        );
        obs_camera_busy_holders_free(later);
        let mut show2 = -1;
        let later = obs_camera_busy_begin(
            OBS_CAMERA_BUSY_CAUSE_BUSY,
            dev.as_ptr(),
            std::ptr::null(),
            4,
            &raw mut show2,
        );
        assert_eq!(show2, OBS_CAMERA_BUSY_RATE_LIMITED);
        obs_camera_busy_holders_free(later);
    }
}

#[test]
fn allowlist_round_trip() {
    let config = CString::new("zoom;cheese").unwrap();
    // SAFETY: a C string.
    unsafe { obs_camera_busy_set_allowlist(config.as_ptr()) };
    assert_eq!(take(obs_camera_busy_allowlist()), "cheese;zoom");
    // Always allow with no holders changes nothing.
    let dev = CString::new("/dev/obs-camera-busy-ffi-allow").unwrap();
    // SAFETY: a C string and a null list.
    assert!(!unsafe {
        obs_camera_busy_end(
            dev.as_ptr(),
            std::ptr::null(),
            OBS_CAMERA_BUSY_ALWAYS_ALLOW,
            0,
        )
    });
}

#[test]
fn close_log_line_through_the_c_abi() {
    let name = CString::new("zoom").unwrap();
    // SAFETY: a C string.
    unsafe {
        assert_eq!(
            take(obs_camera_busy_close_log_line(name.as_ptr(), 12, OBS_CAMERA_BUSY_EXITED)),
            "[camera-busy] zoom (PID 12) exited on SIGTERM"
        );
        assert!(
            take(obs_camera_busy_close_log_line(name.as_ptr(), 12, OBS_CAMERA_BUSY_KILLED))
                .ends_with("was killed (SIGKILL)")
        );
        assert!(
            take(obs_camera_busy_close_log_line(name.as_ptr(), 12, OBS_CAMERA_BUSY_FAILED))
                .ends_with("could not be closed")
        );
    }
}
