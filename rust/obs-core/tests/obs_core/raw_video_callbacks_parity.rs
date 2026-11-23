//! Tier 3: the unchanged original C versus the production Rust bridge/core
//! with one recorder for start_raw_video, stop_raw_video and the warning,
//! over a real-header core read through the real obs_get_video() field
//! adapter. No absent-core cases (C UB). No intentional behavior differences.
use core::ffi::{c_char, c_int, c_void};
use obs_core::ffi::get_video::Fields;
use obs_core::ffi::raw_video_callbacks::{
    Callback, Calls, Conversion, add_with, add2_with, remove_with,
};
use obs_core::raw_video_callbacks::NOT_SET_UP;
use proptest::prelude::*;

type Add = unsafe extern "C" fn(*const Conversion, Callback, *mut c_void);
type Add2 = unsafe extern "C" fn(*const Conversion, u32, Callback, *mut c_void);
type Remove = unsafe extern "C" fn(Callback, *mut c_void);
const TEXT: usize = 160;

unsafe extern "C" {
    fn oracle_obs_add_raw_video_callback(
        conversion: *const Conversion,
        callback: Callback,
        param: *mut c_void,
    );
    fn oracle_obs_add_raw_video_callback2(
        conversion: *const Conversion,
        divisor: u32,
        callback: Callback,
        param: *mut c_void,
    );
    fn oracle_obs_remove_raw_video_callback(callback: Callback, param: *mut c_void);
    fn oracle_raw_video_callbacks_fields() -> Fields;
    fn oracle_raw_video_callbacks_start(
        video: *mut c_void,
        conversion: *const Conversion,
        divisor: u32,
        callback: Callback,
        param: *mut c_void,
    );
    fn oracle_raw_video_callbacks_stop(video: *mut c_void, callback: Callback, param: *mut c_void);
    fn oracle_raw_video_callbacks_log(level: c_int, format: *const c_char, ...);
    fn oracle_raw_video_callbacks_run(
        add: Add,
        add2: Add2,
        remove: Remove,
        state: usize,
        op: usize,
        conversion: usize,
        divisor: u32,
        callback: usize,
        param: usize,
        repeats: usize,
        report: *mut usize,
        text: *mut c_char,
    );
    fn oracle_raw_video_callbacks_layout(out: *mut usize);
}

fn calls() -> Calls {
    Calls {
        fields: oracle_raw_video_callbacks_fields,
        start: oracle_raw_video_callbacks_start,
        stop: oracle_raw_video_callbacks_stop,
        log: oracle_raw_video_callbacks_log,
    }
}

unsafe extern "C" fn rust_add(
    conversion: *const Conversion,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: the C fixture supplies the live real-header core and opaque
    // identities; the recorder never dereferences or calls them.
    unsafe { add_with(conversion, callback, param, &mut calls()) };
}

unsafe extern "C" fn rust_add2(
    conversion: *const Conversion,
    divisor: u32,
    callback: Callback,
    param: *mut c_void,
) {
    // SAFETY: as in rust_add.
    unsafe { add2_with(conversion, divisor, callback, param, &mut calls()) };
}

unsafe extern "C" fn rust_remove(callback: Callback, param: *mut c_void) {
    // SAFETY: as in rust_add.
    unsafe { remove_with(callback, param, &mut calls()) };
}

#[allow(clippy::too_many_arguments)]
fn run(
    original: bool,
    state: usize,
    op: usize,
    conversion: usize,
    divisor: u32,
    callback: usize,
    param: usize,
    repeats: usize,
) -> ([usize; 12], [u8; TEXT]) {
    let mut report = [0usize; 12];
    let mut text = [0u8; TEXT];
    let (add, add2, remove): (Add, Add2, Remove) = if original {
        (
            oracle_obs_add_raw_video_callback,
            oracle_obs_add_raw_video_callback2,
            oracle_obs_remove_raw_video_callback,
        )
    } else {
        (rust_add, rust_add2, rust_remove)
    };
    // SAFETY: a 12-slot report, a TEXT-byte buffer, valid ids and 1..=3
    // repeats. The fixture resets its owned real-header core after each run.
    unsafe {
        oracle_raw_video_callbacks_run(
            add,
            add2,
            remove,
            state,
            op,
            conversion,
            divisor,
            callback,
            param,
            repeats,
            report.as_mut_ptr(),
            text.as_mut_ptr().cast(),
        )
    };
    (report, text)
}

