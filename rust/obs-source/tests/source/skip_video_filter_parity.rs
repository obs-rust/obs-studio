//! Tier 3: the original C `obs_source_skip_video_filter()` and the Rust
//! export, on noise-filled real-header sources
//! (oracle/source_skip_video_filter.c). Both reach the same recording
//! stand-ins for the target and parent lookups and the five draws; the Rust
//! side reaches them through the production adapters, which read the real
//! fields. Compares every call in order with its node, the log line and
//! every byte of the sources, over a grid of every input, plus a proptest;
//! and rejects mutants. The field reads are not traced (they change
//! nothing), so their order is checked by Tier 1 alone. No intentional
//! differences.
use core::ffi::{CStr, c_char, c_void};
use obs_source::ffi::source_skip_video_filter::{Libobs, obs_source_skip_video_filter};
use obs_source::render_route::{FLAG_ASYNC, FLAG_CUSTOM_DRAW, FLAG_VIDEO};
use obs_source::skip_video_filter::Skip;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

const NODES: usize = 3;
const EVENTS: usize = 8;
const NULL: i64 = 0xfe;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Case {
    filter: i8,
    target: i8,
    parent: i8,
    noise: u8,
    flags: [u32; NODES],
    video_render: [bool; NODES],
    deinterlace_mode: [i32; NODES],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    events: u32,
    trace: [[i64; 2]; EVENTS],
    log_count: u32,
    log_level: i32,
    log: [c_char; 128],
    untouched: bool,
}

impl Default for Outcome {
    fn default() -> Self {
        Self {
            events: 0,
            trace: [[0; 2]; EVENTS],
            log_count: 0,
            log_level: 0,
            log: [0; 128],
            untouched: false,
        }
    }
}

impl Outcome {
    fn log(&self) -> &str {
        // SAFETY: the oracle wrote a NUL-terminated string (or left zeros).
        unsafe { CStr::from_ptr(self.log.as_ptr()) }
            .to_str()
            .unwrap()
    }

