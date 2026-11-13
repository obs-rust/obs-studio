//! Tier 3: the Rust blend setters, push, pop and reset against the original
//! C (`state-blend-setters.c.h` and `state-blend-entrypoints.c.h`, compiled
//! as `oracle_gs_*` behind the ported group's context check), plus the safe
//! core over the model context. Both sides run over the real private layout,
//! a stand-in context, a recording device and a recording `blog`; the test
//! compares, call by call, the stored blend state, the saved stack, the
//! device calls (with their device, arguments and the state the device saw)
//! and the log lines. Inputs: random setter arguments (blend types and
//! operations outside the enums too), random push, pop and reset sequences
//! over random or carried-over states, both devices, and no context.
//! No intentional differences. C and Rust each own independent stack storage.
use super::blend_entrypoints::{Call, DEFAULT, Mock};
use obs_graphics::blend_entrypoints as core;
use obs_graphics::ffi::blend_entrypoints as ffi;
use obs_graphics::state_stacks::BlendState;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct Event {
    call: u32,
    device: *const c_void,
    args: [u32; 4],
    seen: BlendState,
}

unsafe extern "C" {
    fn oracle_blend_layout(index: u32) -> usize;
    fn oracle_gs_enable_blending(enable: bool);
    fn oracle_gs_blend_function(src: u32, dest: u32);
    fn oracle_gs_blend_function_separate(src_c: u32, dest_c: u32, src_a: u32, dest_a: u32);
    fn oracle_gs_blend_op(op: u32);
    fn oracle_gs_blend_state_push();
    fn oracle_gs_blend_state_pop();
    fn oracle_gs_reset_blend_state();
    fn oracle_blend_begin(present: bool, state: *const BlendState);
    fn oracle_blend_set(present: bool, state: *const BlendState);
    fn oracle_blend_device(device: *mut c_void);
    fn oracle_blend_clear_calls();
    fn oracle_blend_end();
    fn oracle_blend_count() -> u32;
    fn oracle_blend_event(index: u32) -> Event;
    fn oracle_blend_logs() -> u32;
    fn oracle_blend_level() -> c_int;
    fn oracle_blend_message() -> *const c_char;
    fn oracle_blend_depth() -> usize;
    fn oracle_blend_current() -> BlendState;
    fn oracle_blend_saved(index: usize) -> BlendState;
}

static DEVICES: [u8; 2] = [0; 2];

fn device(index: usize) -> *mut c_void {
    ptr::from_ref(&DEVICES[index]).cast_mut().cast()
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Push,
    Pop,
    Reset,
    Enable(bool),
    Function(u32, u32),
    Separate([u32; 4]),
    Operation(u32),
}

impl Op {
    fn name(self) -> &'static str {
        match self {
            Op::Push => "gs_blend_state_push",
            Op::Pop => "gs_blend_state_pop",
            Op::Reset => "gs_reset_blend_state",
            Op::Enable(_) => "gs_enable_blending",
            Op::Function(..) => "gs_blend_function",
            Op::Separate(_) => "gs_blend_function_separate",
            Op::Operation(_) => "gs_blend_op",
        }
    }
}

/// One call: the context's presence, the device, the state to put in place
/// first (`None` keeps what earlier calls left), and the call.
#[derive(Clone, Copy, Debug)]
struct Step {
    present: bool,
    device: usize,
    state: Option<BlendState>,
    op: Op,
}

#[derive(Debug, PartialEq)]
struct Snapshot {
    current: BlendState,
    stack: Vec<BlendState>,
    events: Vec<Event>,
    logs: u32,
    level: c_int,
    message: String,
}

