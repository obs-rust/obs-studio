//! Tier 3: actual Rust export versus the unchanged extracted C, real-header
//! fixtures and production adapters. No-core, invalid/freed nodes, cyclic lists
//! and concurrent mutation are C UB and outside the generated domain. No
//! intentional behavior differences; no private structs are mirrored in Rust.
use obs_core::ffi::module_post_load::obs_post_load_modules;
use proptest::prelude::*;

type Entry = unsafe extern "C" fn();
unsafe extern "C" {
    fn oracle_obs_post_load_modules();
    fn oracle_module_post_load_run(
        entry: Entry,
        order: *const u32,
        enabled: u32,
        callbacks: u32,
        loaded: u32,
        states: *const i32,
        mutations: u32,
        host: u32,
        repeats: u32,
        report: *mut u64,
    );
    fn oracle_module_post_load_layout(out: *mut usize);
}

#[derive(Debug)]
struct Case {
    order: [u32; 8],
    enabled: u32,
    callbacks: u32,
    loaded: u32,
    states: [i32; 8],
    mutations: u32,
    host: u32,
    repeats: u32,
}
fn compare(case: &Case) {
    let mut c = [0u64; 128];
    let mut rust = [0u64; 128];
    for (entry, report) in [
        (oracle_obs_post_load_modules as Entry, &mut c),
        (obs_post_load_modules as Entry, &mut rust),
    ] {
        // SAFETY: arrays have eight initialized elements, order is a permutation,
        // enabled <= 8, repeats <= 3 and report has exactly 128 elements. The
        // fixture owns live real-header nodes and resets the shared seam on exit.
        unsafe {
            oracle_module_post_load_run(
                entry,
                case.order.as_ptr(),
                case.enabled,
                case.callbacks,
                case.loaded,
                case.states.as_ptr(),
                case.mutations,
                case.host,
                case.repeats,
                report.as_mut_ptr(),
            );
        }
    }
    assert_eq!(rust, c, "{case:?}");
    assert_eq!(c[3], 1, "unrelated record bytes changed: {case:?}");
    assert!(!c[16..80].contains(&0xffff), "detached node called");
    // Disabled-list records preserve every reported field, even when flags
    // characterize them as enabled. They never enter the callback trace.
    for &id in &case.order[case.enabled as usize..] {
        let i = id as usize;
        assert_eq!(c[80 + i * 4], u64::from(case.loaded & (1 << i) != 0));
        assert_eq!(c[81 + i * 4], u64::from(case.callbacks & (1 << i) != 0));
        assert_eq!(c[82 + i * 4], case.states[i] as u64);
    }
}

#[test]
fn original_c_dispatch_order_state_and_callback_mutations() {
    const CHILD: &str = "OBS_MODULE_POST_LOAD_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(format!(
                "{}::original_c_dispatch_order_state_and_callback_mutations",
                module_path!().split_once("::").unwrap().1
            ))
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let mut layout = [0usize; 4];
    // SAFETY: C writes four values measured from the real callback field.
    unsafe { oracle_module_post_load_layout(layout.as_mut_ptr()) };
    assert_eq!(layout[0], size_of::<Option<Entry>>());
    assert_eq!(layout[1], align_of::<Option<Entry>>());
    assert_eq!(&layout[2..], &[1, 1]);

    for enabled in [0, 1, 5, 8] {
        for callbacks in [0, 1, 0x55, 0xff] {
            for mutations in 0..16 {
                for host in 0..3 {
                    compare(&Case {
                        order: [5, 2, 3, 0, 1, 4, 7, 6],
                        enabled,
                        callbacks,
                        loaded: 0x55,
                        states: [0, 1, 2, 3, 4, 5, 6, 1],
                        mutations,
                        host,
                        repeats: 3,
                    });
                }
            }
        }
    }
    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Default::default()
    });
    runner
        .run(
            &(
                any::<[u8; 8]>(),
                0u32..9,
                0u32..256,
                0u32..256,
                prop::array::uniform8(0i32..7),
                0u32..16,
                0u32..3,
                1u32..4,
            ),
            |(keys, enabled, callbacks, loaded, states, mutations, host, repeats)| {
                let mut order = [0, 1, 2, 3, 4, 5, 6, 7];
                order.sort_by_key(|&i| keys[i as usize]);
                compare(&Case {
                    order,
                    enabled,
                    callbacks,
                    loaded,
                    states,
                    mutations,
                    host,
                    repeats,
                });
                Ok(())
            },
        )
        .unwrap();
}
