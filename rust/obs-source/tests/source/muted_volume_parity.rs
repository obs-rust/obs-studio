//! Tier 3: the original C `source_muted()` and `get_source_volume()`
//! (`source_muted_volume.c`, the extracted file compiled renamed) against
//! the Rust exports, each call on its own real-layout source set to the
//! same fields. Both sides load `user_muted` through the oracle's counting
//! `os_atomic_load_bool()` (the Rust side through the production adapters).
//! Every call's result (as bits), the count of `user_muted` loads and every
//! byte of the source after it must match, over every combination of the
//! flags with edge stop times, delays and clocks, the characterized press
//! and release sequence, and random call sequences on one source. Seven
//! mutants of the exports, built on the same adapters, must not.
//! No intentional differences or exclusions.
use core::ffi::{c_int, c_void};
use obs_source::ffi::source_muted_volume as rust;
use obs_source::{ptm_gate, volume_apply};
use proptest::prelude::*;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Fields {
    enabled: bool,
    user_muted: bool,
    muted: bool,
    key_enabled: [bool; 2],
    user_pressed: [bool; 2],
    pressed: [bool; 2],
    delay: [u64; 2],
    stop: [u64; 2],
    volume: f32,
}

unsafe extern "C" {
    fn oracle_source_muted(source: *mut c_void, os_time: u64) -> bool;
    fn oracle_get_source_volume(source: *mut c_void, os_time: u64) -> f32;
    fn oracle_smv_source_new() -> *mut c_void;
    fn oracle_smv_source_free(source: *mut c_void);
    fn oracle_smv_set(source: *mut c_void, fields: *const Fields);
    fn oracle_smv_get(source: *const c_void, fields: *mut Fields);
    fn oracle_smv_source_size() -> usize;
    fn oracle_smv_loads() -> u32;

    // The production adapters, for the mutants.
    fn obs_source_muted_volume_key_enabled(source: *const c_void, key: c_int) -> bool;
    fn obs_source_muted_volume_user_pressed(source: *const c_void, key: c_int) -> bool;
    fn obs_source_muted_volume_pressed(source: *const c_void, key: c_int) -> bool;
    fn obs_source_muted_volume_delay(source: *const c_void, key: c_int) -> u64;
    fn obs_source_muted_volume_stop_time(source: *const c_void, key: c_int) -> u64;
    fn obs_source_muted_volume_set_stop_time(source: *mut c_void, key: c_int, stop_time: u64);
    fn obs_source_muted_volume_enabled(source: *const c_void) -> bool;
    fn obs_source_muted_volume_user_muted(source: *const c_void) -> bool;
    fn obs_source_muted_volume_muted(source: *const c_void) -> bool;
    fn obs_source_muted_volume_volume(source: *const c_void) -> f32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Muted,
    Volume,
}

#[derive(Debug, Clone, Copy)]
enum Side {
    C,
    Rust,
    Mutant(Mutant),
}

/// The exports' gate on the adapters, with one change each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mutant {
    /// Each key's stop time uses the other key's delay.
    SwapsDelays,
    /// `get_source_volume()` reads the user's pressed flags.
    VolumeUserPressed,
    /// `source_muted()` reads `muted`, not `user_muted`.
    MutedFlag,
    /// `enabled` is not checked.
    IgnoresEnabled,
    /// A pressed key's stop time is written even when it is not enabled.
    WritesDisabledKey,
    /// `user_muted` is loaded even when the source is disabled.
    LoadsWhenDisabled,
    /// The volume is snapped as if never muted.
    SnapsUnmuted,
}

const MUTANTS: [Mutant; 7] = [
    Mutant::SwapsDelays,
    Mutant::VolumeUserPressed,
    Mutant::MutedFlag,
    Mutant::IgnoresEnabled,
    Mutant::WritesDisabledKey,
    Mutant::LoadsWhenDisabled,
    Mutant::SnapsUnmuted,
];