    /// The calls as kind and node, in order: "T0 P0 D1".
    fn calls(&self) -> String {
        self.trace
            .iter()
            .take(usize::try_from(self.events).unwrap())
            .map(|e| {
                let kind = char::from(u8::try_from(e[0]).unwrap());
                if e[1] == NULL {
                    format!("{kind}-")
                } else {
                    format!("{kind}{}", e[1])
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

type SkipFn = unsafe extern "C" fn(*mut c_void);
type GetLog = unsafe extern "C" fn(*mut *mut c_void, *mut *mut c_void);
type SetLog = unsafe extern "C" fn(*mut c_void, *mut c_void);

unsafe extern "C" {
    fn oracle_source_skip_video_filter_original() -> SkipFn;
    fn oracle_source_skip_video_filter_run(
        skip: SkipFn,
        get: GetLog,
        set: SetLog,
        case: *const Case,
        out: *mut Outcome,
    ) -> bool;
}

/// Runs one side; panics when the oracle could not build the fixture or an
/// adapter does not reach its field.
fn run(skip: SkipFn, case: &Case) -> Outcome {
    let mut out = Outcome::default();
    // SAFETY: the isolated child owns the log handler; the case and the
    // outcome are valid for the call.
    let valid = unsafe {
        oracle_source_skip_video_filter_run(
            skip,
            base_get_log_handler,
            base_set_log_handler,
            case,
            &mut out,
        )
    };
    assert!(valid, "the oracle fixture or an adapter failed: {case:?}");
    out
}

/// The original's outcome, when `skip` agrees with it.
fn check_with(skip: SkipFn, case: &Case) -> Option<Outcome> {
    // SAFETY: returns the oracle's original function.
    let original = run(unsafe { oracle_source_skip_video_filter_original() }, case);
    let ported = run(skip, case);
    (ported == original).then_some(original)
}

fn check(case: &Case) -> Outcome {
    check_with(obs_source_skip_video_filter, case)
        .unwrap_or_else(|| panic!("the Rust export disagrees with the original C: {case:?}"))
}

/// Node 0 is a filter attached straight to node 1, a plain video source;
/// node 2 is another filter.
fn base() -> Case {
    Case {
        filter: 0,
        target: 1,
        parent: 1,
        noise: 0xa5,
        flags: [FLAG_VIDEO; NODES],
        video_render: [true; NODES],
        deinterlace_mode: [0; NODES],
    }
}

// --- mutants ------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Change {
    NoLog,
    ParentFirst,
    TargetOnly,
    AsyncNotDeinterlace,
    MainAlways,
    DrawParent,
}

fn mutant(change: Change, filter: *mut c_void) {
    let mut d = Libobs;
    // Rebuild the core's steps with one change, through the real adapters.
    let Some(filter) = core::ptr::NonNull::new(filter) else {
        if change != Change::NoLog {
            d.log_null_filter();
        }
        return;
    };
    let filter = source(filter);
    let (target, parent) = if change == Change::ParentFirst {
        let parent = d.parent(filter);
        (d.target(filter), parent)
    } else {
        (d.target(filter), d.parent(filter))
    };
    let Some(target) = target else { return };
    let parent = match (parent, change) {
        (Some(parent), _) => parent,
        (None, Change::TargetOnly) => target,
        (None, _) => return,
    };
    let flags = d.output_flags(parent);
    let drawn = if change == Change::DrawParent {
        parent
    } else {
        target
    };
    if target != parent {
        d.video_render(drawn);
    } else if flags & (FLAG_ASYNC | FLAG_CUSTOM_DRAW) == 0 {
        d.default_render(drawn);
    } else if change == Change::MainAlways || d.has_video_render(target) {
        d.main_render(drawn);
    } else if change != Change::AsyncNotDeinterlace && d.deinterlacing(target) {
        d.deinterlace_render(drawn);
    } else {
        d.async_video(drawn);
    }
}

/// A non-NULL C source as the shim's source type.
fn source(ptr: core::ptr::NonNull<c_void>) -> <Libobs as Skip>::Source {
    obs_source::ffi::source_skip_video_filter::Source::from_raw(ptr)
}

macro_rules! mutant {
    ($name:ident, $change:expr) => {
        unsafe extern "C" fn $name(filter: *mut c_void) {
            mutant($change, filter);
        }
    };
}

mutant!(m_no_log, Change::NoLog);
mutant!(m_parent_first, Change::ParentFirst);
mutant!(m_target_only, Change::TargetOnly);
mutant!(m_async, Change::AsyncNotDeinterlace);
mutant!(m_main, Change::MainAlways);
mutant!(m_draw_parent, Change::DrawParent);

// --- cases --------------------------------------------------------------

fn pinned() {
    // A plain parent: target, parent, then the default render of node 1.
    let out = check(&base());
    assert_eq!(out.calls(), "T0 P0 D1");
    assert_eq!(out.log_count, 0);
    assert!(out.untouched);

    // NULL: one LOG_DEBUG line, nothing called.
    let mut c = base();
    c.filter = -1;
    let out = check(&c);
    assert_eq!((out.events, out.log_count, out.log_level), (0, 1, 400));
    assert_eq!(
        out.log(),
        "obs_source_skip_video_filter: Null 'filter' parameter"
    );

    // Removed: both looked up, nothing drawn.
    for (target, parent) in [(-1, -1), (-1, 1), (1, -1)] {
        let mut c = base();
        c.target = target;
        c.parent = parent;
        assert_eq!(check(&c).calls(), "T0 P0");
    }

    // Another filter below: its render.
    let mut c = base();
    c.target = 2;
    assert_eq!(check(&c).calls(), "T0 P0 V2");

    // Custom draw or async: main render with a video_render, else the
    // deinterlacer or the async frame.
    for flags in [FLAG_CUSTOM_DRAW, FLAG_ASYNC] {
        let mut c = base();
        c.flags[1] = flags;
        assert_eq!(check(&c).calls(), "T0 P0 M1");
        c.video_render[1] = false;
        assert_eq!(check(&c).calls(), "T0 P0 A1");
        c.deinterlace_mode[1] = 1;
        assert_eq!(check(&c).calls(), "T0 P0 I1");
    }
}

fn grid() -> Vec<Case> {
    let mut cases = Vec::new();
    for filter in [-1i8, 0] {
        for target in [-1i8, 1, 2] {
            for parent in [-1i8, 1, 2] {
                for flags in [
                    0,
                    FLAG_VIDEO,
                    FLAG_ASYNC,
                    FLAG_CUSTOM_DRAW,
                    FLAG_ASYNC | FLAG_CUSTOM_DRAW,
                    !(FLAG_ASYNC | FLAG_CUSTOM_DRAW),
                    u32::MAX,
                ] {
                    for video_render in [false, true] {
                        for mode in [0i32, 1, 6, -1] {
                            let mut c = base();
                            c.filter = filter;
                            c.target = target;
                            c.parent = parent;
                            for node in 1..NODES {
                                c.flags[node] = flags;
                                c.video_render[node] = video_render;
                                c.deinterlace_mode[node] = mode;
                            }
                            cases.push(c);
                        }
                    }
                }
            }
        }
    }
    cases
}

fn grid_and_mutants() {
    let cases = grid();
    for c in &cases {
        let out = check(c);
        assert!(out.untouched);
        assert!(out.log_count <= 1);
    }
    let mutants: [(&str, SkipFn); 6] = [
        ("no NULL line", m_no_log),
        ("parent looked up first", m_parent_first),
        ("no parent check", m_target_only),
        ("async instead of deinterlace", m_async),
        ("main render without video_render", m_main),
        ("draws the parent", m_draw_parent),
    ];
    for (name, skip) in mutants {
        assert!(
            cases.iter().any(|c| check_with(skip, c).is_none()),
            "the mutant '{name}' agreed with the original C"
        );
    }
}

fn random_cases() {
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(512),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(
            &(
                (-1i8..1, -1i8..3, -1i8..3, any::<u8>()),
                proptest::array::uniform3(any::<u32>()),
                proptest::array::uniform3(any::<bool>()),
                proptest::array::uniform3(any::<i32>()),
            ),
            |((filter, target, parent, noise), flags, video_render, deinterlace_mode)| {
                let c = Case {
                    filter,
                    target,
                    parent,
                    noise,
                    flags,
                    video_render,
                    deinterlace_mode,
                };
                let out = check(&c);
                prop_assert!(out.untouched);
                prop_assert!(out.log_count <= 1);
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn skip_video_filter_matches_c() {
    if !crate::output::isolated(concat!(module_path!(), "::skip_video_filter_matches_c")) {
        return;
    }
    pinned();
    grid_and_mutants();
    random_cases();
}