fn native(rust: bool, sequence: &[Step]) -> Vec<Snapshot> {
    // SAFETY: the oracle owns thread-local real C layouts and bounded buffers.
    // Every call records at most three device calls; end frees the real C
    // darray. The device pointers are statics the device only records.
    unsafe {
        oracle_blend_begin(true, &DEFAULT);
        let mut result = Vec::new();
        for step in sequence {
            let state = step.state.unwrap_or_else(|| oracle_blend_current());
            oracle_blend_set(step.present, &state);
            oracle_blend_device(device(step.device));
            oracle_blend_clear_calls();
            match (rust, step.op) {
                (true, Op::Push) => ffi::gs_blend_state_push(),
                (true, Op::Pop) => ffi::gs_blend_state_pop(),
                (true, Op::Reset) => ffi::gs_reset_blend_state(),
                (true, Op::Enable(e)) => ffi::gs_enable_blending(e),
                (true, Op::Function(s, d)) => ffi::gs_blend_function(s, d),
                (true, Op::Separate([a, b, c, d])) => ffi::gs_blend_function_separate(a, b, c, d),
                (true, Op::Operation(o)) => ffi::gs_blend_op(o),
                (false, Op::Push) => oracle_gs_blend_state_push(),
                (false, Op::Pop) => oracle_gs_blend_state_pop(),
                (false, Op::Reset) => oracle_gs_reset_blend_state(),
                (false, Op::Enable(e)) => oracle_gs_enable_blending(e),
                (false, Op::Function(s, d)) => oracle_gs_blend_function(s, d),
                (false, Op::Separate([a, b, c, d])) => {
                    oracle_gs_blend_function_separate(a, b, c, d)
                }
                (false, Op::Operation(o)) => oracle_gs_blend_op(o),
            }
            result.push(Snapshot {
                current: oracle_blend_current(),
                stack: (0..oracle_blend_depth())
                    .map(|i| oracle_blend_saved(i))
                    .collect(),
                events: (0..oracle_blend_count())
                    .map(|i| oracle_blend_event(i))
                    .collect(),
                logs: oracle_blend_logs(),
                level: oracle_blend_level(),
                message: CStr::from_ptr(oracle_blend_message())
                    .to_string_lossy()
                    .into_owned(),
            });
        }
        oracle_blend_end();
        result
    }
}

/// The oracle's event code and arguments for a model device call.
fn code(call: &Call) -> (u32, [u32; 4]) {
    match *call {
        Call::Enable(e) => (1, [u32::from(e), 0, 0, 0]),
        Call::Factors(f) => (2, f),
        Call::Operation(o) => (3, [o, 0, 0, 0]),
        Call::Function([s, d]) => (4, [s, d, 0, 0]),
    }
}

fn compare(sequence: &[Step]) {
    let expected = native(false, sequence);
    let actual = native(true, sequence);
    for ((step, actual), expected) in sequence.iter().zip(&actual).zip(&expected) {
        assert_eq!(actual, expected, "{step:?}");
        // And what the C does, so a fault common to both sides shows too.
        if step.present {
            assert_eq!(actual.logs, 0);
            for event in &actual.events {
                assert_eq!(event.device, device(step.device).cast_const());
            }
        } else {
            assert!(actual.events.is_empty());
            assert_eq!((actual.logs, actual.level), (1, 400));
            let line = format!("{}: called while not in a graphics context", step.op.name());
            assert_eq!(actual.message, line);
        }
    }
    // The safe core over the model context makes the same stores and calls.
    let mut m = Mock {
        current: DEFAULT,
        ..Default::default()
    };
    for (step, expected) in sequence.iter().zip(expected) {
        if let Some(state) = step.state {
            m.current = state;
        }
        m.calls.clear();
        m.seen.clear();
        let context = step.present.then_some(&mut m);
        match step.op {
            Op::Push => core::push(context),
            Op::Pop => core::pop(context),
            Op::Reset => core::reset(context),
            Op::Enable(e) => core::enable_blending(context, e),
            Op::Function(s, d) => core::blend_function(context, s, d),
            Op::Separate([a, b, c, d]) => core::blend_function_separate(context, a, b, c, d),
            Op::Operation(o) => core::blend_op(context, o),
        }
        assert_eq!(m.current, expected.current, "{step:?}");
        assert_eq!(m.stack, expected.stack, "{step:?}");
        let model: Vec<_> = m
            .calls
            .iter()
            .map(code)
            .zip(m.seen.iter().copied())
            .collect();
        let native: Vec<_> = expected
            .events
            .iter()
            .map(|e| ((e.call, e.args), e.seen))
            .collect();
        assert_eq!(model, native, "{step:?}");
    }
}