/// A mutant's `source_muted()` (`user`) or `get_source_volume()` gate.
fn mutant_gate(mutant: Mutant, user: bool, source: *mut c_void, os_time: u64) -> bool {
    // SAFETY: the oracle's live source.
    unsafe {
        let pressed_user = user || mutant == Mutant::VolumeUserPressed;
        let pressed = |key| {
            if pressed_user {
                obs_source_muted_volume_user_pressed(source, key)
            } else {
                obs_source_muted_volume_pressed(source, key)
            }
        };
        for key in [0, 1] {
            let enabled = mutant == Mutant::WritesDisabledKey
                || obs_source_muted_volume_key_enabled(source, key);
            if enabled && pressed(key) {
                let delay_key = if mutant == Mutant::SwapsDelays {
                    1 - key
                } else {
                    key
                };
                let delay = obs_source_muted_volume_delay(source, delay_key);
                obs_source_muted_volume_set_stop_time(
                    source,
                    key,
                    ptm_gate::stop_time(os_time, delay),
                );
            }
        }
        let stop = |key: c_int| obs_source_muted_volume_stop_time(source, key);
        let mute_active = pressed(0) || ptm_gate::held(os_time, stop(0));
        let talk_active = pressed(1) || ptm_gate::held(os_time, stop(1));
        let muted_flag = || {
            if user && mutant != Mutant::MutedFlag {
                obs_source_muted_volume_user_muted(source)
            } else {
                obs_source_muted_volume_muted(source)
            }
        };
        let disabled = mutant != Mutant::IgnoresEnabled && !obs_source_muted_volume_enabled(source);
        if mutant == Mutant::LoadsWhenDisabled && disabled && user {
            let _ = obs_source_muted_volume_user_muted(source);
        }
        disabled
            || muted_flag()
            || ptm_gate::mutes(obs_source_muted_volume_key_enabled(source, 0), mute_active)
            || ptm_gate::talk_mutes(obs_source_muted_volume_key_enabled(source, 1), talk_active)
    }
}

/// One call's result as bits (the bool as 0 or 1).
fn call(side: Side, step: Step, source: *mut c_void, os_time: u64) -> u32 {
    // SAFETY: the oracle's live source.
    unsafe {
        match (side, step) {
            (Side::C, Step::Muted) => u32::from(oracle_source_muted(source, os_time)),
            (Side::C, Step::Volume) => oracle_get_source_volume(source, os_time).to_bits(),
            (Side::Rust, Step::Muted) => u32::from(rust::source_muted(source, os_time)),
            (Side::Rust, Step::Volume) => rust::get_source_volume(source, os_time).to_bits(),
            (Side::Mutant(m), Step::Muted) => u32::from(mutant_gate(m, true, source, os_time)),
            (Side::Mutant(m), Step::Volume) => {
                let muted = mutant_gate(m, false, source, os_time) && m != Mutant::SnapsUnmuted;
                volume_apply::snap(muted, obs_source_muted_volume_volume(source)).to_bits()
            }
        }
    }
}

/// After each call: its result, the `user_muted` loads, the two stop times
/// and, when asked for, every byte of the source.
type Outcome = Vec<(u32, u32, [u64; 2], Option<Vec<u8>>)>;

struct Source(*mut c_void);

impl Source {
    fn new(fields: &Fields) -> Self {
        // SAFETY: the oracle allocates a zeroed source; set() writes it.
        unsafe {
            let source = oracle_smv_source_new();
            assert!(!source.is_null());
            oracle_smv_set(source, fields);
            Source(source)
        }
    }
    fn stops(&self) -> [u64; 2] {
        let mut fields = Fields::default();
        // SAFETY: a live source and a mirror of the oracle's struct.
        unsafe { oracle_smv_get(self.0, &raw mut fields) };
        fields.stop
    }
    fn bytes(&self) -> Vec<u8> {
        // SAFETY: the source is that many initialized bytes (bzalloc).
        unsafe {
            std::slice::from_raw_parts(self.0.cast::<u8>(), oracle_smv_source_size()).to_vec()
        }
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        // SAFETY: allocated by oracle_smv_source_new().
        unsafe { oracle_smv_source_free(self.0) }
    }
}

fn run_with(fields: &Fields, calls: &[(Step, u64)], side: Side, bytes: bool) -> Outcome {
    let source = Source::new(fields);
    // SAFETY: clears this thread's count.
    unsafe { oracle_smv_loads() };
    calls
        .iter()
        .map(|&(step, os_time)| {
            let result = call(side, step, source.0, os_time);
            // SAFETY: reads and clears this thread's count.
            let loads = unsafe { oracle_smv_loads() };
            (result, loads, source.stops(), bytes.then(|| source.bytes()))
        })
        .collect()
}

fn run(fields: &Fields, calls: &[(Step, u64)], side: Side) -> Outcome {
    run_with(fields, calls, side, true)
}

