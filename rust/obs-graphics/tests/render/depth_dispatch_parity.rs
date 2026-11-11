//! Original C bodies and real private callback table; no intentional differences.
use super::depth_dispatch::Fixture;
use obs_c_oracle as _;
use obs_graphics::{
    depth_dispatch::{self, Command},
    ffi::depth_dispatch::{gs_depth_function, gs_enable_depth_test},
};
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_uint};
unsafe extern "C" {
    fn oracle_depth_begin();
    fn oracle_depth_context(present: bool, device: u32);
    fn oracle_gs_enable_depth_test(enable: bool);
    fn oracle_gs_depth_function(test: c_uint);
    fn oracle_depth_calls() -> u32;
    fn oracle_depth_logs() -> u32;
    fn oracle_depth_level() -> i32;
    fn oracle_depth_message() -> *const c_char;
    fn oracle_depth_kind(n: u32) -> u32;
    fn oracle_depth_value(n: u32) -> u32;
    fn oracle_depth_device(n: u32) -> u32;
    fn oracle_depth_enum(n: u32) -> c_uint;
    fn oracle_depth_size() -> usize;
    fn oracle_depth_align() -> usize;
}
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    events: Vec<(usize, Command)>,
    logs: u32,
    level: i32,
    message: String,
}
fn snapshot() -> Snapshot {
    // SAFETY: thread-local recording fixture is initialized; indexes are bounded by its call count.
    unsafe {
        let calls = oracle_depth_calls();
        assert!(calls <= 256, "recording backend capacity exceeded");
        Snapshot {
            events: (0..calls)
                .map(|n| {
                    let value = oracle_depth_value(n);
                    let command = match oracle_depth_kind(n) {
                        0 => Command::Enable(value != 0),
                        1 => Command::Function(value),
                        other => panic!("unexpected callback {other}"),
                    };
                    (oracle_depth_device(n) as usize, command)
                })
                .collect(),
            logs: oracle_depth_logs(),
            level: oracle_depth_level(),
            message: CStr::from_ptr(oracle_depth_message())
                .to_str()
                .unwrap()
                .to_owned(),
        }
    }
}
// Each step selects context presence, device identity, API and its exact argument.
type Step = (bool, bool, bool, u32);
fn compare(steps: &[Step]) {
    let mut expected = Vec::new();
    // SAFETY: all fixture storage is thread-local and each sequence has at most 128 calls.
    unsafe {
        oracle_depth_begin();
        for &(context, device, enable, value) in steps {
            oracle_depth_context(context, u32::from(device));
            if enable {
                oracle_gs_enable_depth_test(value != 0);
            } else {
                oracle_gs_depth_function(value);
            }
            expected.push(snapshot());
        }
        oracle_depth_begin();
    }
    let mut model = Fixture::default();
    for (&(context, device, enable, value), expected) in steps.iter().zip(expected) {
        let command = if enable {
            Command::Enable(value != 0)
        } else {
            Command::Function(value)
        };
        model.device = context.then_some(usize::from(device));
        depth_dispatch::command(&mut model, command);
        // SAFETY: the initialized thread-local fixture supplies a valid backend table and device.
        unsafe {
            oracle_depth_context(context, u32::from(device));
            if enable {
                gs_enable_depth_test(value != 0);
            } else {
                gs_depth_function(value);
            }
        }
        let actual = snapshot();
        assert_eq!(actual, expected);
        assert_eq!(model.events, actual.events);
        assert_eq!(model.logs.len(), actual.logs as usize);
        assert_eq!(
            model.logs.last().map(String::as_str).unwrap_or(""),
            actual.message
        );
        assert_eq!(actual.level, if model.logs.is_empty() { 0 } else { 400 });
    }
}
#[test]
fn test_depth_dispatch_matches_original_c() {
    let mut steps = Vec::new();
    let enables = [true, true, false, false, true, false, true, false];
    // Initially absent, both devices present in turn, then released again.
    for (context, device) in [(false, false), (true, false), (true, true), (false, true)] {
        for (value, enable) in enables.into_iter().enumerate() {
            steps.push((context, device, true, u32::from(enable)));
            steps.push((context, device, false, value as u32));
            steps.push((context, device, false, value as u32));
        }
    }
    compare(&steps);
}
#[test]
fn depth_enum_abi_matches_real_header() {
    // SAFETY: constant layout and enum probes have no preconditions.
    unsafe {
        assert_eq!(size_of::<c_uint>(), oracle_depth_size());
        assert_eq!(align_of::<c_uint>(), oracle_depth_align());
        for value in 0..8 {
            assert_eq!(oracle_depth_enum(value), value);
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn generated_depth_matches_original_c(steps in prop::collection::vec((any::<bool>(), any::<bool>(), any::<bool>(), 0u32..8), 1..129)) {
        compare(&steps);
    }
}
