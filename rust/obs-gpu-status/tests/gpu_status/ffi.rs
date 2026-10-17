//! The C ABI the frontend calls (obs-rust/obs-studio#846, #943).

use std::ffi::{CStr, c_int};

use obs_gpu_status::ffi::{
    obs_gpu_status_free, obs_gpu_status_log_line, obs_gpu_status_report, obs_gpu_status_sample,
    obs_gpu_status_take_log, obs_gpu_status_usage_start,
};

const BUDGET: u64 = 33_333_333;

/// The report for `adapter` and its state; frees the JSON.
fn report(adapter: Option<&CStr>, interval_ns: u64) -> (c_int, String) {
    let mut state: c_int = -1;
    let ptr = adapter.map_or(std::ptr::null(), CStr::as_ptr);
    // SAFETY: null or a C string, and a local int.
    let json = unsafe { obs_gpu_status_report(ptr, interval_ns, &mut state) };
    assert!(!json.is_null());
    // SAFETY: NUL-terminated and still ours.
    let text = unsafe { CStr::from_ptr(json) }.to_str().unwrap().to_owned();
    // SAFETY: from obs_gpu_status_report, freed once.
    unsafe { obs_gpu_status_free(json) };
    (state, text)
}

#[test]
fn report_round_trip() {
    let (state, text) = report(Some(c"Microsoft Basic Render Driver"), 0);
    // Yellow or red depending on the machine's GPUs, never green.
    assert!(state == 1 || state == 2, "{state} {text}");
    assert!(
        text.contains("\"adapter\":\"Microsoft Basic Render Driver\""),
        "{text}"
    );
}

#[test]
fn init_failure_is_red_and_null_frees_are_ignored() {
    assert_eq!(report(None, 0).0, 2);
    // SAFETY: null is ignored.
    unsafe { obs_gpu_status_free(std::ptr::null_mut()) };
}

// The only test that samples: the window is process-wide.
#[test]
fn sampled_frames_drive_the_frame_budget() {
    let gpu = c"NVIDIA GeForce RTX 3060";
    assert_eq!(report(Some(gpu), BUDGET).0, 0, "no samples yet: no timings");
    for _ in 0..5 {
        obs_gpu_status_sample(95_000_000);
    }
    assert_eq!(report(Some(gpu), BUDGET).0, 2, "sustained overrun");
    obs_gpu_status_sample(400_000_000);
    for _ in 0..14 {
        obs_gpu_status_sample(2_000_000);
    }
    assert_eq!(
        report(Some(gpu), BUDGET).0,
        0,
        "one slow second among fast ones"
    );
}

#[test]
fn logged_graphics_lines_reach_the_report() {
    // SAFETY: literal C strings, and null is ignored.
    unsafe {
        obs_gpu_status_log_line(c"Unable to open DRM render node.".as_ptr());
        obs_gpu_status_log_line(c"Audio monitoring device:".as_ptr());
        obs_gpu_status_log_line(std::ptr::null());
    }
    let (_, text) = report(Some(c"Mesa llvmpipe"), 0);
    assert!(
        text.contains("\"logLines\":[\"Unable to open DRM render node.\"]"),
        "{text}"
    );
}

#[test]
fn usage_start_accepts_no_hint_and_the_report_carries_the_readout() {
    // SAFETY: null is allowed; a second call only changes the hint.
    unsafe {
        obs_gpu_status_usage_start(std::ptr::null(), -1);
        obs_gpu_status_usage_start(c"/dev/dri/renderD128".as_ptr(), 0);
    }
    let (_, text) = report(Some(c"Mesa llvmpipe (LLVM 21.1.8, 256 bits)"), 0);
    assert!(text.contains("\"usage\":"), "{text}");
    assert!(
        text.contains("\"readout\":{\"text\":\"CPU rendering\""),
        "{text}"
    );
}

#[test]
fn take_log_is_null_without_lines() {
    // No source in this test process is slow, so nothing is queued.
    assert!(obs_gpu_status_take_log().is_null());
    // A null result is safe to free.
    // SAFETY: null is ignored by obs_gpu_status_free.
    unsafe { obs_gpu_status_free(obs_gpu_status_take_log()) };
}
