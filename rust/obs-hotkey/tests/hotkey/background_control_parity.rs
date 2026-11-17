//! Exact state, wake callback observations and mutex parity; no exclusions.
use core::mem::{align_of, size_of};
use obs_hotkey::ffi::background_control::{
    obs_hotkeys_configure_background, obs_hotkeys_retry_background,
    obs_hotkeys_set_background_probe,
};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_hotkey_control_set(
        present: bool,
        retry: bool,
        configure: bool,
        probe: bool,
        flags: u32,
        callback: bool,
    );
    fn oracle_hotkey_control_snapshot(out: *mut usize);
    fn oracle_hotkey_control_layout(out: *mut usize);
    fn oracle_obs_hotkeys_retry_background();
    fn oracle_obs_hotkeys_configure_background();
    fn oracle_obs_hotkeys_set_background_probe(enable: bool);
}

#[test]
fn real_header_field_layout_and_configure_failed_mask() {
    let mut layout = [0; 7];
    // SAFETY: the C oracle writes exactly seven size_t values.
    unsafe { oracle_hotkey_control_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<bool>(),
            size_of::<bool>(),
            size_of::<bool>(),
            size_of::<u32>(),
            align_of::<bool>(),
            align_of::<u32>(),
            1 << 4
        ]
    );
}

fn compare(
    present: bool,
    retry: bool,
    configure: bool,
    probe: bool,
    flags: u32,
    callback: bool,
    operations: &[u8],
) {
    let run = |rust: bool| {
        // SAFETY: the real-header fixture, counters and callback state are all
        // thread-local. The adapters assert lock ownership on every access;
        // snapshot writes eleven size_t values into our live array.
        unsafe {
            oracle_hotkey_control_set(present, retry, configure, probe, flags, callback);
            operations
                .iter()
                .map(|op| {
                    match (rust, op % 4) {
                        (true, 0) => obs_hotkeys_retry_background(),
                        (true, 1) => obs_hotkeys_configure_background(),
                        (true, 2) => obs_hotkeys_set_background_probe(false),
                        (true, _) => obs_hotkeys_set_background_probe(true),
                        (false, 0) => oracle_obs_hotkeys_retry_background(),
                        (false, 1) => oracle_obs_hotkeys_configure_background(),
                        (false, 2) => oracle_obs_hotkeys_set_background_probe(false),
                        (false, _) => oracle_obs_hotkeys_set_background_probe(true),
                    }
                    let mut snapshot = [0; 11];
                    oracle_hotkey_control_snapshot(snapshot.as_mut_ptr());
                    snapshot
                })
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(run(true), run(false));
}

#[test]
fn characterized_edges_and_repeated_requests() {
    for present in [false, true] {
        for callback in [false, true] {
            for retry in [false, true] {
                for configure in [false, true] {
                    for probe in [false, true] {
                        for flags in [0, 1 << 4, u32::MAX, !(1 << 4)] {
                            compare(
                                present,
                                retry,
                                configure,
                                probe,
                                flags,
                                callback,
                                &[2, 0, 0, 1, 1, 3, 3, 2, 2],
                            );
                        }
                    }
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn differential(present in any::<bool>(), retry in any::<bool>(), configure in any::<bool>(), probe in any::<bool>(), flags in any::<u32>(), callback in any::<bool>(), operations in prop::collection::vec(0u8..4, 1..64)) {
        compare(present, retry, configure, probe, flags, callback, &operations);
    }
}
