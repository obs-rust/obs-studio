//! Tier 3: the original extracted C entry and the Rust shim over real canvas
//! layouts, the real flags field, the Rust rules and `obs_video_active()` on
//! the recording core, and a recording internal reset. Compared: the answer,
//! whether and with what the internal reset is called, whether the activity
//! query runs (short-circuit order), and unchanged canvas and info bytes.
//! NULL canvas with non-NULL info is C UB, excluded. No intentional differences.
use core::ffi::c_void;
use obs_core::ffi::canvas_reset_video::obs_canvas_reset_video;
use proptest::prelude::*;

type Reset = unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool;
unsafe extern "C" {
    fn oracle_obs_canvas_reset_video(canvas: *mut c_void, ovi: *mut c_void) -> bool;
    fn oracle_canvas_reset_video_run(
        candidate: Reset,
        flags: u32,
        raw_active: core::ffi::c_long,
        null_info: bool,
        result: bool,
        trace: *mut usize,
    );
}

fn run(candidate: Reset, flags: u32, raw: i32, null_info: bool, result: bool) -> [usize; 6] {
    let mut trace = [0; 6];
    // SAFETY: the fixture owns a live real-layout canvas and info, arms the
    // thread-local recording core, and writes exactly six words.
    unsafe {
        oracle_canvas_reset_video_run(
            candidate,
            flags,
            raw.into(),
            null_info,
            result,
            trace.as_mut_ptr(),
        )
    };
    trace
}

fn compare(flags: u32, raw: i32, null_info: bool, result: bool) -> [usize; 6] {
    let c = run(oracle_obs_canvas_reset_video, flags, raw, null_info, result);
    let rust = run(obs_canvas_reset_video, flags, raw, null_info, result);
    assert_eq!(
        rust, c,
        "flags={flags:x} raw={raw} null={null_info} result={result}"
    );
    assert_eq!(c[5], 1, "canvas or info changed");
    c
}

#[test]
fn characterized_cases_match() {
    // NULL info: nothing read, nothing called.
    assert_eq!(compare(0, 1, true, true)[..5], [0, 0, 0, 0, 0]);
    // Main canvas: refused before the activity query.
    assert_eq!(compare(1, 0, false, true)[..5], [0, 0, 0, 0, 0]);
    // Active video: queried, refused.
    let active = compare(0b1110, 1, false, true);
    assert_eq!(active[..2], [0, 0]);
    assert_ne!(active[4], 0);
    // Removed non-main canvas resets; result forwarded both ways.
    for result in [false, true] {
        let t = compare(0b1110 | (1 << 31), 0, false, result);
        assert_eq!(t[..4], [usize::from(result), 1, 1, 1]);
    }
}

#[test]
fn mutants_are_observable() {
    unsafe extern "C" {
        fn obs_canvas_reset_video_internal(c: *mut c_void, o: *mut c_void) -> bool;
    }
    unsafe extern "C" fn skips_active(canvas: *mut c_void, ovi: *mut c_void) -> bool {
        // SAFETY: forwards the fixture's live, non-null arguments.
        let flags = unsafe { obs_core::ffi::canvas_flags_getter::obs_canvas_get_flags(canvas) };
        // SAFETY: as above.
        flags & 1 == 0 && unsafe { obs_canvas_reset_video_internal(canvas, ovi) }
    }
    let c = run(oracle_obs_canvas_reset_video, 0, 1, false, true);
    let m = run(skips_active, 0, 1, false, true);
    assert_ne!(c, m);
}

proptest! {
    #[test]
    fn any_flags_activity_and_result(flags in any::<u32>(), raw in -2i32..3, null_info in any::<bool>(), result in any::<bool>()) {
        compare(flags, raw, null_info, result);
    }
}
