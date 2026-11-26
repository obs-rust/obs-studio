//! Tier 3: the original C `obs_source_get_color_space()` (with the original
//! C routing) and the Rust export, on noise-filled real-header sources
//! (oracle/source_get_color_space.c). Compares the answer, every callback
//! call (which source, the count, whether it got the caller's list), the log
//! lines and every byte of the sources.
//!
//! Excluded inputs: a cycle of filter parents/targets (the C recurses
//! without end), and a NULL list with a count above 0 on an async pick (the
//! C reads through NULL; the Rust pick reads an empty list, the documented
//! difference of `ffi::source_color_space`).
use core::ffi::{CStr, c_char, c_int, c_void};
use obs_source::ffi::source_get_color_space::obs_source_get_color_space;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

const NODES: usize = 4;
const LIST: usize = 4;
const CALLS: usize = 8;

type GetFn = unsafe extern "C" fn(*mut c_void, usize, *const c_int) -> c_int;
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Case {
    start: i8,
    noise: u8,
    count: u8,
    null_list: bool,
    list: [i32; LIST],
    source_type: [i32; NODES],
    flags: [u32; NODES],
    has_data: [bool; NODES],
    enabled: [bool; NODES],
    parent: [i8; NODES],
    target: [i8; NODES],
    format: [i32; NODES],
    trc: [i32; NODES],
    has_callback: [bool; NODES],
    answer: [i32; NODES],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    returned: i32,
    calls: u32,
    call_source: [i32; CALLS],
    call_count: [u64; CALLS],
    call_list: [bool; CALLS],
    log_count: u32,
    log_level: i32,
    log: [c_char; 160],
    untouched: bool,
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            returned: 0,
            calls: 0,
            call_source: [0; CALLS],
            call_count: [0; CALLS],
            call_list: [false; CALLS],
            log_count: 0,
            log_level: 0,
            log: [0; 160],
            untouched: false,
        }
    }
}

impl Outcome {
    fn log(&self) -> &str {
        // SAFETY: vsnprintf NUL-terminates within the buffer.
        unsafe { CStr::from_ptr(self.log.as_ptr()) }
            .to_str()
            .unwrap()
    }
}

unsafe extern "C" {
    fn oracle_source_get_color_space_original() -> GetFn;
    fn oracle_source_get_color_space_run(
        get_fn: GetFn,
        get: GetLog,
        set: SetLog,
        case: *const Case,
        out: *mut Outcome,
    ) -> bool;
}

fn run(get_fn: GetFn, case: &Case) -> Outcome {
    let mut out = Outcome::default();
    // SAFETY: the isolated child owns the log handler; the case and the
    // outcome are valid for the call.
    let valid = unsafe {
        oracle_source_get_color_space_run(
            get_fn,
            base_get_log_handler,
            base_set_log_handler,
            case,
            &mut out,
        )
    };
    assert!(valid, "the oracle fixture failed: {case:?}");
    out
}

fn check(case: &Case) -> Outcome {
    // SAFETY: returns a function pointer.
    let original = run(unsafe { oracle_source_get_color_space_original() }, case);
    let ported = run(obs_source_get_color_space, case);
    assert_eq!(ported, original, "{case:?}");
    assert!(original.untouched, "{case:?}");
    original
}

/// A node that answers through its callback.
fn plain(case: &mut Case, i: usize, flags: u32) {
    case.source_type[i] = 0;
    case.flags[i] = flags;
    case.has_data[i] = true;
    case.enabled[i] = true;
    case.parent[i] = -1;
    case.target[i] = -1;
    case.has_callback[i] = true;
}

fn base() -> Case {
    let mut c = Case {
        count: 2,
        list: [1, 2, 3, 0],
        ..Case::default()
    };
    for i in 0..NODES {
        plain(&mut c, i, 1);
    }
    c
}

/// Any case whose links point to higher indices only (no cycle), with the
/// async-pick NULL-list difference left out.
fn arb_case() -> impl Strategy<Value = Case> {
    (
        (
            -1i8..NODES as i8,
            any::<u8>(),
            0u8..=LIST as u8,
            any::<bool>(),
        ),
        proptest::array::uniform4(-1i32..5),
        proptest::array::uniform4((
            prop_oneof![Just(0), Just(1), Just(2), Just(3), -1i32..5],
            prop_oneof![0u32..8, any::<u32>()],
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
        )),
        proptest::array::uniform4((0u8..=NODES as u8, 0u8..=NODES as u8, 0i32..30, 0i32..4)),
    )
        .prop_map(|((start, noise, count, null_list), list, nodes, links)| {
            let mut c = Case {
                start,
                noise,
                count,
                null_list,
                list,
                ..Case::default()
            };
            for i in 0..NODES {
                let (t, f, d, e, cb) = nodes[i];
                let (p, tg, fmt, trc) = links[i];
                c.source_type[i] = t;
                c.flags[i] = f;
                c.has_data[i] = d;
                c.enabled[i] = e;
                c.has_callback[i] = cb;
                c.format[i] = fmt;
                c.trc[i] = trc;
                // only to a higher index, else NULL
                let link = |v: u8| {
                    if usize::from(v) > i && usize::from(v) < NODES {
                        v as i8
                    } else {
                        -1
                    }
                };
                c.parent[i] = link(p);
                c.target[i] = link(tg);
            }
            if c.null_list {
                // keep the NULL-list difference out: count 0 with NULL.
                c.count = 0;
            }
            c
        })
}

#[test]
fn get_color_space_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::get_color_space_matches_c")) {
        return;
    }
    pinned();
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(2048),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(&arb_case(), |c| {
            check(&c);
            Ok(())
        })
        .unwrap();
}

fn pinned() {
    // NULL: one LOG_DEBUG line, sRGB.
    let mut c = base();
    c.start = -1;
    let o = check(&c);
    assert_eq!(
        (o.returned, o.calls, o.log_count, o.log_level),
        (0, 0, 1, 400)
    );
    assert_eq!(
        o.log(),
        "obs_source_get_color_space: Null 'source' parameter"
    );

    // The callback answers (index + 100) with the caller's list and count.
    let c = base();
    let o = check(&c);
    assert_eq!(
        (
            o.returned,
            o.calls,
            o.call_source[0],
            o.call_count[0],
            o.call_list[0]
        ),
        (100, 1, 0, 2, true)
    );

    // No callback: sRGB.
    let mut c = base();
    c.has_callback[0] = false;
    assert_eq!(check(&c).returned, 0);

    // No data: sRGB, nothing called.
    let mut c = base();
    c.has_data[0] = false;
    let o = check(&c);
    assert_eq!((o.returned, o.calls, o.log_count), (0, 0, 0));

    // An audio filter (type 1, no VIDEO) asks its target from the top.
    let mut c = base();
    c.source_type[0] = 1;
    c.flags[0] = 2;
    c.target[0] = 2;
    let o = check(&c);
    assert_eq!((o.returned, o.call_source[0]), (102, 2));

    // An audio-only input asks its filter parent.
    let mut c = base();
    c.flags[0] = 2;
    c.parent[0] = 3;
    assert_eq!(check(&c).returned, 103);

    // An async source picks: VIDEO_FORMAT_P010 (18) with PQ (2) is
    // GS_CS_709_EXTENDED (2), found in [1, 2].
    let mut c = base();
    c.flags[0] = 1 | 4;
    c.format[0] = 18;
    c.trc[0] = 2;
    let o = check(&c);
    assert_eq!((o.returned, o.calls), (2, 0));
}
