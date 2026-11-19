//! Tier 3: extracted original C, the production ABI conversion/dispatch, and
//! safe-core recorder. No intentional differences or excluded defined inputs.
use core::ffi::c_void;
use obs_hotkey::ffi::inject_entry::{self as ffi, Visitor};
use obs_hotkey::ffi::nix_hotkey_state::obs_key_combination;
use obs_hotkey::ffi::binding_press::obs_hotkey_binding;
use obs_hotkey::inject_entry::inject;
use obs_hotkey::name_map::{KEY_NONE, OBS_KEY_A, OBS_KEY_MOUSE3};
use obs_hotkey::press::KeyCombination;
use proptest::prelude::*;

unsafe extern "C" {
    fn oracle_hotkey_inject_entry_reset(present: bool, strict: bool, count: usize, mutate: bool);
    fn oracle_obs_hotkey_inject_event(key: obs_key_combination, pressed: bool);
    fn oracle_hotkey_inject_entry_lock() -> *mut c_void;
    fn oracle_hotkey_inject_entry_strict(core: *const c_void) -> bool;
    fn oracle_hotkey_inject_entry_walk(visitor: Visitor, data: *mut c_void);
    fn oracle_hotkey_inject_entry_binding(
        binding: *mut obs_hotkey_binding,
        key: obs_key_combination,
        pressed: bool,
        strict: bool,
    );
    fn oracle_hotkey_inject_entry_unlock();
    fn oracle_hotkey_inject_entry_snapshot(state: *mut usize, visits: *mut u32);
    fn oracle_hotkey_inject_entry_layout(out: *mut usize);
}

fn compare(
    present: bool,
    strict: bool,
    count: usize,
    mutate: bool,
    key: KeyCombination,
    pressed: bool,
) {
    let run = |original| {
        let mut state = [0; 7];
        let mut visits = [[0; 5]; 16];
        // SAFETY: bounded thread-local C world, no allocations or shared global
        // state. The adapters and callbacks finish before snapshot copies the
        // exact seven size_t and sixteen five-u32 records to these arrays.
        unsafe {
            oracle_hotkey_inject_entry_reset(present, strict, count, mutate);
            let combo = obs_key_combination {
                modifiers: key.modifiers,
                key: key.key,
            };
            if original {
                oracle_obs_hotkey_inject_event(combo, pressed);
            } else {
                ffi::with_seams(
                    combo,
                    pressed,
                    oracle_hotkey_inject_entry_lock,
                    oracle_hotkey_inject_entry_strict,
                    oracle_hotkey_inject_entry_walk,
                    oracle_hotkey_inject_entry_binding,
                    oracle_hotkey_inject_entry_unlock,
                );
            }
            oracle_hotkey_inject_entry_snapshot(state.as_mut_ptr(), visits.as_mut_ptr().cast());
        }
        (state, visits)
    };
    let expected = run(true);
    assert_eq!(run(false), expected);

    let mut recorder = super::inject_entry::Recorder {
        present,
        strict_at_lock: strict,
        count,
        ..Default::default()
    };
    recorder.strict.set(!strict);
    inject(&mut recorder, key, pressed);
    let mut visits = [[0; 5]; 16];
    for (index, event) in &recorder.visits {
        visits[*index] = [
            *index as u32,
            event.hotkey.modifiers,
            event.hotkey.key as u32,
            u32::from(event.pressed),
            u32::from(event.strict_modifiers),
        ];
    }
    let trace = recorder.order.iter().fold(0usize, |n, b| {
        n * 4
            + match b {
                b'L' => 1,
                b'E' => 0,
                b'B' => 2,
                b'U' => 3,
                _ => unreachable!(),
            }
    });
    assert_eq!(
        expected,
        (
            [
                recorder.locks,
                recorder.enumerations,
                recorder.unlocks,
                recorder.visits.len(),
                trace,
                usize::from(recorder.strict.get()),
                if present && count > 0 && mutate {
                    0
                } else {
                    count
                }
            ],
            visits
        )
    );
    assert!(!recorder.locked);
}

#[test]
fn real_header_combination_layout() {
    let mut layout = [0; 4];
    // SAFETY: writes exactly four size_t values measured from the real header.
    unsafe { oracle_hotkey_inject_entry_layout(layout.as_mut_ptr()) };
    assert_eq!(
        layout,
        [
            size_of::<obs_key_combination>(),
            align_of::<obs_key_combination>(),
            core::mem::offset_of!(obs_key_combination, modifiers),
            core::mem::offset_of!(obs_key_combination, key)
        ]
    );
}

#[test]
fn differential() {
    for present in [false, true] {
        for strict in [false, true] {
            for pressed in [false, true] {
                for count in [0, 1, 3, 16] {
                    for key in [KEY_NONE, OBS_KEY_A, OBS_KEY_MOUSE3] {
                        for modifiers in [0, 1, 4, u32::MAX] {
                            for mutate in [false, true] {
                                compare(
                                    present,
                                    strict,
                                    count,
                                    mutate,
                                    KeyCombination { modifiers, key },
                                    pressed,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    let strategy = (
        any::<bool>(),
        any::<bool>(),
        0usize..=16,
        any::<bool>(),
        prop::sample::select(vec![KEY_NONE, OBS_KEY_A, OBS_KEY_MOUSE3]),
        any::<u32>(),
        any::<bool>(),
    );
    proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: obs_c_oracle::parity::cases(128),
        failure_persistence: None,
        ..Default::default()
    })
    .run(
        &strategy,
        |(present, strict, count, mutate, key, modifiers, pressed)| {
            compare(
                present,
                strict,
                count,
                mutate,
                KeyCombination { modifiers, key },
                pressed,
            );
            Ok(())
        },
    )
    .unwrap();
}
