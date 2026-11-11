//! Compare every item byte after each operation with original C. No exclusions;
//! bounded initial values leave room for 64 operations even on 32-bit C long.
use core::ffi::c_void;
use obs_scene::ffi::group_resize_defer::{
    obs_sceneitem_defer_group_resize_begin as begin, obs_sceneitem_defer_group_resize_end as end,
};
use proptest::prelude::*;

type Operation = unsafe extern "C" fn(*mut c_void);
unsafe extern "C" {
    fn oracle_scene_group_resize_defer_probe(
        initial: i32,
        flag: bool,
        ops: *const u8,
        count: usize,
        begin: Operation,
        end: Operation,
    ) -> u32;
    fn oracle_scene_group_resize_defer_null(begin: Operation, end: Operation) -> u32;
}

fn check(initial: i32, flag: bool, ops: &[u8]) {
    // SAFETY: C owns complete real-header items; callbacks run only while live.
    assert_eq!(
        unsafe {
            oracle_scene_group_resize_defer_probe(
                initial,
                flag,
                ops.as_ptr(),
                ops.len(),
                begin,
                end,
            )
        },
        0,
        "initial={initial}, flag={flag}, ops={ops:?}"
    );
}

#[test]
fn nested_and_unbalanced_edges() {
    for count in [-3, -1, 0, 1, 2, 3, i32::MIN + 64, i32::MAX - 64] {
        for flag in [false, true] {
            for ops in [&[1, 1, 0, 0][..], &[0, 0, 1, 1], &[1], &[0]] {
                check(count, flag, ops);
            }
        }
    }
}

#[test]
fn null_diagnostics_match_c() {
    // Logging callbacks are process-wide: re-exec this exact category case.
    const CHILD: &str = "OBS_GROUP_RESIZE_DEFER_NULL_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let module = module_path!().split_once("::").unwrap().1;
        let selector = format!("{module}::null_diagnostics_match_c");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &selector, "--nocapture"])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    // SAFETY: the C harness passes only NULL and restores the logging callback.
    assert_eq!(
        unsafe { oracle_scene_group_resize_defer_null(begin, end) },
        0
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn sequences_match_c(initial in -10000i32..10000, flag in any::<bool>(),
        ops in prop::collection::vec(0u8..2, 0..=64)) {
        check(initial, flag, &ops);
    }
}
