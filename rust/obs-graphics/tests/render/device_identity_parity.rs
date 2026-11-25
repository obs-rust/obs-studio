//! Tier 3: original C bodies and Rust ABI through real private adapters.
//! Required NULL callbacks in an admitted context are C UB, excluded here.
//! No intentional differences. All fixture and diagnostic state is thread-local.
use obs_c_oracle as _;
use obs_graphics::ffi::device_identity as rust;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_void};

unsafe extern "C" {
    fn oracle_gs_get_device_obj() -> *mut c_void;
    fn oracle_gs_get_device_name() -> *const c_char;
    fn oracle_device_identity_reset(
        object: bool,
        name: bool,
        object_result: u32,
        name_result: u32,
        device: u32,
    );
    fn oracle_device_identity_context(context: bool);
    fn oracle_device_identity_token(name: bool, token: u32) -> *const c_void;
    fn oracle_device_identity_calls(name: bool) -> u32;
    fn oracle_device_identity_last_device() -> u32;
    fn oracle_device_identity_mutate_name();
    fn oracle_device_queries_logs() -> u32;
    fn oracle_device_queries_level() -> i32;
    fn oracle_device_queries_log() -> *const c_char;
}
#[derive(Clone, Copy, Debug)]
struct State {
    callbacks: [bool; 2],
    results: [u32; 2],
    device: u32,
}
#[derive(Debug, PartialEq, Eq)]
struct Observed {
    pointer: usize,
    name: Option<Vec<u8>>,
    calls: [u32; 2],
    device: u32,
    logs: u32,
    level: i32,
    message: Vec<u8>,
}
fn run(ported: bool, state: State, queries: &[(bool, bool)]) -> Vec<Observed> {
    // SAFETY: fixture tokens, backend callbacks, context and logs are thread-local
    // and live throughout. NULL required callbacks are never invoked with context.
    unsafe {
        oracle_device_identity_reset(
            state.callbacks[0],
            state.callbacks[1],
            state.results[0],
            state.results[1],
            state.device,
        );
        queries
            .iter()
            .map(|&(context, name)| {
                assert!(!context || state.callbacks == [true; 2]);
                oracle_device_identity_context(context);
                let value = match (ported, name) {
                    (true, false) => rust::gs_get_device_obj().cast_const(),
                    (true, true) => rust::gs_get_device_name().cast(),
                    (false, false) => oracle_gs_get_device_obj().cast_const(),
                    (false, true) => oracle_gs_get_device_name().cast(),
                };
                Observed {
                    pointer: value as usize,
                    name: if name && !value.is_null() {
                        Some(CStr::from_ptr(value.cast()).to_bytes().to_vec())
                    } else {
                        None
                    },
                    calls: [
                        oracle_device_identity_calls(false),
                        oracle_device_identity_calls(true),
                    ],
                    device: oracle_device_identity_last_device(),
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
    let mut device = 3;
    let mut logs = 0;
    let mut message = Vec::new();
    for (&(context, name), observed) in queries.iter().zip(ported) {
        let index = usize::from(name);
        let token = if context { state.results[index] } else { 0 };
        // SAFETY: borrowed thread-local identity, compared without dereferencing.
        assert_eq!(
            observed.pointer,
            unsafe { oracle_device_identity_token(name, token) } as usize
        );
        if context {
            calls[index] += 1;
            if !name {
                device = state.device;
            }
        } else {
            logs += 1;
            let name = if name {
                "gs_get_device_name"
            } else {
                "gs_get_device_obj"
            };
            message = format!("{name}: called while not in a graphics context").into_bytes();
        }
        assert_eq!(observed.calls, calls);
        assert_eq!(observed.device, device);
        assert_eq!(observed.logs, logs);
        assert_eq!(observed.level, if logs == 0 { 0 } else { 400 });
        assert_eq!(observed.message, message);
    }
}
#[test]
fn all_identities_devices_callbacks_and_context_transitions_match_c() {
    for object in 0..3 {
        for name in 0..4 {
            for device in 0..3 {
                for callbacks in [[false, false], [true, false], [false, true], [true, true]] {
                    let live = callbacks == [true; 2];
                    compare(
                        State {
                            callbacks,
                            results: [object, name],
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
#[test]
fn borrowed_name_reflects_backend_mutation_without_changing_equal_peer() {
    for ported in [false, true] {
        // SAFETY: live thread-local context, writable NUL-terminated name arrays,
        // and mandatory callbacks remain valid until the next reset.
        unsafe {
            oracle_device_identity_reset(true, true, 1, 2, 0);
            oracle_device_identity_context(true);
            let borrowed = if ported {
                rust::gs_get_device_name()
            } else {
                oracle_gs_get_device_name()
            };
            assert_eq!(borrowed.cast(), oracle_device_identity_token(true, 2));
            oracle_device_identity_mutate_name();
            assert_eq!(CStr::from_ptr(borrowed), c"Recording backend");
            let peer = oracle_device_identity_token(true, 3).cast();
            assert_eq!(CStr::from_ptr(peer), c"recording backend");
            assert_eq!(oracle_device_identity_calls(true), 1);
            assert_eq!(oracle_device_identity_calls(false), 0);
            assert_eq!(oracle_device_queries_logs(), 0);
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default()
    })]
    #[test]
    fn bounded_identity_sequences_match_c(
        object in 0_u32..3, name in 0_u32..4, device in 0_u32..3,
        queries in prop::collection::vec((any::<bool>(), any::<bool>()), 0..=16),
    ) {
        compare(State { callbacks: [true; 2], results: [object, name], device }, &queries);
    }
}