fn check(fields: &Fields, calls: &[(Step, u64)]) {
    let c = run(fields, calls, Side::C);
    let rust = run(fields, calls, Side::Rust);
    assert_eq!(c, rust, "{fields:?} {calls:?}");
}

/// Without the byte compare, for the exhaustive sweep.
fn check_light(fields: &Fields, calls: &[(Step, u64)]) {
    let c = run_with(fields, calls, Side::C, false);
    let rust = run_with(fields, calls, Side::Rust, false);
    assert_eq!(c, rust, "{fields:?} {calls:?}");
}

const MS: u64 = ptm_gate::NS_PER_MS;
const T: u64 = 1_000_000 * MS;
const STOPS: [u64; 5] = [0, T - 1, T, T + 1, u64::MAX];
const DELAYS: [u64; 3] = [0, 300, u64::MAX];
const CLOCKS: [u64; 3] = [0, T, u64::MAX];
const VOLUMES: [f32; 6] = [0.5, 1.0, 0.0, 1.00005, -0.0, f32::NAN];

fn flags(bits: u32) -> Fields {
    let bit = |n: u32| bits & (1 << n) != 0;
    Fields {
        enabled: bit(0),
        user_muted: bit(1),
        muted: bit(2),
        key_enabled: [bit(3), bit(4)],
        user_pressed: [bit(5), bit(6)],
        pressed: [bit(7), bit(8)],
        ..Fields::default()
    }
}