fn state() -> impl Strategy<Value = BlendState> {
    (
        any::<bool>(),
        0u32..12,
        0u32..12,
        0u32..12,
        0u32..12,
        0u32..6,
    )
        .prop_map(|(enabled, src_c, dest_c, src_a, dest_a, op)| BlendState {
            enabled,
            src_c,
            dest_c,
            src_a,
            dest_a,
            op,
        })
}

/// Blend types, mostly declared, sometimes any 32-bit value.
fn factor() -> impl Strategy<Value = u32> {
    prop_oneof![4 => 0u32..11, 1 => any::<u32>()]
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        Just(Op::Push),
        Just(Op::Pop),
        Just(Op::Reset),
        any::<bool>().prop_map(Op::Enable),
        (factor(), factor()).prop_map(|(s, d)| Op::Function(s, d)),
        [factor(), factor(), factor(), factor()].prop_map(Op::Separate),
        prop_oneof![4 => 0u32..5, 1 => any::<u32>()].prop_map(Op::Operation),
    ]
}

fn step(present: bool, state: BlendState, op: Op) -> Step {
    Step {
        present,
        device: 0,
        state: Some(state),
        op,
    }
}

#[test]
fn deterministic_edges() {
    let mut sequence = vec![
        step(false, DEFAULT, Op::Push),
        step(false, DEFAULT, Op::Pop),
        step(false, DEFAULT, Op::Reset),
        step(false, DEFAULT, Op::Enable(false)),
        step(false, DEFAULT, Op::Function(1, 0)),
        step(false, DEFAULT, Op::Separate([1, 0, 4, 5])),
        step(false, DEFAULT, Op::Operation(1)),
        step(true, DEFAULT, Op::Pop),
    ];
    for mask in 0..8 {
        let mut state = DEFAULT;
        if mask & 1 != 0 {
            state.enabled = false;
        }
        if mask & 2 != 0 {
            state.src_c = 0;
        }
        if mask & 4 != 0 {
            state.op = 1;
        }
        sequence.extend([
            step(true, state, Op::Push),
            step(true, DEFAULT, Op::Push),
            step(true, DEFAULT, Op::Pop),
            step(true, DEFAULT, Op::Pop),
            step(true, DEFAULT, Op::Pop),
            step(true, state, Op::Pop),
            step(true, state, Op::Reset),
            step(true, DEFAULT, Op::Reset),
        ]);
    }
    // The setters on both devices, then what push saves and pop restores.
    for device in 0..2 {
        for op in [
            Op::Enable(true),
            Op::Enable(false),
            Op::Function(42, 0x7fff),
            Op::Separate([99, 300, 1000, 0]),
            Op::Separate([u32::MAX, 0, 1, u32::MAX]),
            Op::Operation(17),
            Op::Operation(u32::MAX),
            Op::Push,
            Op::Function(2, 3),
            Op::Operation(3),
            Op::Pop,
            Op::Reset,
        ] {
            sequence.push(Step {
                present: true,
                device,
                state: None,
                op,
            });
        }
    }
    compare(&sequence);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(100), failure_persistence: None, ..ProptestConfig::default()
    })]
    #[test]
    fn callback_and_state_sequences(
        sequence in prop::collection::vec(
            (any::<bool>(), 0usize..2, prop::option::weighted(0.3, state()), op())
                .prop_map(|(present, device, state, op)| Step { present, device, state, op }),
            0..60,
        ),
    ) {
        compare(&sequence);
    }
}

#[test]
fn real_c_layout() {
    let values = [
        size_of::<BlendState>(),
        align_of::<BlendState>(),
        std::mem::offset_of!(BlendState, enabled),
        std::mem::offset_of!(BlendState, src_c),
        std::mem::offset_of!(BlendState, dest_c),
        std::mem::offset_of!(BlendState, src_a),
        std::mem::offset_of!(BlendState, dest_a),
        std::mem::offset_of!(BlendState, op),
    ];
    for (index, value) in values.into_iter().enumerate() {
        // SAFETY: all eight layout indices are in range; this reads the real C type.
        assert_eq!(unsafe { oracle_blend_layout(index as u32) }, value);
    }
}
