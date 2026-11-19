//! Tier 3: original C and the Rust ABI use real adapters over fake TLS devices.
//! No intentional differences; no host capability or GPU is queried.
use obs_c_oracle as _;
use obs_graphics::ffi::planar_capability_queries as rust;
use proptest::prelude::*;
use std::ffi::{CStr, c_char};

unsafe extern "C" {
    fn oracle_gs_nv12_available() -> bool;
    fn oracle_gs_p010_available() -> bool;
    fn oracle_planar_capability_reset(
        context: bool,
        nv12: bool,
        p010: bool,
        nv12_result: bool,
        p010_result: bool,
        device: bool,
    );
    fn oracle_planar_capability_calls(p010: bool) -> u32;
    fn oracle_planar_capability_last_device() -> u32;
    fn oracle_device_queries_logs() -> u32;
    fn oracle_device_queries_level() -> i32;
    fn oracle_device_queries_log() -> *const c_char;
}

#[derive(Clone, Copy, Debug)]
struct State {
    context: bool,
    callbacks: [bool; 2],
    results: [bool; 2],
    device: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Observed {
    value: bool,
    calls: [u32; 2],
    last_device: u32,
    logs: u32,
    level: i32,
    message: Vec<u8>,
}

fn run(ported: bool, state: State, queries: &[bool]) -> Vec<Observed> {
    // SAFETY: all fixture state, devices, logging and callbacks are thread-local
    // and live throughout the sequence. The C headers supply the real layouts.
    unsafe {
        oracle_planar_capability_reset(
            state.context,
            state.callbacks[0],
            state.callbacks[1],
            state.results[0],
            state.results[1],
            state.device,
        );
        queries
            .iter()
            .map(|p010| {
                let value = match (ported, p010) {
                    (true, false) => rust::gs_nv12_available(),
                    (true, true) => rust::gs_p010_available(),
                    (false, false) => oracle_gs_nv12_available(),
                    (false, true) => oracle_gs_p010_available(),
                };
                Observed {
                    value,
                    calls: [
                        oracle_planar_capability_calls(false),
                        oracle_planar_capability_calls(true),
                    ],
                    last_device: oracle_planar_capability_last_device(),
                    logs: oracle_device_queries_logs(),
                    level: oracle_device_queries_level(),
                    message: CStr::from_ptr(oracle_device_queries_log())
                        .to_bytes()
                        .to_vec(),
                }
            })
            .collect()
    }
}

fn compare(state: State, queries: &[bool]) {
    let original = run(false, state, queries);
    let ported = run(true, state, queries);
    assert_eq!(ported, original, "{state:?}, {queries:?}");
    let mut calls = [0; 2];
    let mut device = 2;
    for (i, (&p010, observed)) in queries.iter().zip(ported).enumerate() {
        let index = usize::from(p010);
        let dispatch = state.context && state.callbacks[index];
        assert_eq!(observed.value, dispatch && state.results[index]);
        if dispatch {
            calls[index] += 1;
            device = u32::from(state.device);
        }
        assert_eq!(observed.calls, calls);
        assert_eq!(observed.last_device, device);
        if state.context {
            assert_eq!((observed.logs, observed.level), (0, 0));
            assert!(observed.message.is_empty());
        } else {
            assert_eq!(observed.logs as usize, i + 1);
            assert_eq!(observed.level, 400);
            let function = if p010 {
                "gs_p010_available"
            } else {
                "gs_nv12_available"
            };
            assert_eq!(
                observed.message,
                format!("{function}: called while not in a graphics context").as_bytes()
            );
        }
    }
}

#[test]
fn all_fake_device_callback_and_result_combinations_match_c() {
    for bits in 0..64 {
        compare(
            State {
                context: bits & 1 != 0,
                callbacks: [bits & 2 != 0, bits & 4 != 0],
                results: [bits & 8 != 0, bits & 16 != 0],
                device: bits & 32 != 0,
            },
            &[false, true, true, false],
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default()
    })]
    #[test]
    fn bounded_query_sequences_match_c(
        context in any::<bool>(), callbacks in any::<[bool; 2]>(),
        results in any::<[bool; 2]>(), device in any::<bool>(),
        queries in prop::collection::vec(any::<bool>(), 0..=16),
    ) {
        compare(State { context, callbacks, results, device }, &queries);
    }
}