#[allow(clippy::too_many_arguments)]
fn compare(
    state: usize,
    op: usize,
    conversion: usize,
    divisor: u32,
    callback: usize,
    param: usize,
    repeats: usize,
) {
    let c = run(
        true, state, op, conversion, divisor, callback, param, repeats,
    );
    let rust = run(
        false, state, op, conversion, divisor, callback, param, repeats,
    );
    let at = format!(
        "state={state} op={op} conversion={conversion} divisor={divisor} callback={callback} param={param} repeats={repeats}"
    );
    assert_eq!(rust.0, c.0, "{at}");
    assert_eq!(rust.1, c.1, "{at}");
    let report = c.0;
    let has_mix = state >= 2;
    let video = match state {
        3 => 1,
        4 => 2,
        _ => 0,
    };
    let (starts, stops) = match op {
        2 => (0, if has_mix { repeats } else { 0 }),
        _ => (if has_mix { repeats } else { 0 }, 0),
    };
    assert_eq!(&report[0..2], &[starts, stops], "{at}");
    let warned = op != 2 && !has_mix;
    assert_eq!(report[2], if warned { repeats } else { 0 }, "{at}");
    if warned {
        assert_eq!(report[8], report[11], "not LOG_WARNING: {at}");
        let len = c.1.iter().position(|&b| b == 0).unwrap();
        assert_eq!(&c.1[..len], NOT_SET_UP.to_bytes(), "{at}");
    } else {
        assert_eq!(report[8], 0, "{at}");
    }
    if has_mix {
        let divisor = match op {
            0 => 1,
            1 => usize::try_from(divisor).unwrap(),
            _ => 0,
        };
        let conversion = if op == 2 { 0 } else { conversion };
        assert_eq!(
            &report[3..8],
            &[video, conversion, divisor, callback, param],
            "{at}"
        );
    } else {
        assert_eq!(&report[3..8], &[0; 5], "{at}");
    }
    assert_eq!(report[9], 1, "wrapper mutated the core: {at}");
    assert_eq!(report[10], 1, "wrapper mutated the conversion: {at}");
}

#[test]
fn original_c_arguments_calls_and_warning() {
    const CHILD: &str = "OBS_RAW_VIDEO_CALLBACKS_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(format!(
                "{}::original_c_arguments_calls_and_warning",
                module_path!().split_once("::").unwrap().1
            ))
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let mut layout = [0usize; 15];
    // SAFETY: the C header probe writes exactly fifteen size/align/offset values.
    unsafe { oracle_raw_video_callbacks_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<Conversion>(),
            align_of::<Conversion>(),
            std::mem::offset_of!(Conversion, format),
            std::mem::offset_of!(Conversion, width),
            std::mem::offset_of!(Conversion, height),
            std::mem::offset_of!(Conversion, range),
            std::mem::offset_of!(Conversion, colorspace),
            size_of::<Callback>(),
            align_of::<Callback>(),
            size_of::<u32>(),
            size_of::<*mut c_void>(),
            size_of::<Fields>(),
            std::mem::offset_of!(Fields, canvas),
            std::mem::offset_of!(Fields, mix),
            std::mem::offset_of!(Fields, video),
        ]
    );
    for state in 0..5 {
        for op in 0..3 {
            for divisor in [0, 1, 2, 3, u32::MAX] {
                for conversion in 0..3 {
                    for callback in 0..4 {
                        for param in 0..4 {
                            compare(state, op, conversion, divisor, callback, param, 2);
                        }
                    }
                }
            }
        }
    }
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Default::default()
    });
    runner
        .run(
            &(
                0usize..5,
                0usize..3,
                0usize..3,
                any::<u32>(),
                0usize..4,
                0usize..4,
                1usize..4,
            ),
            |(state, op, conversion, divisor, callback, param, repeats)| {
                compare(state, op, conversion, divisor, callback, param, repeats);
                Ok(())
            },
        )
        .unwrap();
}
