//! Original C versus the production ABI dispatch and safe recorder. All size_t
//! values are defined inputs; no intentional differences or exclusions.
use obs_hotkey::ffi::unregister_entry as ffi;
use obs_hotkey::unregister_entry::{Removal, unregister};
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_hotkey_unregister_entry_reset(present: bool);
    fn oracle_obs_hotkey_unregister(id: usize);
    fn oracle_obs_hotkey_pair_unregister(id: usize);
    fn oracle_hotkey_unregister_entry_lock() -> bool;
    fn oracle_hotkey_unregister_entry_unlock();
    fn oracle_hotkey_unregister_entry_single(id: usize);
    fn oracle_hotkey_unregister_entry_pair(id: usize);
    fn oracle_hotkey_unregister_entry_snapshot(trace: *mut usize) -> usize;
}

fn compare(present: bool, calls: &[(usize, bool)]) {
    assert!(calls.len() <= 24);
    let run = |original| {
        let mut trace = [0usize; 128];
        // SAFETY: a bounded thread-local recorder shares exactly the same
        // synchronous lock/remover contracts as production. Snapshot writes
        // exactly 128 size_t values, with no C layouts crossing this boundary.
        let count = unsafe {
            oracle_hotkey_unregister_entry_reset(present);
            for &(id, pair) in calls {
                if original {
                    if pair {
                        oracle_obs_hotkey_pair_unregister(id);
                    } else {
                        oracle_obs_hotkey_unregister(id);
                    }
                } else {
                    ffi::with_seams(
                        id,
                        if pair { Removal::Pair } else { Removal::Single },
                        oracle_hotkey_unregister_entry_lock,
                        oracle_hotkey_unregister_entry_single,
                        oracle_hotkey_unregister_entry_pair,
                        oracle_hotkey_unregister_entry_unlock,
                    );
                }
            }
            oracle_hotkey_unregister_entry_snapshot(trace.as_mut_ptr())
        };
        trace[..count].to_vec()
    };
    let expected = run(true);
    assert_eq!(run(false), expected);
    let mut recorder = super::unregister_entry::Recorder {
        present,
        ..Default::default()
    };
    let mut trace = Vec::new();
    for &(id, pair) in calls {
        unregister(
            &mut recorder,
            id,
            if pair { Removal::Pair } else { Removal::Single },
        );
        if present {
            trace.extend([1, if pair { 3 } else { 2 }, id, 4]);
        }
    }
    assert_eq!(trace, expected);
    assert_eq!(
        recorder.singles,
        calls
            .iter()
            .filter(|c| present && !c.1)
            .map(|c| c.0)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        recorder.pairs,
        calls
            .iter()
            .filter(|c| present && c.1)
            .map(|c| c.0)
            .collect::<Vec<_>>()
    );
    let order: Vec<_> = calls
        .iter()
        .filter(|_| present)
        .flat_map(|c| {
            if c.1 {
                b"LU".as_slice()
            } else {
                b"LRU".as_slice()
            }
        })
        .copied()
        .collect();
    assert_eq!(recorder.order, order);
    assert!(!recorder.locked);
}

#[test]
fn differential() {
    let edges = [
        0,
        1,
        3,
        4,
        u32::MAX as usize,
        (u32::MAX as usize).wrapping_add(1),
        1usize << (usize::BITS - 1),
        usize::MAX - 1,
        usize::MAX,
    ];
    for present in [false, true] {
        for id in edges {
            compare(present, &[(id, false), (id, true), (id, true), (id, false)]);
        }
        compare(present, &[]);
    }
    let strategy = (
        any::<bool>(),
        prop::collection::vec((any::<usize>(), any::<bool>()), 0..=24),
    );
    proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..Default::default()
    })
    .run(&strategy, |(present, calls)| {
        compare(present, &calls);
        Ok(())
    })
    .unwrap();
}
