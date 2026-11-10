//! Original extracted C is the behavioral baseline; no intentional differences.
use obs_c_oracle as _;
use obs_graphics::ffi::timer_dispatch::{
    gs_timer_begin, gs_timer_destroy, gs_timer_end, gs_timer_get_data,
};
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_void};
unsafe extern "C" {
    fn oracle_td_run(
        context: bool,
        timer: bool,
        ticks: bool,
        op: u32,
        result: bool,
        write: bool,
        value: u64,
        output: *mut u64,
    ) -> bool;
    fn oracle_td_timer() -> *mut c_void;
    fn oracle_td_log() -> *const c_char;
    fn oracle_td_events() -> *const c_char;
    fn oracle_td_level() -> i32;
}
use super::timer_dispatch as model;
fn compare(
    context: bool,
    timer: bool,
    ticks: bool,
    op: u32,
    result: bool,
    write: bool,
    value: u64,
) {
    let mut output = 42;
    // SAFETY: the oracle uses a thread-local fixture and writes only this live output.
    let expected =
        unsafe { oracle_td_run(context, timer, ticks, op, result, write, value, &mut output) };
    // SAFETY: thread-local log and event buffers are live and terminated.
    let expected_log = unsafe { CStr::from_ptr(oracle_td_log()).to_str().unwrap().to_owned() };
    // SAFETY: same fixture owns this event buffer.
    let expected_events = unsafe {
        CStr::from_ptr(oracle_td_events())
            .to_str()
            .unwrap()
            .to_owned()
    };
    let mut abi_output = 42;
    // SAFETY: reset the same thread-local real-header context without dispatching.
    unsafe {
        oracle_td_run(
            context,
            timer,
            ticks,
            4,
            result,
            write,
            value,
            &mut abi_output,
        );
    }
    // SAFETY: opaque handle belongs to the initialized backend; output is live.
    let abi = unsafe {
        let handle = if timer {
            oracle_td_timer()
        } else {
            std::ptr::null_mut()
        };
        let result = if op == 3 {
            gs_timer_get_data(
                handle,
                if ticks {
                    &mut abi_output
                } else {
                    std::ptr::null_mut()
                },
            )
        } else {
            [gs_timer_destroy, gs_timer_begin, gs_timer_end][op as usize](handle);
            false
        };
        (
            result,
            abi_output,
            CStr::from_ptr(oracle_td_log()).to_str().unwrap().to_owned(),
            CStr::from_ptr(oracle_td_events())
                .to_str()
                .unwrap()
                .to_owned(),
        )
    };
    let actual = model::run(context, timer, ticks, op, result, write, value);
    assert_eq!(actual, abi);
    // SAFETY: oracle buffers remain alive and terminated until the next call on this thread.
    unsafe {
        assert_eq!(actual, (expected, output, expected_log, expected_events));
        assert_eq!(oracle_td_level(), if actual.2.is_empty() { 0 } else { 400 });
    }
}
#[test]
fn characterization_matches_original_c() {
    for context in [false, true] {
        for timer in [false, true] {
            for ticks in [false, true] {
                for op in 0..4 {
                    for result in [false, true] {
                        for write in [false, true] {
                            for value in [0, 1, 0xfedcba9876543210, u64::MAX] {
                                compare(context, timer, ticks, op, result, write, value);
                            }
                        }
                    }
                }
            }
        }
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(256), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn random_matches_original_c(context in any::<bool>(), timer in any::<bool>(), ticks in any::<bool>(), op in 0u32..4, result in any::<bool>(), write in any::<bool>(), value in any::<u64>()) { compare(context,timer,ticks,op,result,write,value); }
}
