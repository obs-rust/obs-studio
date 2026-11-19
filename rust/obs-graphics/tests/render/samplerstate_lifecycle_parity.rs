//! Tier 3: original C vs production Rust ABI and real adapters. No differences.
//! Missing callbacks are generated only when validation prevents their use;
//! calling an absent required callback is undefined in the original C.
use obs_c_oracle as _;
use obs_graphics::ffi::samplerstate_lifecycle as rust;
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_void};
use std::ptr;

unsafe extern "C" {
    fn oracle_gs_samplerstate_create(info: *const c_void) -> *mut c_void;
    fn oracle_gs_samplerstate_destroy(sampler: *mut c_void);
    fn oracle_samplerstate_reset();
    fn oracle_samplerstate_info(
        slot: u32,
        filter: u32,
        u: u32,
        v: u32,
        w: u32,
        anisotropy: i32,
        border: u32,
    ) -> *const c_void;
    fn oracle_samplerstate_token(slot: u32) -> *mut c_void;
    fn oracle_samplerstate_configure(
        context: bool,
        callbacks: u32,
        info: *const c_void,
        result: *mut c_void,
        sampler: *mut c_void,
    );
    fn oracle_samplerstate_calls(index: u32) -> u32;
    fn oracle_samplerstate_logs() -> u32;
    fn oracle_samplerstate_level() -> i32;
    fn oracle_samplerstate_log() -> *const c_char;
    fn oracle_samplerstate_events() -> *const c_char;
    fn oracle_samplerstate_unchanged() -> bool;
}
#[derive(Clone, Debug)]
struct Step {
    context: bool,
    callbacks: u32,
    destroy: bool,
    info: Option<u32>,
    sampler: Option<u32>,
    result: Option<u32>,
    settings: (u32, u32, u32, u32, i32, u32),
}
#[derive(Debug, PartialEq, Eq)]
struct Observed {
    result: Option<u32>,
    calls: [u32; 2],
    logs: u32,
    level: i32,
    message: Vec<u8>,
    events: Vec<u8>,
    unchanged: bool,
}
fn run(ported: bool, steps: &[Step]) -> Vec<Observed> {
    // SAFETY: all recording state and adapters use fixture-local TLS. Settings,
    // device and opaque token pointers remain live; no callback frees a token.
    // Required callbacks are installed for every reachable dispatch. C owns
    // the settings layout; Rust forwards only pointers returned by the fixture.
    unsafe {
        oracle_samplerstate_reset();
        let tokens = [oracle_samplerstate_token(0), oracle_samplerstate_token(1)];
        steps
            .iter()
            .map(|s| {
                let (f, u, v, w, a, b) = s.settings;
                let info = s.info.map_or(ptr::null(), |i| {
                    oracle_samplerstate_info(i, f, u, v, w, a, b)
                });
                let sampler = s.sampler.map_or(ptr::null_mut(), |i| tokens[i as usize]);
                let result = s.result.map_or(ptr::null_mut(), |i| tokens[i as usize]);
                let required = if s.context {
                    if s.destroy {
                        u32::from(s.sampler.is_some()) * 2
                    } else {
                        u32::from(s.info.is_some())
                    }
                } else {
                    0
                };
                oracle_samplerstate_configure(
                    s.context,
                    s.callbacks | required,
                    info,
                    result,
                    sampler,
                );
                let returned = match (ported, s.destroy) {
                    (true, false) => rust::gs_samplerstate_create(info),
                    (false, false) => oracle_gs_samplerstate_create(info),
                    (true, true) => {
                        rust::gs_samplerstate_destroy(sampler);
                        ptr::null_mut()
                    }
                    (false, true) => {
                        oracle_gs_samplerstate_destroy(sampler);
                        ptr::null_mut()
                    }
                };
                let result = if returned.is_null() {
                    None
                } else {
                    Some(
                        u32::try_from(
                            tokens
                                .iter()
                                .position(|&p| p == returned)
                                .expect("original handle identity"),
                        )
                        .unwrap(),
                    )
                };
                Observed {
                    result,
                    calls: [oracle_samplerstate_calls(0), oracle_samplerstate_calls(1)],
                    logs: oracle_samplerstate_logs(),
                    level: oracle_samplerstate_level(),
                    message: CStr::from_ptr(oracle_samplerstate_log())
                        .to_bytes()
                        .to_vec(),
                    events: CStr::from_ptr(oracle_samplerstate_events())
                        .to_bytes()
                        .to_vec(),
                    unchanged: oracle_samplerstate_unchanged(),
                }
            })
            .collect()
    }
}
fn compare(steps: &[Step]) {
    let original = run(false, steps);
    let ported = run(true, steps);
    assert_eq!(ported, original, "{steps:?}");
    let mut calls = [0, 0];
    let mut logs = 0;
    let mut message = Vec::new();
    let mut events = Vec::new();
    for (s, observed) in steps.iter().zip(ported) {
        let mut result = None;
        let diagnostic = if !s.context {
            Some(format!(
                "gs_samplerstate_{}: called while not in a graphics context",
                if s.destroy { "destroy" } else { "create" }
            ))
        } else if !s.destroy && s.info.is_none() {
            Some("gs_samplerstate_create: Null 'info' parameter".into())
        } else {
            None
        };
        if let Some(diagnostic) = diagnostic {
            logs += 1;
            message = diagnostic.into_bytes();
            events.push(b'L');
        } else if !s.destroy {
            calls[0] += 1;
            events.push(b'C');
            result = s.result;
        } else if s.sampler.is_some() {
            calls[1] += 1;
            events.push(b'D');
        }
        assert_eq!(
            observed,
            Observed {
                result,
                calls,
                logs,
                level: if logs == 0 { 0 } else { 400 },
                message: message.clone(),
                events: events.clone(),
                unchanged: true
            }
        );
    }
}
#[test]
fn characterized_sequences_match_c() {
    for context in [false, true] {
        for callbacks in 0..4 {
            let mut steps = Vec::new();
            for i in 0..3 {
                for (destroy, info, sampler) in [
                    (false, Some(i % 2), None),
                    (false, None, None),
                    (true, None, Some(i % 2)),
                    (true, None, None),
                ] {
                    steps.push(Step {
                        context,
                        callbacks,
                        destroy,
                        info,
                        sampler,
                        result: if i < 2 { Some(i) } else { None },
                        settings: if i % 2 == 0 {
                            (0, 0, 1, 2, i32::MIN, 0)
                        } else {
                            (2, 3, 4, 0, i32::MAX, u32::MAX)
                        },
                    });
                }
            }
            compare(&steps);
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_sequences_match_c(steps in prop::collection::vec(
        (any::<bool>(),0u32..4,any::<bool>(),prop::option::of(0u32..2),prop::option::of(0u32..2),prop::option::of(0u32..2),
        (0u32..9,0u32..5,0u32..5,0u32..5,any::<i32>(),any::<u32>())),1..=16)) {
        let steps: Vec<_> = steps.into_iter().map(|(context,callbacks,destroy,info,sampler,result,settings)| Step {context,callbacks,destroy,info,sampler,result,settings}).collect();
        compare(&steps);
    }
}
