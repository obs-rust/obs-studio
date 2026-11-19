//! Tier 3: unchanged original C and Rust ABI over the real private adapters.
//! NULL mandatory callbacks in a live context are C UB and are never invoked.
//! Missing-context cases include absent callbacks; there are no parity differences.
use obs_c_oracle as _;
use obs_graphics::ffi::active_shader_queries as rust;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_void};

unsafe extern "C" {
    fn oracle_gs_get_vertex_shader() -> *mut c_void;
    fn oracle_gs_get_pixel_shader() -> *mut c_void;
    fn oracle_active_shader_reset(
        vertex: bool,
        pixel: bool,
        vertex_result: u32,
        pixel_result: u32,
        device: bool,
    );
    fn oracle_active_shader_context(context: bool);
    fn oracle_active_shader_token(token: u32) -> *mut c_void;
    fn oracle_active_shader_calls(pixel: bool) -> u32;
    fn oracle_active_shader_last_device() -> u32;
    fn oracle_active_shader_ref() -> i32;
    fn oracle_device_queries_logs() -> u32;
    fn oracle_device_queries_level() -> i32;
    fn oracle_device_queries_log() -> *const c_char;
}

#[derive(Clone, Copy, Debug)]
struct State {
    callbacks: [bool; 2],
    results: [u32; 2],
    device: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Observed {
    value: usize,
    calls: [u32; 2],
    last_device: u32,
    reference_count: i32,
    logs: u32,
    level: i32,
    message: Vec<u8>,
}

fn run(ported: bool, state: State, queries: &[(bool, bool)]) -> Vec<Observed> {
    // SAFETY: the real C layouts, fake device/shader tokens, callbacks and log
    // recorder are thread-local and remain live. Live contexts have both mandatory
    // callbacks. Only the C fixture projects its private layouts.
    unsafe {
        oracle_active_shader_reset(
            state.callbacks[0],
            state.callbacks[1],
            state.results[0],
            state.results[1],
            state.device,
        );
        queries
            .iter()
            .map(|&(context, pixel)| {
                assert!(!context || state.callbacks == [true; 2]);
                oracle_active_shader_context(context);
                let value = match (ported, pixel) {
                    (true, false) => rust::gs_get_vertex_shader(),
                    (true, true) => rust::gs_get_pixel_shader(),
                    (false, false) => oracle_gs_get_vertex_shader(),
                    (false, true) => oracle_gs_get_pixel_shader(),
                };
                Observed {
                    value: value as usize,
                    calls: [
                        oracle_active_shader_calls(false),
                        oracle_active_shader_calls(true),
                    ],
                    last_device: oracle_active_shader_last_device(),
                    reference_count: oracle_active_shader_ref(),
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

fn compare(state: State, queries: &[(bool, bool)]) {
    let original = run(false, state, queries);
    let ported = run(true, state, queries);
    assert_eq!(ported, original, "{state:?}, {queries:?}");
    let mut calls = [0; 2];
    let mut last_device = 2;
    let mut logs = 0;
    let mut message = Vec::new();
    for (&(context, pixel), observed) in queries.iter().zip(ported) {
        let index = usize::from(pixel);
        let token = if context { state.results[index] } else { 0 };
        // SAFETY: the token getter returns a borrowed thread-local fixture identity.
        assert_eq!(observed.value, unsafe { oracle_active_shader_token(token) }
            as usize);
        if context {
            calls[index] += 1;
            last_device = u32::from(state.device);
        } else {
            logs += 1;
            let name = if pixel {
                "gs_get_pixel_shader"
            } else {
                "gs_get_vertex_shader"
            };
            message = format!("{name}: called while not in a graphics context").into_bytes();
        }
        assert_eq!(observed.calls, calls);
        assert_eq!(observed.last_device, last_device);
        assert_eq!(observed.reference_count, i32::from(context));
        assert_eq!(observed.logs, logs);
        assert_eq!(observed.level, if logs == 0 { 0 } else { 400 });
        assert_eq!(observed.message, message);
    }
}

#[test]
fn all_shader_identities_devices_and_context_transitions_match_c() {
    for vertex in 0..3 {
        for pixel in 0..3 {
            for device in [false, true] {
                for callbacks in [[false, false], [true, false], [false, true], [true, true]] {
                    let live = callbacks == [true; 2];
                    compare(
                        State {
                            callbacks,
                            results: [vertex, pixel],
                            device,
                        },
                        &[
                            (false, false),
                            (false, true),
                            (live, true),
                            (live, false),
                            (false, false),
                            (false, true),
                            (live, false),
                            (live, true),
                        ],
                    );
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default()
    })]
    #[test]
    fn bounded_shader_query_sequences_match_c(
        vertex in 0_u32..3, pixel in 0_u32..3, device in any::<bool>(),
        queries in prop::collection::vec((any::<bool>(), any::<bool>()), 0..=16),
    ) {
        compare(State { callbacks: [true; 2], results: [vertex, pixel], device }, &queries);
    }
}
