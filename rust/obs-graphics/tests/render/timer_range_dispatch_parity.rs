//! Original C bodies are the baseline. No intentional behavior differences.
use super::timer_range_dispatch::{self as model, Case};
use obs_c_oracle as _;
use obs_graphics::ffi::timer_range_dispatch::{
    gs_timer_range_begin, gs_timer_range_destroy, gs_timer_range_end, gs_timer_range_get_data,
};
use proptest::prelude::*;
use std::ffi::{CStr, c_char, c_void};
unsafe extern "C" {
    fn oracle_trd_run(
        context: bool,
        range: bool,
        disjoint: bool,
        frequency: bool,
        op: u32,
        result: bool,
        writes: u32,
        value_disjoint: bool,
        value: u64,
        out_disjoint: *mut bool,
        out_frequency: *mut u64,
    ) -> bool;
    fn oracle_trd_timer() -> *mut c_void;
    fn oracle_trd_log() -> *const c_char;
    fn oracle_trd_events() -> *const c_char;
    fn oracle_trd_level() -> i32;
}
fn compare(c: Case) {
    let mut disjoint = !c.value_disjoint;
    let mut frequency = 42;
    // SAFETY: the thread-local oracle writes only these live outputs.
    let result = unsafe {
        oracle_trd_run(
            c.context,
            c.range,
            c.disjoint,
            c.frequency,
            c.op,
            c.result,
            c.writes,
            c.value_disjoint,
            c.value,
            &mut disjoint,
            &mut frequency,
        )
    };
    // SAFETY: the oracle's thread-local diagnostic and event buffers are terminated and live.
    let (log, events, level) = unsafe {
        (
            CStr::from_ptr(oracle_trd_log())
                .to_str()
                .unwrap()
                .to_owned(),
            CStr::from_ptr(oracle_trd_events())
                .to_str()
                .unwrap()
                .to_owned(),
            oracle_trd_level(),
        )
    };
    let mut abi_disjoint = !c.value_disjoint;
    let mut abi_frequency = 42;
    // SAFETY: reset the thread-local real-header backend without dispatching, using live outputs.
    unsafe {
        oracle_trd_run(
            c.context,
            c.range,
            c.disjoint,
            c.frequency,
            4,
            c.result,
            c.writes,
            c.value_disjoint,
            c.value,
            &mut abi_disjoint,
            &mut abi_frequency,
        );
    }
    // SAFETY: the fixture owns the handle and both writable outputs throughout synchronous dispatch.
    let abi_result = unsafe {
        let handle = if c.range {
            oracle_trd_timer()
        } else {
            std::ptr::null_mut()
        };
        if c.op == 3 {
            gs_timer_range_get_data(
                handle,
                if c.disjoint {
                    &mut abi_disjoint
                } else {
                    std::ptr::null_mut()
                },
                if c.frequency {
                    &mut abi_frequency
                } else {
                    std::ptr::null_mut()
                },
            )
        } else {
            [
                gs_timer_range_destroy,
                gs_timer_range_begin,
                gs_timer_range_end,
            ][c.op as usize](handle);
            false
        }
    };
    // SAFETY: oracle buffers are live, thread-local and terminated after dispatch.
    let abi = unsafe {
        (
            abi_result,
            abi_disjoint,
            abi_frequency,
            CStr::from_ptr(oracle_trd_log())
                .to_str()
                .unwrap()
                .to_owned(),
            CStr::from_ptr(oracle_trd_events())
                .to_str()
                .unwrap()
                .to_owned(),
        )
    };
    let actual = model::run(c);
    assert_eq!(actual, abi, "ABI {c:?}");
    assert_eq!(actual, (result, disjoint, frequency, log, events), "{c:?}");
    assert_eq!(level, if actual.3.is_empty() { 0 } else { 400 });
}
#[test]
fn characterization_matches_original_c() {
    for context in [false, true] {
        for range in [false, true] {
            for disjoint in [false, true] {
                for frequency in [false, true] {
                    for op in 0..4 {
                        for result in [false, true] {
                            for writes in 0..4 {
                                for value_disjoint in [false, true] {
                                    for value in [0, 1, 0xfedcba9876543210, u64::MAX] {
                                        compare(Case {
                                            context,
                                            range,
                                            disjoint,
                                            frequency,
                                            op,
                                            result,
                                            writes,
                                            value_disjoint,
                                            value,
                                        });
                                    }
                                }
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
    fn random_matches_original_c(flags in any::<[bool; 6]>(), op in 0u32..4, writes in 0u32..4, value in any::<u64>()) {
        compare(Case { context: flags[0], range: flags[1], disjoint: flags[2], frequency: flags[3], result: flags[4], value_disjoint: flags[5], op, writes, value });
    }
}
