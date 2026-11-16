//! Tier 3: the Rust cull and colour calls against the original C
//! (`raster-state-entrypoints.c.h`, compiled as `oracle_gs_*` with its own
//! copy of `gs_valid`) over the real private graphics layout, a stand-in
//! context, a recording device and a recording `blog`. Both sides see the
//! same context, device and cull answer; the test compares the device calls
//! (with their device and arguments), the results and the log lines, call by
//! call. Inputs: random cull modes and cull answers (outside the enum too),
//! colour masks, both devices, and no context. No intentional differences.
use obs_c_oracle as _;
use obs_graphics::ffi::raster_entrypoints as rust;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct Event {
    call: c_int,
    device: *const c_void,
    cull_mode: c_int,
    rgba: [bool; 4],
}

const NO_EVENT: Event = Event {
    call: -1,
    device: ptr::null(),
    cull_mode: 0,
    rgba: [false; 4],
};

unsafe extern "C" {
    fn oracle_gs_set_cull_mode(mode: c_int);
    fn oracle_gs_get_cull_mode() -> c_int;
    fn oracle_gs_enable_color(red: bool, green: bool, blue: bool, alpha: bool);
    fn oracle_raster_reset(context: bool, device: *mut c_void, cull_answer: c_int);
    fn oracle_raster_events(out: *mut Event, max: usize) -> usize;
    fn oracle_raster_logs() -> u32;
    fn oracle_raster_level() -> c_int;
    fn oracle_raster_log() -> *const c_char;
}

static DEVICES: [u8; 2] = [0; 2];

fn device(index: usize) -> *mut c_void {
    ptr::from_ref(&DEVICES[index]).cast_mut().cast()
}

#[derive(Clone, Copy, Debug)]
enum Op {
    SetCullMode(i32),
    GetCullMode,
    EnableColor([bool; 4]),
}

#[derive(Debug, PartialEq)]
struct Observed {
    cull: Option<i32>,
    events: Vec<Event>,
    logs: u32,
    level: c_int,
    log: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct Step {
    context: bool,
    device: usize,
    cull_answer: i32,
    op: Op,
}

fn run(use_rust: bool, step: Step) -> Observed {
    // SAFETY: the stand-in context is this thread's and is reset before the
    // call; the device pointer is a static the device only records.
    unsafe {
        oracle_raster_reset(step.context, device(step.device), step.cull_answer);
        let mut cull = None;
        match (use_rust, step.op) {
            (true, Op::SetCullMode(m)) => rust::gs_set_cull_mode(m),
            (false, Op::SetCullMode(m)) => oracle_gs_set_cull_mode(m),
            (true, Op::GetCullMode) => cull = Some(rust::gs_get_cull_mode()),
            (false, Op::GetCullMode) => cull = Some(oracle_gs_get_cull_mode()),
            (true, Op::EnableColor([r, g, b, a])) => rust::gs_enable_color(r, g, b, a),
            (false, Op::EnableColor([r, g, b, a])) => oracle_gs_enable_color(r, g, b, a),
        }
        let mut events = [NO_EVENT; 4];
        let n = oracle_raster_events(events.as_mut_ptr(), events.len());
        Observed {
            cull,
            events: events[..n].to_vec(),
            logs: oracle_raster_logs(),
            level: oracle_raster_level(),
            log: CStr::from_ptr(oracle_raster_log()).to_bytes().to_vec(),
        }
    }
}

fn name(op: Op) -> &'static str {
    match op {
        Op::SetCullMode(_) => "gs_set_cull_mode",
        Op::GetCullMode => "gs_get_cull_mode",
        Op::EnableColor(_) => "gs_enable_color",
    }
}

fn compare(step: Step) {
    let original = run(false, step);
    let ported = run(true, step);
    assert_eq!(ported, original, "{step:?}");
    // And what the C does, so a fault common to both sides shows too.
    if step.context {
        assert_eq!(ported.logs, 0);
        assert_eq!(ported.events.len(), 1);
        assert_eq!(ported.events[0].device, device(step.device).cast_const());
        match step.op {
            Op::SetCullMode(m) => {
                assert_eq!((ported.events[0].call, ported.events[0].cull_mode), (0, m))
            }
            Op::GetCullMode => assert_eq!(ported.events[0].call, 1),
            Op::EnableColor(rgba) => {
                assert_eq!((ported.events[0].call, ported.events[0].rgba), (2, rgba))
            }
        }
    } else {
        assert!(ported.events.is_empty());
        assert_eq!((ported.logs, ported.level), (1, 400));
        let line = format!("{}: called while not in a graphics context", name(step.op));
        assert_eq!(ported.log, line.as_bytes());
    }
    if let Op::GetCullMode = step.op {
        // GS_NEITHER without a context; the device's answer with one.
        assert_eq!(
            ported.cull,
            Some(if step.context { step.cull_answer } else { 2 })
        );
    }
}

fn edge_ops() -> Vec<Op> {
    let mut ops = vec![Op::GetCullMode];
    for mode in [0, 1, 2, 3, 77, -1, i32::MIN, i32::MAX] {
        ops.push(Op::SetCullMode(mode));
    }
    for mask in 0..16u8 {
        ops.push(Op::EnableColor([
            mask & 1 != 0,
            mask & 2 != 0,
            mask & 4 != 0,
            mask & 8 != 0,
        ]));
    }
    ops
}

#[test]
fn characterized_matrix_matches_c() {
    for op in edge_ops() {
        for context in [false, true] {
            for device in 0..2 {
                for cull_answer in [0, 1, 2, 9, -1, i32::MIN, i32::MAX] {
                    compare(Step {
                        context,
                        device,
                        cull_answer,
                        op,
                    });
                }
            }
        }
    }
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        prop_oneof![0i32..=2, any::<i32>()].prop_map(Op::SetCullMode),
        Just(Op::GetCullMode),
        any::<[bool; 4]>().prop_map(Op::EnableColor),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default()
    })]
    #[test]
    fn random_sequences_match_c(
        steps in prop::collection::vec(
            (any::<bool>(), 0usize..2, prop_oneof![0i32..=2, any::<i32>()], op()),
            1..=16,
        ),
    ) {
        for (context, device, cull_answer, op) in steps {
            compare(Step { context, device, cull_answer, op });
        }
    }
}
