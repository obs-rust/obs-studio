//! Tier 3: real-header C items, original C operations, production atomics, and
//! the existing bounds-transform recorder. No intentional differences. Inputs
//! leave space for 64 operations on the host's actual C long width.
use core::ffi::{c_long, c_void};
use obs_scene::ffi::item_defer_update::{
    obs_sceneitem_defer_update_begin as begin, obs_sceneitem_defer_update_end as end,
};
use proptest::prelude::*;

type Operation = unsafe extern "C" fn(*mut c_void);
const INITIAL_MIN: c_long = -10000;
const INITIAL_MAX: c_long = 10000;
unsafe extern "C" {
    fn oracle_sceneitem_defer_probe(
        initial: c_long,
        flags: u32,
        ops: *const u8,
        count: usize,
        begin: Operation,
        end: Operation,
    ) -> u32;
    fn oracle_sceneitem_defer_null(begin: Operation, end: Operation) -> u32;
}

fn check(initial: c_long, flags: u32, ops: &[u8]) {
    // SAFETY: C owns complete native items and keeps them live for both callbacks.
    assert_eq!(
        unsafe {
            oracle_sceneitem_defer_probe(initial, flags, ops.as_ptr(), ops.len(), begin, end)
        },
        0,
        "initial={initial}, flags={flags}, ops={ops:?}"
    );
}

#[test]
fn nested_negative_zero_crossings_and_native_long_edges() {
    for initial in [-3, -1, 0, 1, 2, 3, c_long::MIN + 64, c_long::MAX - 64] {
        for flags in 0..8 {
            for ops in [
                &[1, 1, 1, 0, 0, 0][..],
                &[0, 0, 1, 1, 1, 0],
                &[1],
                &[0],
                &[],
            ] {
                check(initial, flags, ops);
            }
        }
    }
}

#[test]
fn null_item_logs_exactly_and_touches_nothing() {
    // Logging callbacks are process-wide: isolate this case in the same binary.
    const CHILD: &str = "OBS_SCENEITEM_DEFER_NULL_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let module = module_path!().split_once("::").unwrap().1;
        let selector = format!("{module}::null_item_logs_exactly_and_touches_nothing");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &selector, "--nocapture"])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    // SAFETY: C passes only NULL and restores the process-wide logging callback.
    assert_eq!(unsafe { oracle_sceneitem_defer_null(begin, end) }, 0);
    check(0, 1, &[1, 0]);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn operation_traces_match_c(initial in INITIAL_MIN..INITIAL_MAX,
        flags in 0u32..8, ops in prop::collection::vec(0u8..2, 0..=64)) {
        check(initial, flags, &ops);
    }
}
