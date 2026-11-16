//! Tier 3: `gs_texture_create_nv12` and `gs_texture_create_p010`, the Rust
//! shims against the original `texture-planar-creators.c.h`, compiled as
//! `oracle_gs_texture_create_*` (oracle/texture_planar_creators.c).
//!
//! Both sides read one stand-in graphics context with the real private
//! layout: the C directly, the shim through the production field adapter
//! compiled over it. Its two planar device exports, the planes' create
//! and destroy, and every log line go to the recording `gs_*` stand-in's
//! one call log (oracle/texture_render_device.c). A case runs each side
//! from the same reset and compares the return value, the full call log
//! (the arguments of every device call and plane, each destroy's handle,
//! and each log line with its level), and what ends in both outputs.
//!
//! The cases are every combination of the context, the two exports, their
//! answer and whether they write the outputs, plane failures, outputs that
//! start null or set and outputs that are the same slot, over a grid of
//! sizes and flags, and then random sizes and flags.
//!
//! No intentional differences. Null out-pointers are not passed: the C does
//! not check them either.

use core::ffi::{CStr, c_void};

use obs_c_oracle::texture_planar_creators as c;
use obs_c_oracle::texture_render as device;
use obs_graphics::ffi::texture_planar_creators as r;
use proptest::prelude::*;

type Entry = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void, u32, u32, u32) -> bool;

#[derive(Clone, Copy, Debug)]
struct Case {
    p010: bool,
    present: bool,
    nv12_export: bool,
    p010_export: bool,
    answer: bool,
    writes: bool,
    /// Bit 0 fails the first plane create, bit 1 the second.
    fail: u32,
    alias: bool,
    start_set: bool,
    width: u32,
    height: u32,
    flags: u32,
}

/// What the outputs start as when [`Case::start_set`].
static START: u8 = 0;

fn start() -> *mut c_void {
    (&raw const START).cast_mut().cast()
}

