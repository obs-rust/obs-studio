#[test]
fn bundled_libobs_reports_the_public_c_api_version() {
    // SAFETY: the bundled build links the original no-argument C API function.
    let version = unsafe { obs_sys::ffi::obs_get_version() };
    assert_eq!(version >> 24, 33);
    assert!(std::path::Path::new(env!("OBS_SYS_LIBRARY")).is_file());
}

#[test]
fn bundled_libobs_starts_and_shuts_down_without_a_frontend() {
    // SAFETY: the locale is a NUL-terminated static string. Null optional paths
    // and profiler store are explicitly supported by obs_startup in obs.h.
    // This is the only test that owns the process-global OBS lifecycle.
    unsafe {
        assert!(!obs_sys::ffi::obs_initialized());
        assert!(obs_sys::ffi::obs_startup(
            c"en-US".as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
        ));
        assert!(obs_sys::ffi::obs_initialized());
        obs_sys::ffi::obs_shutdown();
        assert!(!obs_sys::ffi::obs_initialized());
    }
}