#[test]
fn matches_c_over_every_combination() {
    let mut i: u64 = 0;
    for bits in 0..(1u32 << 9) {
        for (s, &stop_mute) in STOPS.iter().enumerate() {
            for &stop_talk in &STOPS {
                for &delay_mute in &DELAYS {
                    for &delay_talk in &DELAYS {
                        i = i.wrapping_add(1);
                        if !obs_c_oracle::parity::keep(i) {
                            continue;
                        }
                        let fields = Fields {
                            stop: [stop_mute, stop_talk],
                            delay: [delay_mute, delay_talk],
                            volume: VOLUMES[(bits as usize + s) % VOLUMES.len()],
                            ..flags(bits)
                        };
                        for &os_time in &CLOCKS {
                            check_light(&fields, &[(Step::Muted, os_time)]);
                            check_light(&fields, &[(Step::Volume, os_time)]);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn matches_c_on_every_volume_bit_edge() {
    let volumes = [
        0.5f32,
        1.0,
        0.0,
        -0.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MIN_POSITIVE,
        1.0001,
        0.9999,
        0.0001,
        -0.0001,
        f32::from_bits(0x7fc0_0001),
        20.0,
    ];
    for volume in volumes {
        for bits in [0b1, 0b0, 0b101, 0b1_1001] {
            let fields = Fields {
                volume,
                stop: [T + 1, 0],
                ..flags(bits)
            };
            check(&fields, &[(Step::Volume, T)]);
        }
    }
}

/// The `test_source_muted_volume` press and release: push-to-mute and
/// push-to-talk held with a 300 ms delay, released, read inside and after
/// the delay, by both steps on one source.
#[test]
fn pins_the_characterized_sequence() {
    for key in [0usize, 1] {
        let mut fields = Fields {
            enabled: true,
            volume: 0.5,
            delay: [300, 300],
            ..Fields::default()
        };
        fields.key_enabled[key] = true;
        fields.user_pressed[key] = true;
        fields.pressed[key] = true;
        let calls = [(Step::Muted, T), (Step::Volume, T + 10 * MS)];
        check(&fields, &calls);
        let c = run(&fields, &calls, Side::C);
        // Pressed: push-to-mute mutes, push-to-talk lets it through.
        let pressed_muted = u32::from(key == 0);
        assert_eq!(c[0].0, pressed_muted);
        assert_eq!(c[0].2[key], T + 300 * MS);
        assert_eq!(c[1].2[key], T + 310 * MS);
        let volume = if key == 0 { 0.0f32 } else { 0.5 };
        assert_eq!(c[1].0, volume.to_bits());
        assert_eq!(c[0].1, 1, "user_muted loaded once");
        assert_eq!(c[1].1, 0, "the mix step loads no user_muted");

        // Released: held inside the delay, then not.
        fields.user_pressed[key] = false;
        fields.pressed[key] = false;
        fields.stop[key] = T + 300 * MS;
        let calls = [
            (Step::Muted, T + 299 * MS),
            (Step::Volume, T + 299 * MS),
            (Step::Muted, T + 300 * MS),
            (Step::Volume, T + 300 * MS),
        ];
        check(&fields, &calls);
        let c = run(&fields, &calls, Side::C);
        let held = |muted: bool| u32::from(muted == (key == 0));
        assert_eq!(c[0].0, held(true));
        assert_eq!(c[2].0, held(false));
        assert_eq!(c[1].0 == 0, key == 0);
        assert_eq!(c[3].0 == 0, key == 1);
        assert!(c.iter().all(|step| step.2[key] == T + 300 * MS));
    }

    // A disabled source loads no user_muted; its pressed keys still write.
    let fields = Fields {
        user_muted: true,
        key_enabled: [true, true],
        user_pressed: [true, true],
        delay: [300, 0],
        ..Fields::default()
    };
    check(&fields, &[(Step::Muted, T)]);
    let c = run(&fields, &[(Step::Muted, T)], Side::C);
    assert_eq!(c[0].0, 1);
    assert_eq!(c[0].1, 0);
    assert_eq!(c[0].2, [T + 300 * MS, T]);
}

#[test]
fn rejects_mutants() {
    let enabled = |f: Fields| Fields {
        enabled: true,
        volume: 0.5,
        ..f
    };
    let cases: [(Mutant, Fields, Step); 7] = [
        (
            Mutant::SwapsDelays,
            enabled(Fields {
                key_enabled: [true, true],
                user_pressed: [true, true],
                delay: [0, 300],
                ..Fields::default()
            }),
            Step::Muted,
        ),
        (
            Mutant::VolumeUserPressed,
            enabled(Fields {
                key_enabled: [true, false],
                user_pressed: [true, false],
                ..Fields::default()
            }),
            Step::Volume,
        ),
        (
            Mutant::MutedFlag,
            enabled(Fields {
                user_muted: true,
                ..Fields::default()
            }),
            Step::Muted,
        ),
        (
            Mutant::IgnoresEnabled,
            Fields {
                volume: 0.5,
                ..Fields::default()
            },
            Step::Volume,
        ),
        (
            Mutant::WritesDisabledKey,
            enabled(Fields {
                user_pressed: [true, false],
                ..Fields::default()
            }),
            Step::Muted,
        ),
        (Mutant::LoadsWhenDisabled, Fields::default(), Step::Muted),
        (
            Mutant::SnapsUnmuted,
            enabled(Fields {
                muted: true,
                ..Fields::default()
            }),
            Step::Volume,
        ),
    ];
    for (mutant, fields, step) in cases {
        let calls = [(step, T)];
        let c = run(&fields, &calls, Side::C);
        assert_eq!(c, run(&fields, &calls, Side::Rust), "{mutant:?}");
        assert_ne!(c, run(&fields, &calls, Side::Mutant(mutant)), "{mutant:?}");
    }
    assert_eq!(MUTANTS.len(), cases.len());
}

fn any_fields() -> impl Strategy<Value = Fields> {
    let u64s = || {
        prop_oneof![
            prop::sample::select(vec![0, 1, T, u64::MAX / MS, u64::MAX]),
            any::<u64>()
        ]
    };
    (
        0..(1u32 << 9),
        [u64s(), u64s()],
        [u64s(), u64s()],
        prop_oneof![
            prop::sample::select(VOLUMES.to_vec()),
            any::<u32>().prop_map(f32::from_bits)
        ],
    )
        .prop_map(|(bits, delay, stop, volume)| Fields {
            delay,
            stop,
            volume,
            ..flags(bits)
        })
}

fn any_calls() -> impl Strategy<Value = Vec<(Step, u64)>> {
    prop::collection::vec(
        (
            prop::sample::select(vec![Step::Muted, Step::Volume]),
            prop_oneof![
                prop::sample::select(vec![0, T, T + 300 * MS, u64::MAX]),
                any::<u64>()
            ],
        ),
        1..10,
    )
}

#[test]
fn matches_c_on_random_sequences() {
    let mut runner = proptest::test_runner::TestRunner::new(ProptestConfig {
        cases: obs_c_oracle::parity::cases(256),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    runner
        .run(&(any_fields(), any_calls()), |(fields, calls)| {
            let c = run(&fields, &calls, Side::C);
            let rust = run(&fields, &calls, Side::Rust);
            prop_assert_eq!(c, rust);
            Ok(())
        })
        .unwrap();
}
