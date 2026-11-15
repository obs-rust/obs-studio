//! Tier 3: the Rust `gs_perspective` against the original C
//! (`graphics-perspective.c.h`, compiled as `oracle_gs_perspective` with its
//! own copy of `gs_valid`) over the real private graphics layout, a stand-in
//! context, a recording `device_frustum` and a recording `blog`. Both sides
//! see the same context and device; the test compares the device calls (the
//! device and the six float arguments as bit patterns), the log count, level
//! and line, call by call, and the safe core's frustum. Inputs: the C
//! characterization's cases, signed zeros, infinities, NaNs, subnormals,
//! huge and tiny values, and random bit patterns; both devices and no
//! context. No intentional differences. A NaN argument compares as "NaN",
//! not by its bits: when two NaNs meet in a multiply, which one's sign and
//! payload survive depends on the operand order the compiler emits, which
//! neither C nor Rust specifies (the debug build's `ymax * aspect` keeps the
//! C's first operand on GCC and the second on LLVM). Every other value
//! compares bit for bit, signed zeros included.
use super::perspective::{CASES, Fixture};
use obs_c_oracle as _;
use obs_graphics::ffi::perspective as rust;
use obs_graphics::perspective;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Event {
    device: *const c_void,
    args: [f32; 6],
}

const NO_EVENT: Event = Event {
    device: ptr::null(),
    args: [0.0; 6],
};

unsafe extern "C" {
    fn oracle_gs_perspective(angle: f32, aspect: f32, near: f32, far: f32);
    fn oracle_perspective_reset(context: bool, device: *mut c_void);
    fn oracle_perspective_events(out: *mut Event, max: usize) -> usize;
    fn oracle_perspective_logs() -> u32;
    fn oracle_perspective_level() -> c_int;
    fn oracle_perspective_log() -> *const c_char;
}

static DEVICES: [u8; 2] = [0; 2];

/// The argument's bits, or one marker for every NaN (see the module doc).
fn key(value: f32) -> u32 {
    if value.is_nan() {
        u32::MAX
    } else {
        value.to_bits()
    }
}

fn device(index: usize) -> *mut c_void {
    ptr::from_ref(&DEVICES[index]).cast_mut().cast()
}

#[derive(Debug, PartialEq)]
struct Observed {
    events: Vec<(*const c_void, [u32; 6])>,
    logs: u32,
    level: c_int,
    log: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct Step {
    context: bool,
    device: usize,
    args: [f32; 4],
}

fn run(use_rust: bool, step: Step) -> Observed {
    let [angle, aspect, near, far] = step.args;
    // SAFETY: the stand-in context is this thread's and is reset before the
    // call; the device pointer is a static the device only records.
    unsafe {
        oracle_perspective_reset(step.context, device(step.device));
        if use_rust {
            rust::gs_perspective(angle, aspect, near, far);
        } else {
            oracle_gs_perspective(angle, aspect, near, far);
        }
        let mut events = [NO_EVENT; 4];
        let n = oracle_perspective_events(events.as_mut_ptr(), events.len());
        Observed {
            events: events[..n]
                .iter()
                .map(|e| (e.device, e.args.map(key)))
                .collect(),
            logs: oracle_perspective_logs(),
            level: oracle_perspective_level(),
            log: CStr::from_ptr(oracle_perspective_log()).to_bytes().to_vec(),
        }
    }
}

fn compare(step: Step) {
    let original = run(false, step);
    let ported = run(true, step);
    assert_eq!(ported, original, "{step:?}");
    let [angle, aspect, near, far] = step.args;
    // The safe core over the model context agrees with both.
    let mut model = Fixture {
        device: step.context.then_some(step.device),
        ..Fixture::default()
    };
    perspective::perspective(&mut model, angle, aspect, near, far);
    assert_eq!(model.logs.len(), original.logs as usize);
    if step.context {
        assert_eq!(original.logs, 0);
        assert_eq!(
            original.events,
            [(
                device(step.device).cast_const(),
                perspective::frustum(angle, aspect, near, far)
                    .bits()
                    .map(|bits| key(f32::from_bits(bits)))
            )],
            "{step:?}"
        );
        let model_keys = model
            .events
            .iter()
            .map(|&(device, bits)| (device, bits.map(|b| key(f32::from_bits(b)))))
            .collect::<Vec<_>>();
        assert_eq!(model_keys, [(step.device, original.events[0].1)]);
    } else {
        assert!(original.events.is_empty());
        assert!(model.events.is_empty());
        assert_eq!((original.logs, original.level), (1, 400));
        assert_eq!(
            original.log,
            b"gs_perspective: called while not in a graphics context"
        );
        assert_eq!(model.logs[0].as_bytes(), original.log);
    }
}

fn edge_values() -> Vec<f32> {
    vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        45.0,
        90.0,
        180.0,
        -180.0,
        270.0,
        360.0,
        720.0,
        1.0e7,
        -1.0e7,
        f32::MAX,
        f32::MIN,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::from_bits(0x8000_0001),
        f32::EPSILON,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::from_bits(0xffc0_0001),
        f32::from_bits(0x7f80_0001),
    ]
}

#[test]
fn characterized_cases_match_c() {
    for context in [false, true] {
        for device in 0..2 {
            for args in CASES {
                compare(Step {
                    context,
                    device,
                    args,
                });
            }
        }
    }
}

#[test]
fn edge_values_match_c() {
    let values = edge_values();
    for &angle in &values {
        for &other in &values {
            for args in [
                [angle, 1.0, other, 100.0],
                [angle, other, 1.0, -other],
                [other, -1.5, angle, other],
            ] {
                compare(Step {
                    context: true,
                    device: 1,
                    args,
                });
            }
        }
    }
}

fn float() -> impl Strategy<Value = f32> {
    prop_oneof![
        -360.0f32..360.0,
        -1.0e3f32..1.0e3,
        any::<u32>().prop_map(f32::from_bits),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default()
    })]
    #[test]
    fn random_calls_match_c(
        steps in prop::collection::vec(
            (any::<bool>(), 0usize..2, [float(), float(), float(), float()]),
            1..=16,
        ),
    ) {
        for (context, device, args) in steps {
            compare(Step { context, device, args });
        }
    }
}
