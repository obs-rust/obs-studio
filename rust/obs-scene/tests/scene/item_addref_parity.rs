//! Tier 3: the original C and the shim on byte copies of real-header items,
//! through the production atomic adapter. No intentional differences.
use core::ffi::{c_long, c_void};
use obs_scene::ffi::item_addref::obs_sceneitem_addref as addref;
use proptest::prelude::*;

type Operation = unsafe extern "C" fn(*mut c_void);
unsafe extern "C" {
    fn oracle_sceneitem_addref_layout(field: u32) -> usize;
    fn oracle_sceneitem_addref_probe(
        initial: c_long,
        fill: u8,
        times: u32,
        addref: Operation,
    ) -> u32;
    fn oracle_sceneitem_addref_null(addref: Operation) -> u32;
    fn oracle_sceneitem_addref_new(initial: c_long) -> *mut c_void;
    fn oracle_sceneitem_addref_count(item: *const c_void) -> c_long;
    fn oracle_sceneitem_addref_free(item: *mut c_void);
    fn oracle_sceneitem_addref_original(item: *mut c_void);
}

fn check(initial: c_long, fill: u8, times: u32) {
    // SAFETY: C owns complete native items and keeps them live for both calls.
    let result = unsafe { oracle_sceneitem_addref_probe(initial, fill, times, addref) };
    assert_eq!(
        result, 0,
        "initial={initial}, fill={fill:#x}, times={times}"
    );
}

#[test]
fn counter_is_the_native_long_in_the_real_header() {
    // SAFETY: pure layout queries on the real private header.
    let (size, offset, item) = unsafe {
        (
            oracle_sceneitem_addref_layout(0),
            oracle_sceneitem_addref_layout(1),
            oracle_sceneitem_addref_layout(2),
        )
    };
    assert_eq!(size, size_of::<c_long>());
    assert!(offset + size <= item);
}

#[test]
fn null_is_silent_and_touches_nothing() {
    // Logging callbacks are process-wide: isolate this case in the same binary.
    const CHILD: &str = "OBS_SCENEITEM_ADDREF_NULL_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let module = module_path!().split_once("::").unwrap().1;
        let selector = format!("{module}::null_is_silent_and_touches_nothing");
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &selector, "--nocapture"])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    // SAFETY: C passes only NULL and restores the process-wide logging callback.
    assert_eq!(unsafe { oracle_sceneitem_addref_null(addref) }, 0);
    check(0, 0, 1);
}

#[test]
fn characterized_counts_and_native_long_edges() {
    for initial in [
        c_long::MIN,
        c_long::MIN + 1,
        -3,
        -1,
        0,
        1,
        42,
        c_long::MAX - 3,
        c_long::MAX - 1,
        c_long::MAX,
    ] {
        for fill in [0x00, 0xa5, 0xff] {
            for times in 0..=4 {
                check(initial, fill, times);
            }
        }
    }
}

#[test]
fn concurrent_calls_each_add_exactly_one() {
    const THREADS: usize = 4;
    const CALLS: usize = 2000;
    for original in [false, true] {
        // SAFETY: C allocates a zeroed real-header item, freed below.
        let item = unsafe { oracle_sceneitem_addref_new(-5) };
        assert!(!item.is_null());
        let shared = item as usize;
        std::thread::scope(|scope| {
            for _ in 0..THREADS {
                scope.spawn(move || {
                    for _ in 0..CALLS {
                        let item = shared as *mut c_void;
                        // SAFETY: the item stays live until every thread joins.
                        unsafe {
                            if original {
                                oracle_sceneitem_addref_original(item);
                            } else {
                                addref(item);
                            }
                        }
                    }
                });
            }
        });
        // SAFETY: every caller has joined; the item is still live.
        let count = unsafe { oracle_sceneitem_addref_count(item) };
        // SAFETY: allocated by oracle_sceneitem_addref_new, freed once.
        unsafe { oracle_sceneitem_addref_free(item) };
        assert_eq!(
            count,
            -5 + (THREADS * CALLS) as c_long,
            "original={original}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(128), failure_persistence: None,
        ..ProptestConfig::default()
    })]
    #[test]
    fn repeated_calls_match_c(initial in any::<c_long>(), fill in any::<u8>(), times in 0u32..=16) {
        check(initial, fill, times);
    }
}