/// -3 for [`START`], 20000 + a device export's token, else the texture
/// stand-in's handle (-1 for null).
fn handle(ptr: *mut c_void) -> i32 {
    if ptr == start() {
        return -3;
    }
    // SAFETY: both only compare the address.
    unsafe {
        match c::oracle_tp_token(ptr) {
            -1 => device::oracle_texrender_device_handle(ptr),
            token => 20000 + token,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    result: bool,
    log: String,
    outputs: [i32; 2],
}

fn run(entry: Entry, case: &Case) -> Outcome {
    let first = if case.start_set {
        start()
    } else {
        core::ptr::null_mut()
    };
    let mut slots = [first; 2];
    let base = slots.as_mut_ptr();
    // SAFETY: the stand-ins are reset before the call; both out-pointers
    // point into `slots`, which outlives it.
    let result = unsafe {
        device::oracle_texrender_device_reset();
        device::oracle_texrender_device_fail_creates(case.fail);
        c::oracle_tp_reset(
            case.present,
            case.nv12_export,
            case.p010_export,
            case.answer,
            case.writes,
        );
        device::oracle_texrender_device_capture_log(true);
        let uv = if case.alias { base } else { base.add(1) };
        let result = entry(base, uv, case.width, case.height, case.flags);
        device::oracle_texrender_device_capture_log(false);
        result
    };
    // SAFETY: the stand-in's log is a NUL-terminated static buffer.
    let log = unsafe { CStr::from_ptr(device::oracle_texrender_device_log()) }
        .to_string_lossy()
        .into_owned();
    Outcome {
        result,
        log,
        outputs: slots.map(handle),
    }
}

fn compare(case: &Case) -> Outcome {
    let (rust, original): (Entry, Entry) = if case.p010 {
        (r::gs_texture_create_p010, c::oracle_gs_texture_create_p010)
    } else {
        (r::gs_texture_create_nv12, c::oracle_gs_texture_create_nv12)
    };
    let expected = run(original, case);
    let actual = run(rust, case);
    assert_eq!(actual, expected, "{case:?}");
    expected
}

const SIZES: [(u32, u32); 8] = [
    (0, 0),
    (2, 2),
    (3, 4),
    (4, 3),
    (1920, 1080),
    (u32::MAX, 2),
    (2, u32::MAX),
    (u32::MAX - 1, u32::MAX - 1),
];
const FLAGS: [u32; 3] = [0, 1 << 1, u32::MAX];

fn grid() -> impl Iterator<Item = Case> {
    let bits =
        |n: u32| (0..1u32 << n).map(move |b| (0..n).map(|i| b >> i & 1 == 1).collect::<Vec<_>>());
    bits(8).flat_map(|b| {
        (0..4).flat_map(move |fail| {
            let b = b.clone();
            SIZES.into_iter().flat_map(move |(width, height)| {
                let b = b.clone();
                FLAGS.into_iter().map(move |flags| Case {
                    p010: b[0],
                    present: b[1],
                    nv12_export: b[2],
                    p010_export: b[3],
                    answer: b[4],
                    writes: b[5],
                    fail,
                    alias: b[6],
                    start_set: b[7],
                    width,
                    height,
                    flags,
                })
            })
        })
    })
}

fn size() -> impl Strategy<Value = u32> {
    prop_oneof![
        2 => any::<u32>(),
        1 => 0u32..64,
        1 => (0u32..16).prop_map(|k| u32::MAX - k),
    ]
}

fn case() -> impl Strategy<Value = Case> {
    (any::<[bool; 8]>(), 0u32..4, size(), size(), any::<u32>()).prop_map(
        |(b, fail, width, height, flags)| Case {
            p010: b[0],
            present: b[1],
            nv12_export: b[2],
            p010_export: b[3],
            answer: b[4],
            writes: b[5],
            fail,
            alias: b[6],
            start_set: b[7],
            width,
            height,
            flags,
        },
    )
}

fn parity() {
    // SAFETY: plain reads of the stand-in after a reset.
    unsafe {
        c::oracle_tp_reset(true, true, true, false, false);
        assert!(c::oracle_tp_adapter_layout(), "field adapter layout");
        c::oracle_tp_reset(true, false, false, false, false);
        assert!(c::oracle_tp_adapter_layout(), "field adapter layout");
    }

    // A few pinned outcomes, so a stand-in that records nothing can't pass.
    let base = Case {
        p010: false,
        present: true,
        nv12_export: false,
        p010_export: false,
        answer: false,
        writes: false,
        fail: 0,
        alias: false,
        start_set: true,
        width: 4,
        height: 2,
        flags: 2,
    };
    let fallback = compare(&base);
    assert_eq!(
        (fallback.result, fallback.outputs, fallback.log.as_str()),
        (
            true,
            [1, 2],
            "tex_create 4 2 f2 l1 null 0x2;tex_create 2 1 f18 l1 null 0x2"
        )
    );
    let odd = compare(&Case {
        width: 3,
        p010: true,
        ..base
    });
    assert_eq!(
        (odd.result, odd.outputs, odd.log.as_str()),
        (
            false,
            [-3, -3],
            "log 100 P010 textures must have dimensions divisible by 2."
        )
    );
    let absent = compare(&Case {
        present: false,
        ..base
    });
    assert_eq!(
        absent.log,
        "log 400 gs_texture_create_nv12: called while not in a graphics context"
    );
    let native = compare(&Case {
        nv12_export: true,
        answer: true,
        writes: true,
        ..base
    });
    assert_eq!(
        (native.result, native.outputs, native.log.as_str()),
        (true, [20000, 20001], "native nv12 dev:1 4 2 0x2")
    );
    let failed = compare(&Case { fail: 2, ..base });
    assert_eq!(
        (failed.result, failed.outputs, failed.log.as_str()),
        (
            false,
            [-1, -1],
            "tex_create 4 2 f2 l1 null 0x2;tex_create 2 1 f18 l1 null 0x2;tex_destroy 1"
        )
    );

    let mut runs = 0usize;
    let percent =
        obs_c_oracle::parity::percent_at(std::env::var(obs_c_oracle::parity::ENV).ok().as_deref());
    let step = usize::try_from(100 / percent.max(1)).unwrap_or(1);
    for case in grid().step_by(step) {
        compare(&case);
        runs += 1;
    }
    assert!(runs > 0);

    proptest::test_runner::TestRunner::new(ProptestConfig::with_cases(
        obs_c_oracle::parity::cases(4096),
    ))
    .run(&case(), |case| {
        compare(&case);
        Ok(())
    })
    .unwrap();
}

#[test]
fn planar_creators_match_original_c() {
    const CHILD: &str = "OBS_TEXTURE_PLANAR_PARITY_CHILD";
    if std::env::var_os(CHILD).is_some() {
        parity();
        return;
    }
    // The stand-in device and log handler are process-wide: re-exec this
    // exact case alone, in this same test executable.
    let name = format!("{}::planar_creators_match_original_c", module_path!());
    let name = name.split_once("::").unwrap().1;
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("running 1 test"), "{stdout}");
    assert!(
        output.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
