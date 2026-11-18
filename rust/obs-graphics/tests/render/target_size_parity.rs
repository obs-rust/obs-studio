//! Tier 3: original C and production Rust ABI/adapters. No intentional differences.
//! Missing required callbacks are allowed only when no context/query can reach them.
use obs_c_oracle as _;
use obs_graphics::ffi::target_size as rust;
use proptest::prelude::*;
use std::ffi::{CStr, c_char};
use std::ptr;

unsafe extern "C" {
    fn oracle_gs_get_size(x: *mut u32, y: *mut u32);
    fn oracle_gs_get_width() -> u32;
    fn oracle_gs_get_height() -> u32;
    fn oracle_target_size_reset(context: bool, callbacks: u32, width: u32, height: u32);
    fn oracle_target_size_expect(x: *mut u32, y: *mut u32);
    fn oracle_target_size_calls(index: u32) -> u32;
    fn oracle_target_size_logs() -> u32;
    fn oracle_target_size_level() -> i32;
    fn oracle_target_size_log() -> *const c_char;
}
#[derive(Clone, Copy, Debug)]
struct Step {
    context: bool,
    callbacks: u32,
    query: u32,
    outputs: [Option<usize>; 2],
    values: [u32; 2],
}
#[derive(Debug, PartialEq, Eq)]
struct Observed {
    result: u32,
    outputs: [u32; 2],
    calls: [u32; 3],
    logs: u32,
    level: i32,
    message: Vec<u8>,
}
fn run(ported: bool, step: Step) -> Observed {
    let mut outputs = [0xa5a5_a5a5, 0x5a5a_5a5a];
    let [x, y] = step
        .outputs
        .map(|slot| slot.map_or(ptr::null_mut(), |index| &raw mut outputs[index]));
    // SAFETY: the recording fixture, context and logging are thread-local.
    // Live queries have the required callback. Output pointers are NULL or live
    // u32 slots, and the tolerant backend accepts aliasing without Rust references.
    unsafe {
        oracle_target_size_reset(step.context, step.callbacks, step.values[0], step.values[1]);
        oracle_target_size_expect(x, y);
        let result = match (ported, step.query) {
            (true, 0) => {
                rust::gs_get_size(x, y);
                0
            }
            (false, 0) => {
                oracle_gs_get_size(x, y);
                0
            }
            (true, 1) => rust::gs_get_width(),
            (false, 1) => oracle_gs_get_width(),
            (true, 2) => rust::gs_get_height(),
            (false, 2) => oracle_gs_get_height(),
            _ => unreachable!(),
        };
        Observed {
            result,
            outputs,
            calls: [
                oracle_target_size_calls(0),
                oracle_target_size_calls(1),
                oracle_target_size_calls(2),
            ],
            logs: oracle_target_size_logs(),
            level: oracle_target_size_level(),
            message: CStr::from_ptr(oracle_target_size_log()).to_bytes().to_vec(),
        }
    }
}
fn compare(step: Step) {
    let original = run(false, step);
    let ported = run(true, step);
    assert_eq!(ported, original, "{step:?}");
    let mut outputs = [0xa5a5_a5a5, 0x5a5a_5a5a];
    let mut calls = [0; 3];
    let mut result = 0;
    if step.context {
        calls[step.query as usize] = 1;
        if step.query == 0 {
            for (slot, value) in step.outputs.into_iter().zip(step.values) {
                if let Some(slot) = slot {
                    outputs[slot] = value;
                }
            }
        } else {
            result = step.values[step.query as usize - 1];
        }
        assert_eq!((ported.logs, ported.level), (0, 0));
        assert!(ported.message.is_empty());
    } else {
        assert_eq!((ported.logs, ported.level), (1, 400));
        let function = ["gs_get_size", "gs_get_width", "gs_get_height"][step.query as usize];
        assert_eq!(
            ported.message,
            format!("{function}: called while not in a graphics context").as_bytes()
        );
    }
    assert_eq!(ported.outputs, outputs);
    assert_eq!(ported.calls, calls);
    assert_eq!(ported.result, result);
}
#[test]
fn characterized_matrix_matches_c() {
    for context in [false, true] {
        for callbacks in 0..8 {
            for query in 0..3 {
                if context && callbacks & (1 << query) == 0 {
                    continue;
                }
                for x in [None, Some(0), Some(1)] {
                    for y in [None, Some(0), Some(1)] {
                        for values in [
                            [1920, 1080],
                            [0, 0],
                            [0, 1],
                            [1, 0],
                            [0x8000_0000, u32::MAX],
                            [u32::MAX, 0x8000_0000],
                        ] {
                            compare(Step {
                                context,
                                callbacks,
                                query,
                                outputs: [x, y],
                                values,
                            });
                        }
                    }
                }
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_queries_match_c(steps in prop::collection::vec(
        (any::<bool>(), 0u32..8, 0u32..3, prop::option::of(0usize..2), prop::option::of(0usize..2), any::<[u32;2]>()), 1..=16
    )) {
        for (context, callbacks, query, x, y, values) in steps {
            compare(Step { context, callbacks: callbacks | if context { 1 << query } else { 0 }, query, outputs: [x,y], values });
        }
    }
}
