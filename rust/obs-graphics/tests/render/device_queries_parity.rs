//! Tier 3: original C, real graphics layouts, and production field adapters.
//! No intentional differences. A live device-type query requires its backend
//! callback, as in C; absent required callbacks are tested only without context.
use obs_c_oracle as _;
use obs_graphics::ffi::device_queries as rust;
use proptest::prelude::*;
use std::ffi::{CStr, c_char};

unsafe extern "C" {
    fn oracle_gs_get_device_type() -> i32;
    fn oracle_gs_get_adapter_count() -> u32;
    fn oracle_device_queries_reset(
        context: bool,
        type_callback: bool,
        adapter_callback: bool,
        device_type: i32,
        count: u32,
    );
    fn oracle_device_queries_type_calls() -> u32;
    fn oracle_device_queries_adapter_calls() -> u32;
    fn oracle_device_queries_logs() -> u32;
    fn oracle_device_queries_level() -> i32;
    fn oracle_device_queries_log() -> *const c_char;
}

#[derive(Clone, Copy, Debug)]
struct Step {
    context: bool,
    type_callback: bool,
    adapter_callback: bool,
    adapters: bool,
    device_type: i32,
    count: u32,
}

#[derive(Debug, PartialEq, Eq)]
struct Observed {
    value: i64,
    type_calls: u32,
    adapter_calls: u32,
    logs: u32,
    level: i32,
    message: Vec<u8>,
}

fn run(ported: bool, step: Step) -> Observed {
    // SAFETY: the fixture, context, logging and recording callbacks are all
    // thread-local. Live type queries always have their required callback.
    unsafe {
        oracle_device_queries_reset(
            step.context,
            step.type_callback,
            step.adapter_callback,
            step.device_type,
            step.count,
        );
        let value = match (ported, step.adapters) {
            (true, false) => i64::from(rust::gs_get_device_type()),
            (false, false) => i64::from(oracle_gs_get_device_type()),
            (true, true) => i64::from(rust::gs_get_adapter_count()),
            (false, true) => i64::from(oracle_gs_get_adapter_count()),
        };
        Observed {
            value,
            type_calls: oracle_device_queries_type_calls(),
            adapter_calls: oracle_device_queries_adapter_calls(),
            logs: oracle_device_queries_logs(),
            level: oracle_device_queries_level(),
            message: CStr::from_ptr(oracle_device_queries_log())
                .to_bytes()
                .to_vec(),
        }
    }
}

fn compare(step: Step) {
    let original = run(false, step);
    let ported = run(true, step);
    assert_eq!(ported, original, "{step:?}");
    if !step.context {
        assert_eq!(ported.value, if step.adapters { 0 } else { -1 });
        assert_eq!(
            (
                ported.type_calls,
                ported.adapter_calls,
                ported.logs,
                ported.level
            ),
            (0, 0, 1, 400)
        );
        let function = if step.adapters {
            "gs_get_adapter_count"
        } else {
            "gs_get_device_type"
        };
        assert_eq!(
            ported.message,
            format!("{function}: called while not in a graphics context").as_bytes()
        );
    } else {
        assert_eq!(ported.logs, 0);
        assert_eq!(ported.level, 0);
        assert!(ported.message.is_empty());
        if step.adapters {
            assert_eq!(ported.type_calls, 0);
            assert_eq!(ported.adapter_calls, u32::from(step.adapter_callback));
            assert_eq!(
                ported.value,
                if step.adapter_callback {
                    i64::from(step.count)
                } else {
                    0
                }
            );
        } else {
            assert_eq!((ported.type_calls, ported.adapter_calls), (1, 0));
            assert_eq!(ported.value, i64::from(step.device_type));
        }
    }
}

#[test]
fn characterized_matrix_matches_c() {
    for context in [false, true] {
        for type_callback in [false, true] {
            for adapter_callback in [false, true] {
                for adapters in [false, true] {
                    if context && !adapters && !type_callback {
                        continue; // C would call a missing required function pointer.
                    }
                    for device_type in [i32::MIN, -7, -1, 0, 1, 2, 42, i32::MAX] {
                        for count in [0, 1, 2, 37, 0x8000_0000, u32::MAX] {
                            compare(Step {
                                context,
                                type_callback,
                                adapter_callback,
                                adapters,
                                device_type,
                                count,
                            });
                        }
                    }
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
    fn random_queries_match_c(steps in prop::collection::vec(
        (any::<bool>(), any::<bool>(), any::<bool>(), any::<bool>(), any::<i32>(), any::<u32>()), 1..=16,
    )) {
        for (context, type_callback, adapter_callback, adapters, device_type, count) in steps {
            compare(Step { context, type_callback: type_callback || (context && !adapters), adapter_callback, adapters, device_type, count });
        }
    }
}
