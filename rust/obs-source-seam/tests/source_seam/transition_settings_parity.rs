//! Tier 3 for `obs_source::ffi::source_transition_settings` against the
//! original `libobs/obs-source-transition-settings.c.h`
//! (`obs_c_oracle::source_transition_settings`, renamed `oracle_*`).
//!
//! Both sides run on their own test transition (a zeroed `struct
//! obs_source` with a type, `transition_mutex` and up to two named
//! children, built by the oracle) from the same starting fields, and reach
//! them through the same C transition seam, as in the hybrid libobs. After
//! every call the two must agree on what it returned, on every settings
//! field (read through the seam), on the lock being free, and for
//! `obs_transition_save()` on the saved data's JSON, keys and order
//! included. The sources are transitions, inputs (not transitions) and,
//! for the eight functions that check, NULL; `obs_transition_is_active()`
//! and `obs_transition_save()` do not check, in C or Rust, so they get no
//! NULL.
//!
//! The one intentional difference: for a NULL source,
//! `obs_transition_get_size()` logs `"obs_transition_get_size: Null
//! 'transition' parameter"`, where the C names `obs_transition_set_size`.
//! [`null_source_logs`] compares every other NULL log line exactly and that
//! one with the name swapped; the other tests compare no logs.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

use obs_c_oracle::parity::cases;
use obs_c_oracle::source_transition_settings::{self as c, seam};
use obs_data::ffi::obs_data::{obs_data_create, obs_data_get_json, obs_data_release};
use obs_source::ffi::source_transition_settings as rs;
use obs_util::ffi::base::{base_get_log_handler, base_set_log_handler};
use proptest::prelude::*;

/// `OBS_SOURCE_TYPE_INPUT`.
const INPUT: c_int = 0;
/// `OBS_SOURCE_TYPE_TRANSITION`.
const TRANSITION: c_int = 2;

/// The fields a test transition starts with.
#[derive(Debug, Clone)]
struct Init {
    source_type: c_int,
    child_a: c_int,
    child_b: c_int,
    video: bool,
    audio: bool,
    mode: c_int,
    scale_type: c_int,
    alignment: u32,
    cx: u32,
    cy: u32,
    fixed: bool,
    duration: u32,
}

