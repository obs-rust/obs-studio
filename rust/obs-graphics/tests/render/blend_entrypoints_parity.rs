//! Tier 3: original C and the production Rust shims, plus the safe core.
//! No intentional differences. C and Rust each own independent stack storage.
use super::blend_entrypoints::{Call, DEFAULT, Mock};
use obs_graphics::blend_entrypoints as core;
use obs_graphics::ffi::blend_entrypoints as ffi;
use obs_graphics::state_stacks::BlendState;
use proptest::prelude::*;
use std::ffi::{CStr, c_char};

unsafe extern "C" {
    fn oracle_blend_layout(index: u32) -> usize;
    fn oracle_gs_blend_state_push();
    fn oracle_gs_blend_state_pop();
    fn oracle_gs_reset_blend_state();
    fn oracle_blend_begin(present: bool, state: *const BlendState);
    fn oracle_blend_set(present: bool, state: *const BlendState);
    fn oracle_blend_clear_calls();
    fn oracle_blend_end();
    fn oracle_blend_count() -> u32;
    fn oracle_blend_call(index: u32) -> u32;
    fn oracle_blend_logs() -> u32;
    fn oracle_blend_message() -> *const c_char;
    fn oracle_blend_depth() -> usize;
    fn oracle_blend_current() -> BlendState;
    fn oracle_blend_saved(index: usize) -> BlendState;
}
#[derive(Debug, PartialEq)]
struct Snapshot {
    current: BlendState,
    stack: Vec<BlendState>,
    calls: Vec<u32>,
    logs: u32,
    message: String,
}
fn native(rust: bool, sequence: &[(bool, u8, BlendState)]) -> Vec<Snapshot> {
    // SAFETY: the oracle owns thread-local real C layouts and bounded buffers.
    // Every call records at most three callbacks; end frees the real C darray.
    unsafe {
        oracle_blend_begin(true, &DEFAULT);
        let mut result = Vec::new();
        for &(present, action, state) in sequence {
            oracle_blend_set(present, &state);
            oracle_blend_clear_calls();
            match (rust, action % 3) {
                (true, 0) => ffi::gs_blend_state_push(),
                (true, 1) => ffi::gs_blend_state_pop(),
                (true, _) => ffi::gs_reset_blend_state(),
                (false, 0) => oracle_gs_blend_state_push(),
                (false, 1) => oracle_gs_blend_state_pop(),
                (false, _) => oracle_gs_reset_blend_state(),
            }
            result.push(Snapshot {
                current: oracle_blend_current(),
                stack: (0..oracle_blend_depth())
                    .map(|i| oracle_blend_saved(i))
                    .collect(),
                calls: (0..oracle_blend_count())
                    .map(|i| oracle_blend_call(i))
                    .collect(),
                logs: oracle_blend_logs(),
                message: CStr::from_ptr(oracle_blend_message())
                    .to_string_lossy()
                    .into_owned(),
            });
        }
        oracle_blend_end();
        result
    }
}
fn compare(sequence: &[(bool, u8, BlendState)]) {
    let expected = native(false, sequence);
    for (actual, expected) in native(true, sequence).iter().zip(&expected) {
        assert_eq!(actual, expected);
    }
    let mut m = Mock::default();
    for (&(present, action, state), expected) in sequence.iter().zip(expected) {
        m.current = state;
        m.calls.clear();
        let context = present.then_some(&mut m);
        match action % 3 {
            0 => core::push(context),
            1 => core::pop(context),
            _ => core::reset(context),
        }
        assert_eq!(m.current, expected.current);
        assert_eq!(m.stack, expected.stack);
        assert_eq!(
            m.calls
                .iter()
                .map(|call| match call {
                    Call::Enable(_) => 1,
                    Call::Factors(_) => 2,
                    Call::Operation(_) => 3,
                    Call::Function(_) => 4,
                })
                .collect::<Vec<_>>(),
            expected.calls
        );
    }
}
fn state() -> impl Strategy<Value = BlendState> {
    (
        any::<bool>(),
        0u32..12,
        0u32..12,
        0u32..12,
        0u32..12,
        0u32..5,
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
#[test]
fn deterministic_edges() {
    let mut sequence = vec![
        (false, 0, DEFAULT),
        (false, 1, DEFAULT),
        (false, 2, DEFAULT),
        (true, 1, DEFAULT),
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
            (true, 0, state),
            (true, 0, DEFAULT),
            (true, 1, DEFAULT),
            (true, 1, DEFAULT),
            (true, 1, state),
            (true, 2, state),
            (true, 2, DEFAULT),
        ]);
    }
    compare(&sequence);
}
proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(100), failure_persistence: None, ..ProptestConfig::default()
    })]
    #[test]
    fn callback_and_state_sequences(sequence in prop::collection::vec((any::<bool>(), 0u8..3, state()), 0..60)) {
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
