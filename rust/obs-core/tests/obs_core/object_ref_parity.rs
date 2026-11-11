//! Tier 3 over the original C, including the original generic weak helpers.
//! No intentional differences. Excludes signed overflow and release without
//! strong ownership/control. Globals/logging/allocation probes re-exec in isolation.
use core::{
    ffi::{c_long, c_void},
    mem::{align_of, offset_of, size_of},
};
use obs_core::ffi::{
    canvas_ref::WeakRef,
    object_ref::{WeakObject, obs_object_get_ref, obs_object_release},
};
use proptest::test_runner::{Config, TestRunner};

unsafe extern "C" {
    fn oracle_object_ref_layout(layout: *mut usize);
    fn oracle_object_ref_probe(
        operation: u32,
        flags: u32,
        kind: u32,
        refs: c_long,
        weak: c_long,
        get: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        release: unsafe extern "C" fn(*mut c_void),
    ) -> bool;
    fn oracle_object_ref_final_free(kind: u32, release: unsafe extern "C" fn(*mut c_void)) -> bool;
}

#[test]
fn real_header_layout() {
    let mut layout = [0; 8];
    // SAFETY: the oracle writes exactly eight native size/alignment/offset values.
    unsafe { oracle_object_ref_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<WeakRef>(),
            align_of::<WeakRef>(),
            offset_of!(WeakRef, refs),
            offset_of!(WeakRef, weak_refs),
            size_of::<WeakObject>(),
            align_of::<WeakObject>(),
            offset_of!(WeakObject, counts),
            offset_of!(WeakObject, object),
        ]
    );
}

fn check(operation: u32, flags: u32, kind: u32, refs: c_long, weak: c_long) {
    // SAFETY: C owns the real-header fixtures; valid ownership is enforced by
    // the callers below, with weak >= 1 retaining stack controls across destroy.
    assert!(
        unsafe {
            oracle_object_ref_probe(
                operation,
                flags,
                kind,
                refs,
                weak,
                obs_object_get_ref,
                obs_object_release,
            )
        },
        "operation={operation} flags={flags} kind={kind} refs={refs} weak={weak}"
    );
}

#[test]
fn characterized_edges_and_random_sequences_match_c() {
    const CHILD: &str = "OBS_OBJECT_REF_PARITY_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let name = format!(
            "{}::characterized_edges_and_random_sequences_match_c",
            module_path!().split_once("::").unwrap().1
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    for kind in 0..6 {
        for refs in [c_long::MIN, -3, -2, -1, 0, 1, 7, 1023, c_long::MAX - 2] {
            for flags in 0..32 {
                check(0, flags, kind, refs, 3);
            }
        }
        for refs in [0, 1, 7, 1023, c_long::MAX - 2] {
            for flags in [0, 1, 4, 8, 9, 12, 13] {
                check(1, flags, kind, refs, 1);
            }
            for flags in [0, 4, 16, 20] {
                check(2, flags, kind, refs, 4);
            }
        }
        // Shutdown guard precedes NULL/control access, even for expired controls.
        for flags in [3, 8, 10, 11] {
            check(1, flags, kind, -3, 1);
        }
        // SAFETY: the C fixture transfers one bzalloc control to final release
        // and checks the callback and allocation balance without reading freed data.
        assert!(unsafe { oracle_object_ref_final_free(kind, obs_object_release) });
    }
    let strong_counts: std::ops::Range<c_long> = -3..4096;
    let weak_counts: std::ops::Range<c_long> = 1..16;
    let strategy = (0u32..3, 0u32..32, 0u32..6, strong_counts, weak_counts);
    TestRunner::new(Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Config::default()
    })
    .run(&strategy, |(operation, mut flags, kind, mut refs, weak)| {
        if operation == 1 {
            flags &= 13;
            refs = refs.max(0);
        }
        if operation == 2 {
            flags &= 20;
            refs = refs.max(0);
        }
        check(operation, flags, kind, refs, weak);
        Ok(())
    })
    .unwrap();
}