impl Init {
    fn transition() -> Self {
        Self {
            source_type: TRANSITION,
            child_a: 1,
            child_b: 1,
            video: false,
            audio: false,
            mode: 0,
            scale_type: 0,
            alignment: 5,
            cx: 0,
            cy: 0,
            fixed: false,
            duration: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Op {
    IsActive,
    SetScaleType(c_int),
    GetScaleType,
    SetAlignment(u32),
    GetAlignment,
    SetSize(u32, u32),
    GetSize,
    Save,
    EnableFixed(bool, u32),
    Fixed,
    /// Not a settings call: what `obs_transition_start()` and the stops set,
    /// written through the seam on both sides.
    Flags(bool, bool),
}

/// The two test transitions, C's and Rust's.
struct Pair {
    c: *mut c_void,
    rs: *mut c_void,
}

fn new_transition(init: &Init) -> *mut c_void {
    // SAFETY: the oracle's constructor; the seam setters on its live source.
    unsafe {
        let tr = c::oracle_transition_settings_new(
            init.source_type,
            init.child_a,
            c"child a".as_ptr(),
            init.child_b,
            c"child b".as_ptr(),
        );
        seam::obs_source_transition_seam_set_transitioning_video(tr, init.video);
        seam::obs_source_transition_seam_set_transitioning_audio(tr, init.audio);
        seam::obs_source_transition_seam_set_mode(tr, init.mode);
        seam::obs_source_transition_seam_set_scale_type(tr, init.scale_type);
        seam::obs_source_transition_seam_set_alignment(tr, init.alignment);
        seam::obs_source_transition_seam_set_cx(tr, init.cx);
        seam::obs_source_transition_seam_set_cy(tr, init.cy);
        seam::obs_source_transition_seam_set_use_fixed_duration(tr, init.fixed);
        seam::obs_source_transition_seam_set_fixed_duration(tr, init.duration);
        tr
    }
}

impl Pair {
    fn new(init: &Init) -> Self {
        Self {
            c: new_transition(init),
            rs: new_transition(init),
        }
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        // SAFETY: both were made by the oracle and are freed once.
        unsafe {
            c::oracle_transition_settings_free(self.c);
            c::oracle_transition_settings_free(self.rs);
        }
    }
}

/// Every settings field, through the seam, and whether the lock is free.
fn fields(tr: *mut c_void) -> (c_int, c_int, u32, u32, u32, bool, u32, bool) {
    // SAFETY: a live test transition; a trylock that succeeds is undone.
    unsafe {
        let free = seam::obs_source_transition_seam_trylock(tr) == 0;
        if free {
            seam::obs_source_transition_seam_unlock(tr);
        }
        (
            seam::obs_source_transition_seam_get_mode(tr),
            seam::obs_source_transition_seam_get_scale_type(tr),
            seam::obs_source_transition_seam_get_alignment(tr),
            seam::obs_source_transition_seam_get_cx(tr),
            seam::obs_source_transition_seam_get_cy(tr),
            seam::obs_source_transition_seam_get_use_fixed_duration(tr),
            seam::obs_source_transition_seam_get_fixed_duration(tr),
            free,
        )
    }
}

/// The JSON `save` writes into a new `obs_data_t`.
fn saved(save: unsafe extern "C" fn(*mut c_void, *mut c_void), tr: *mut c_void) -> String {
    // SAFETY: a live transition and a new data object, released here.
    unsafe {
        let data = obs_data_create();
        save(tr, data);
        let json = CStr::from_ptr(obs_data_get_json(data))
            .to_string_lossy()
            .into_owned();
        obs_data_release(data);
        json
    }
}

/// What one call gave: its return value or outputs.
#[derive(Debug, PartialEq, Eq)]
enum Out {
    None,
    Bool(bool),
    Int(c_int),
    U32(u32),
    Size(u32, u32),
    Json(String),
}

/// Runs `op` on `tr` with C's functions (`c_side`) or Rust's.
fn call(op: Op, tr: *mut c_void, c_side: bool) -> Out {
    // SAFETY: `tr` is NULL only for the eight functions that check it (see
    // `null_ops`); every other call gets a live test transition.
    unsafe {
        match op {
            Op::IsActive => Out::Bool(if c_side {
                c::oracle_obs_transition_is_active(tr)
            } else {
                rs::obs_transition_is_active(tr)
            }),
            Op::SetScaleType(t) => {
                if c_side {
                    c::oracle_obs_transition_set_scale_type(tr, t);
                } else {
                    rs::obs_transition_set_scale_type(tr, t);
                }
                Out::None
            }
            Op::GetScaleType => Out::Int(if c_side {
                c::oracle_obs_transition_get_scale_type(tr)
            } else {
                rs::obs_transition_get_scale_type(tr)
            }),
            Op::SetAlignment(a) => {
                if c_side {
                    c::oracle_obs_transition_set_alignment(tr, a);
                } else {
                    rs::obs_transition_set_alignment(tr, a);
                }
                Out::None
            }
            Op::GetAlignment => Out::U32(if c_side {
                c::oracle_obs_transition_get_alignment(tr)
            } else {
                rs::obs_transition_get_alignment(tr)
            }),
            Op::SetSize(cx, cy) => {
                if c_side {
                    c::oracle_obs_transition_set_size(tr, cx, cy);
                } else {
                    rs::obs_transition_set_size(tr, cx, cy);
                }
                Out::None
            }
            Op::GetSize => {
                // a guard fill, so an output left unwritten shows
                let (mut cx, mut cy) = (0xdead_beef_u32, 0xfeed_face_u32);
                if c_side {
                    c::oracle_obs_transition_get_size(tr, &raw mut cx, &raw mut cy);
                } else {
                    rs::obs_transition_get_size(tr, &raw mut cx, &raw mut cy);
                }
                Out::Size(cx, cy)
            }
            Op::Save => Out::Json(if c_side {
                saved(c::oracle_obs_transition_save, tr)
            } else {
                saved(rs::obs_transition_save, tr)
            }),
            Op::EnableFixed(enable, duration) => {
                if c_side {
                    c::oracle_obs_transition_enable_fixed(tr, enable, duration);
                } else {
                    rs::obs_transition_enable_fixed(tr, enable, duration);
                }
                Out::None
            }
            Op::Fixed => Out::Bool(if c_side {
                c::oracle_obs_transition_fixed(tr)
            } else {
                rs::obs_transition_fixed(tr)
            }),
            Op::Flags(video, audio) => {
                seam::obs_source_transition_seam_set_transitioning_video(tr, video);
                seam::obs_source_transition_seam_set_transitioning_audio(tr, audio);
                Out::None
            }
        }
    }
}

/// Runs `ops` on both sides from `init`, comparing after every call.
fn run(init: &Init, ops: &[Op]) {
    let pair = Pair::new(init);
    assert_eq!(fields(pair.c), fields(pair.rs), "{init:?}: start");
    for (i, &op) in ops.iter().enumerate() {
        let c_out = call(op, pair.c, true);
        let rs_out = call(op, pair.rs, false);
        assert_eq!(rs_out, c_out, "{init:?}: op {i} {op:?} of {ops:?}");
        assert_eq!(
            fields(pair.rs),
            fields(pair.c),
            "{init:?}: fields after op {i} {op:?} of {ops:?}"
        );
    }
}

/// Every call once, with the getters after each setter.
fn every_op() -> Vec<Op> {
    vec![
        Op::IsActive,
        Op::GetScaleType,
        Op::GetAlignment,
        Op::GetSize,
        Op::Fixed,
        Op::Save,
        Op::SetScaleType(1),
        Op::GetScaleType,
        Op::SetScaleType(2),
        Op::GetScaleType,
        Op::SetScaleType(3),
        Op::GetScaleType,
        Op::SetScaleType(-1),
        Op::GetScaleType,
        Op::SetScaleType(c_int::MAX),
        Op::GetScaleType,
        Op::SetAlignment(0),
        Op::GetAlignment,
        Op::SetAlignment(u32::MAX),
        Op::GetAlignment,
        Op::SetSize(1920, 1080),
        Op::GetSize,
        Op::SetSize(u32::MAX, 0),
        Op::GetSize,
        Op::Save,
        Op::EnableFixed(true, 300),
        Op::Fixed,
        Op::EnableFixed(false, u32::MAX),
        Op::Fixed,
        Op::Flags(true, false),
        Op::IsActive,
        Op::Save,
        Op::Flags(false, true),
        Op::IsActive,
        Op::Save,
        Op::Flags(true, true),
        Op::Save,
        Op::Flags(false, false),
        Op::IsActive,
        Op::Save,
    ]
}

/// Each function on a transition, on one with empty slots, on one whose
/// children have a NULL name, and in manual mode.
#[test]
fn every_call_on_transitions() {
    for (child_a, child_b) in [(1, 1), (0, 0), (0, 1), (1, 0), (2, 2), (2, 1)] {
        for mode in [0, 1, -1] {
            let init = Init {
                child_a,
                child_b,
                mode,
                ..Init::transition()
            };
            run(&init, &every_op());
        }
    }
}

/// A source that is not a transition: the getters give their defaults and
/// the setters write nothing; the save reads it anyway, as the C does.
#[test]
fn every_call_on_an_input() {
    for source_type in [INPUT, 1, 3, -1] {
        let init = Init {
            source_type,
            scale_type: 2,
            alignment: 9,
            cx: 640,
            cy: 480,
            fixed: true,
            duration: 77,
            ..Init::transition()
        };
        run(&init, &every_op());
    }
}

// ---------------------------------------------------------------------------
// NULL, and the log lines

#[repr(C)]
struct Rec {
    calls: c_int,
    level: c_int,
    message: [c_char; 256],
}

unsafe extern "C" {
    fn base_test_record_ptr() -> *mut c_void;
    fn base_test_record_only_this_thread();
}

/// The process-wide log handler, pointed at a record while it lives.
struct Capture {
    rec: Box<Rec>,
    saved: (*mut c_void, *mut c_void),
}

impl Capture {
    fn arm() -> Self {
        let mut saved = (ptr::null_mut(), ptr::null_mut());
        let mut rec = Box::new(Rec {
            calls: 0,
            level: -1,
            message: [0; 256],
        });
        // SAFETY: the record outlives the handler (restored in drop).
        unsafe {
            base_get_log_handler(&raw mut saved.0, &raw mut saved.1);
            base_test_record_only_this_thread();
            base_set_log_handler(base_test_record_ptr(), (&raw mut *rec).cast());
        }
        Self { rec, saved }
    }

    /// The calls since the last take, the level and text of the last line.
    fn take(&mut self) -> (c_int, c_int, String) {
        // SAFETY: the recorder NUL-terminates what it writes; the record
        // starts zeroed.
        let text = unsafe { CStr::from_ptr(self.rec.message.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        let line = (self.rec.calls, self.rec.level, text);
        self.rec.calls = 0;
        self.rec.level = -1;
        self.rec.message[0] = 0;
        line
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        // SAFETY: the handler read in `arm`.
        unsafe { base_set_log_handler(self.saved.0, self.saved.1) };
    }
}

/// `LOG_DEBUG`.
const LOG_DEBUG: c_int = 400;

/// The eight functions that check their source, with their C names.
fn null_ops() -> [(Op, &'static str); 8] {
    [
        (Op::SetScaleType(2), "obs_transition_set_scale_type"),
        (Op::GetScaleType, "obs_transition_get_scale_type"),
        (Op::SetAlignment(5), "obs_transition_set_alignment"),
        (Op::GetAlignment, "obs_transition_get_alignment"),
        (Op::SetSize(1, 2), "obs_transition_set_size"),
        (Op::GetSize, "obs_transition_get_size"),
        (Op::EnableFixed(true, 3), "obs_transition_enable_fixed"),
        (Op::Fixed, "obs_transition_fixed"),
    ]
}

/// A NULL source: the same results, and one debug line each, the same but
/// for the size getter's name (the intentional difference). Swaps the
/// process-wide log handler, so it runs in a child process.
#[test]
fn null_source_logs() {
    if !crate::output::isolated(concat!(module_path!(), "::null_source_logs")) {
        return;
    }
    let mut log = Capture::arm();
    for (op, name) in null_ops() {
        let c_out = call(op, ptr::null_mut(), true);
        let c_line = log.take();
        let rs_out = call(op, ptr::null_mut(), false);
        let rs_line = log.take();
        assert_eq!(rs_out, c_out, "{op:?}");
        let c_name = if name == "obs_transition_get_size" {
            // the C passes the setter's name
            "obs_transition_set_size"
        } else {
            name
        };
        assert_eq!(
            c_line,
            (
                1,
                LOG_DEBUG,
                format!("{c_name}: Null 'transition' parameter")
            ),
            "C {op:?}"
        );
        assert_eq!(
            rs_line,
            (1, LOG_DEBUG, format!("{name}: Null 'transition' parameter")),
            "Rust {op:?}"
        );
    }
}

/// A NULL source gives the same results (no logs compared), in-process.
#[test]
fn null_source_results() {
    for (op, _) in null_ops() {
        assert_eq!(
            call(op, ptr::null_mut(), false),
            call(op, ptr::null_mut(), true),
            "{op:?}"
        );
    }
}

/// A valid transition logs nothing on either side.
#[test]
fn valid_source_logs_nothing() {
    if !crate::output::isolated(concat!(module_path!(), "::valid_source_logs_nothing")) {
        return;
    }
    let mut log = Capture::arm();
    let pair = Pair::new(&Init::transition());
    for op in every_op() {
        call(op, pair.c, true);
        call(op, pair.rs, false);
        assert_eq!(log.take().0, 0, "{op:?}");
    }
}

// ---------------------------------------------------------------------------
// proptests

fn scale_type() -> impl Strategy<Value = c_int> {
    prop_oneof![0..3, Just(-1), Just(3), any::<c_int>()]
}

fn source_type() -> impl Strategy<Value = c_int> {
    prop_oneof![4 => Just(TRANSITION), 1 => Just(INPUT), 1 => any::<c_int>()]
}

fn init() -> impl Strategy<Value = Init> {
    (
        (source_type(), 0..3, 0..3, any::<bool>(), any::<bool>()),
        (
            prop_oneof![0..2, any::<c_int>()],
            scale_type(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<bool>(),
            any::<u32>(),
        ),
    )
        .prop_map(
            |(
                (source_type, child_a, child_b, video, audio),
                (mode, scale_type, alignment, cx, cy, fixed, duration),
            )| Init {
                source_type,
                child_a,
                child_b,
                video,
                audio,
                mode,
                scale_type,
                alignment,
                cx,
                cy,
                fixed,
                duration,
            },
        )
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        Just(Op::IsActive),
        scale_type().prop_map(Op::SetScaleType),
        Just(Op::GetScaleType),
        any::<u32>().prop_map(Op::SetAlignment),
        Just(Op::GetAlignment),
        (any::<u32>(), any::<u32>()).prop_map(|(cx, cy)| Op::SetSize(cx, cy)),
        Just(Op::GetSize),
        Just(Op::Save),
        (any::<bool>(), any::<u32>()).prop_map(|(e, d)| Op::EnableFixed(e, d)),
        Just(Op::Fixed),
        (any::<bool>(), any::<bool>()).prop_map(|(v, a)| Op::Flags(v, a)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(2000)))]

    /// Any starting fields and any sequence of up to 24 calls.
    #[test]
    fn any_sequence(init in init(), ops in prop::collection::vec(op(), 0..24)) {
        run(&init, &ops);
    }
}
