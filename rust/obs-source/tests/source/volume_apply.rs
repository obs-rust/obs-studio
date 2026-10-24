//! Tier 1: `obs_source::volume_apply`, the volume decisions of
//! `get_source_volume()` and `apply_audio_volume()`.
//!
//! The `steps_*` cases port `test/cmocka/test_source_volume_apply_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the stand-in source and the copies of
//! `get_source_volume()` and `apply_audio_volume()` included (the gate
//! through `obs_source::ptm_gate`, as `obs-source.c.h` calls it).
//!
//! The `edge_*` cases are new: the snap against the rule computed in `f64`
//! over every float near both edges, a stride through all 2^32 bit patterns
//! and a proptest over any bit pattern; the step against the IEEE compares
//! written another way; the due compare against `u128` arithmetic over the
//! full `u64` range; the mix choice for every mix index up to 64; and both
//! compositions over any flags, stop times, volumes and masks.

use obs_source::ptm_gate::{held, mutes, stop_time, talk_mutes};
use obs_source::volume_apply::{
    MAX_AUDIO_MIXES, SNAP_EPSILON, Step, action_due, mixes, snap, step,
};
use proptest::prelude::*;

const MS: u64 = 1_000_000;
const SEC: u64 = 1000 * MS;
/// A clock well away from 0 and `u64::MAX`.
const T: u64 = 1000 * SEC;
const MIXES: usize = 6;
const CHANNELS: usize = 8;
const FRAMES: usize = 1024;
/// `conv_frames_to_time(48000, AUDIO_OUTPUT_FRAMES)`.
const TICK: u64 = 21_333_333;

/// The last float below 1 that snaps to 1 and the first that does not.
const ONE_LOW_IN: f32 = 1.0 - 1677.0 / 16_777_216.0;
const ONE_LOW_OUT: f32 = 1.0 - 1678.0 / 16_777_216.0;
const ONE_HIGH_IN: f32 = 1.0 + 838.0 / 8_388_608.0;
const ONE_HIGH_OUT: f32 = 1.0 + 839.0 / 8_388_608.0;

const MASK_EDGES: [u32; 12] = [
    0,
    1,
    2,
    0x20,
    0x3f,
    0x40,
    0x41,
    0x7f,
    0x80,
    0xff,
    0x8000_0000,
    u32::MAX,
];

fn bitsf(bits: u32) -> f32 {
    f32::from_bits(bits)
}

/// `nextafterf(from, to)` for finite `from`, as the C test uses it.
fn next_toward(from: f32, to: f32) -> f32 {
    if from == to {
        return to;
    }
    if from == 0.0 {
        let tiny = bitsf(1);
        return if to > 0.0 { tiny } else { -tiny };
    }
    let bits = from.to_bits();
    let away = (to > from) == (from > 0.0);
    bitsf(if away { bits + 1 } else { bits - 1 })
}

#[track_caller]
fn check_float(actual: f32, expected: f32, what: &str) {
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{what} is {actual:e} ({:#010x}), expected {expected:e} ({:#010x})",
        actual.to_bits(),
        expected.to_bits()
    );
}

#[track_caller]
fn check_snap(muted: bool, volume: f32, expected: f32) {
    check_float(
        snap(muted, volume),
        expected,
        &format!("snap({muted}, {volume:e})"),
    );
}

#[track_caller]
fn kept(volume: f32) {
    check_snap(false, volume, volume);
}

#[track_caller]
fn check_due(action_ts: u64, audio_ts: u64, duration: u64, expected: bool) {
    assert_eq!(
        action_due(action_ts, audio_ts, duration),
        expected,
        "action_due({action_ts}, {audio_ts}, {duration})"
    );
}

#[track_caller]
fn check_step(vol: f32, mixers: u32, expected: Step) {
    assert_eq!(
        step(vol, mixers),
        expected,
        "step({vol:e} ({:#010x}), {mixers:#x})",
        vol.to_bits()
    );
}

// ---------------------------------------------------------------------------
// snap()

#[test]
fn steps_snap_zero_edge() {
    check_snap(false, 0.0, 0.0);
    // -0 snaps to +0
    check_snap(false, -0.0, 0.0);
    check_snap(false, 0.0001, 0.0);
    check_snap(false, -0.0001, 0.0);
    check_snap(false, next_toward(0.0001, 0.0), 0.0);
    check_snap(false, next_toward(-0.0001, 0.0), 0.0);
    kept(next_toward(0.0001, 1.0));
    kept(next_toward(-0.0001, -1.0));
    kept(next_toward(next_toward(0.0001, 1.0), 1.0));
    // the smallest subnormal and f32::MIN_POSITIVE, of either sign
    check_snap(false, bitsf(1), 0.0);
    check_snap(false, bitsf(0x8000_0001), 0.0);
    check_snap(false, f32::MIN_POSITIVE, 0.0);
    check_snap(false, -f32::MIN_POSITIVE, 0.0);
    // -60 dB is 0.001: kept
    kept(0.001);
    kept(0.00011);
}

#[test]
fn steps_snap_one_edge() {
    // the edges are float neighbours
    assert_eq!(
        next_toward(ONE_LOW_IN, 0.0).to_bits(),
        ONE_LOW_OUT.to_bits()
    );
    assert_eq!(
        next_toward(ONE_HIGH_IN, 2.0).to_bits(),
        ONE_HIGH_OUT.to_bits()
    );

    check_snap(false, 1.0, 1.0);
    check_snap(false, next_toward(1.0, 0.0), 1.0);
    check_snap(false, next_toward(1.0, 2.0), 1.0);
    check_snap(false, ONE_LOW_IN, 1.0);
    check_snap(false, next_toward(ONE_LOW_IN, 1.0), 1.0);
    kept(ONE_LOW_OUT);
    kept(next_toward(ONE_LOW_OUT, 0.0));
    check_snap(false, ONE_HIGH_IN, 1.0);
    check_snap(false, next_toward(ONE_HIGH_IN, 1.0), 1.0);
    kept(ONE_HIGH_OUT);
    kept(next_toward(ONE_HIGH_OUT, 2.0));

    // characterized, not endorsed: none of these snaps
    let one_minus: f32 = 1.0 - 0.0001_f32;
    let one_plus: f32 = 1.0 + 0.0001_f32;
    assert_eq!(one_minus.to_bits(), ONE_LOW_OUT.to_bits());
    assert_eq!(one_plus.to_bits(), ONE_HIGH_OUT.to_bits());
    assert_eq!(0.9999_f32.to_bits(), ONE_LOW_OUT.to_bits());
    assert_eq!(1.0001_f32.to_bits(), ONE_HIGH_OUT.to_bits());
    kept(one_minus);
    kept(one_plus);
    kept(0.9999);
    kept(1.0001);
    check_snap(false, 0.99995, 1.0);
    check_snap(false, 1.00005, 1.0);

    // -1 is 2 away from 1
    kept(-1.0);
    kept(-next_toward(1.0, 0.0));
}

#[test]
fn steps_snap_keeps_other_volumes() {
    kept(0.5);
    kept(2.0);
    // the most the volume controls set, +26 dB
    kept(20.0);
    kept(f32::MAX);
    kept(-f32::MAX);
    kept(-0.5);
    // +-inf are infinitely far from 0 and 1
    kept(f32::INFINITY);
    kept(f32::NEG_INFINITY);
    // NaN compares false both times and passes through with its bits
    kept(bitsf(0x7fc0_0000));
    kept(bitsf(0xffc0_0000));
    kept(bitsf(0x7fc0_0001));
    kept(bitsf(0xffff_ffff));
    assert!(snap(false, bitsf(0x7f80_0001)).is_nan());
}

#[test]
fn steps_snap_muted() {
    let volumes = [
        0.0,
        -0.0,
        0.0001,
        0.5,
        1.0,
        20.0,
        -1.0,
        f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        bitsf(1),
        ONE_LOW_IN,
        bitsf(0x7fc0_0000),
        bitsf(0xffc0_0000),
    ];
    for volume in volumes {
        check_snap(true, volume, 0.0);
    }
}

// ---------------------------------------------------------------------------
// get_source_volume()

/// The fields of `struct obs_source` `get_source_volume()` reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct VolumeSource {
    enabled: bool,
    muted: bool,
    push_to_mute_enabled: bool,
    push_to_mute_pressed: bool,
    push_to_talk_enabled: bool,
    push_to_talk_pressed: bool,
    push_to_mute_delay: u64,
    push_to_mute_stop_time: u64,
    push_to_talk_delay: u64,
    push_to_talk_stop_time: u64,
    volume: f32,
}

/// `get_source_volume()` of `obs-source.c`, on the stand-in.
fn get_source_volume(source: &mut VolumeSource, os_time: u64) -> f32 {
    if source.push_to_mute_enabled && source.push_to_mute_pressed {
        source.push_to_mute_stop_time = stop_time(os_time, source.push_to_mute_delay);
    }

    if source.push_to_talk_enabled && source.push_to_talk_pressed {
        source.push_to_talk_stop_time = stop_time(os_time, source.push_to_talk_delay);
    }

    let push_to_mute_active =
        source.push_to_mute_pressed || held(os_time, source.push_to_mute_stop_time);
    let push_to_talk_active =
        source.push_to_talk_pressed || held(os_time, source.push_to_talk_stop_time);

    let muted = !source.enabled
        || source.muted
        || mutes(source.push_to_mute_enabled, push_to_mute_active)
        || talk_mutes(source.push_to_talk_enabled, push_to_talk_active);

    snap(muted, source.volume)
}

fn source_of(bits: u8, stops: (u64, u64), volume: f32) -> VolumeSource {
    VolumeSource {
        enabled: bits & 1 != 0,
        muted: bits & 2 != 0,
        push_to_mute_enabled: bits & 4 != 0,
        push_to_mute_pressed: bits & 8 != 0,
        push_to_talk_enabled: bits & 16 != 0,
        push_to_talk_pressed: bits & 32 != 0,
        push_to_mute_delay: 300,
        push_to_talk_delay: 500,
        push_to_mute_stop_time: stops.0,
        push_to_talk_stop_time: stops.1,
        volume,
    }
}

#[test]
fn steps_gate_every_flag() {
    let stops = [0, T - 1, T, T + 1, u64::MAX];
    let mut runs = 0_u32;

    for bits in 0_u8..64 {
        for &ptm in &stops {
            for &ptt in &stops {
                let mut s = source_of(bits, (ptm, ptt), 0.5);
                let ptm_stop = if s.push_to_mute_enabled && s.push_to_mute_pressed {
                    T + 300 * MS
                } else {
                    ptm
                };
                let ptt_stop = if s.push_to_talk_enabled && s.push_to_talk_pressed {
                    T + 500 * MS
                } else {
                    ptt
                };
                let ptm_active = s.push_to_mute_pressed || T < ptm_stop;
                let ptt_active = s.push_to_talk_pressed || T < ptt_stop;
                let muted = !s.enabled
                    || s.muted
                    || (s.push_to_mute_enabled && ptm_active)
                    || (s.push_to_talk_enabled && !ptt_active);

                let vol = get_source_volume(&mut s, T);
                check_float(
                    vol,
                    if muted { 0.0 } else { 0.5 },
                    &format!("flags {bits:#04x}, stops {ptm} {ptt}"),
                );
                assert_eq!(s.push_to_mute_stop_time, ptm_stop, "ptm stop time");
                assert_eq!(s.push_to_talk_stop_time, ptt_stop, "ptt stop time");
                runs += 1;
            }
        }
    }
    assert_eq!(runs, 64 * 25);
}

#[test]
fn steps_gate_then_snap() {
    let mut s = VolumeSource {
        enabled: true,
        push_to_talk_enabled: true,
        push_to_talk_delay: 300,
        ..VolumeSource::default()
    };

    let volumes = [
        0.5,
        1.0,
        ONE_LOW_IN,
        ONE_LOW_OUT,
        0.0001,
        -0.0,
        bitsf(0x7fc0_0000),
        f32::INFINITY,
    ];
    let snapped = [
        0.5,
        1.0,
        1.0,
        ONE_LOW_OUT,
        0.0,
        0.0,
        bitsf(0x7fc0_0000),
        f32::INFINITY,
    ];
    for (&volume, &expected) in volumes.iter().zip(snapped.iter()) {
        s.volume = volume;
        s.push_to_talk_pressed = true;
        check_float(get_source_volume(&mut s, T), expected, "held");
        s.push_to_talk_pressed = false;
        check_float(
            get_source_volume(&mut s, T + 300 * MS - 1),
            expected,
            "inside the delay",
        );
        check_float(
            get_source_volume(&mut s, T + 300 * MS),
            0.0,
            "after the delay",
        );
    }
}

// ---------------------------------------------------------------------------
// action_due()

#[test]
fn steps_action_due_threshold() {
    check_due(T + TICK - 1, T, TICK, true);
    check_due(T + TICK, T, TICK, false);
    check_due(T + TICK + 1, T, TICK, false);
    // an action from before the tick is due
    check_due(T - 1, T, TICK, true);
    check_due(0, T, TICK, true);
    check_due(T, T, TICK, true);
    // a duration of 0: only actions before audio_ts
    check_due(T - 1, T, 0, true);
    check_due(T, T, 0, false);
    check_due(0, 0, 0, false);
    check_due(0, 0, 1, true);
    check_due(u64::MAX, T, TICK, false);
    check_due(u64::MAX - 1, u64::MAX - TICK, TICK, true);
    check_due(u64::MAX - 1, u64::MAX, 0, true);
    check_due(u64::MAX, u64::MAX, 0, false);
}

#[test]
fn steps_action_due_wraps() {
    let audio_ts = u64::MAX - 10;
    // audio_ts + TICK wraps to TICK - 11
    check_due(audio_ts, audio_ts, TICK, false);
    check_due(audio_ts + 5, audio_ts, TICK, false);
    check_due(u64::MAX, audio_ts, TICK, false);
    check_due(TICK - 12, audio_ts, TICK, true);
    check_due(TICK - 11, audio_ts, TICK, false);
    check_due(0, audio_ts, TICK, true);
    // the sum exactly u64::MAX does not wrap; one more wraps to 0
    check_due(u64::MAX - 1, u64::MAX - TICK, TICK, true);
    check_due(u64::MAX - 1, u64::MAX - TICK + 1, TICK, false);
    check_due(0, u64::MAX - TICK + 1, TICK, false);
    check_due(0, u64::MAX, u64::MAX, true);
}

// ---------------------------------------------------------------------------
// step() and mixes()

#[test]
fn steps_step_values() {
    // the C enum's values, as the shims return them
    assert_eq!(u32::from(Step::Skip), 0, "SKIP");
    assert_eq!(u32::from(Step::Clear), 1, "CLEAR");
    assert_eq!(u32::from(Step::Multiply), 2, "MULTIPLY");
    assert_eq!(MAX_AUDIO_MIXES, 6);
}

#[test]
fn steps_step_each_volume_and_mask() {
    for mixers in MASK_EDGES {
        let other = if mixers != 0 {
            Step::Multiply
        } else {
            Step::Clear
        };

        check_step(1.0, mixers, Step::Skip);
        check_step(0.0, mixers, Step::Clear);
        check_step(-0.0, mixers, Step::Clear);
        check_step(next_toward(1.0, 0.0), mixers, other);
        check_step(next_toward(1.0, 2.0), mixers, other);
        check_step(bitsf(1), mixers, other);
        check_step(bitsf(0x8000_0001), mixers, other);
        check_step(0.5, mixers, other);
        check_step(20.0, mixers, other);
        check_step(-1.0, mixers, other);
        check_step(f32::INFINITY, mixers, other);
        check_step(f32::NEG_INFINITY, mixers, other);
        check_step(bitsf(0x7fc0_0000), mixers, other);
        check_step(bitsf(0xffc0_0000), mixers, other);
        check_step(bitsf(0x7f80_0001), mixers, other);
    }
}

#[test]
fn steps_mixes_mask_edges() {
    for a in MASK_EDGES {
        for b in MASK_EDGES {
            for mix in 0..MIXES {
                let bit = 1_u32 << mix;
                assert_eq!(
                    mixes(a, b, mix),
                    a & bit != 0 && b & bit != 0,
                    "mixes({a:#x}, {b:#x}, {mix})"
                );
            }
        }
    }

    assert!(mixes(0x3f, 0x3f, 5));
    assert!(!mixes(0x1f, 0x3f, 5));
    assert!(!mixes(0x3f, 0x1f, 5));
    assert!(!mixes(u32::MAX, 0x40, 5));
    assert!(!mixes(0, u32::MAX, 0));
}

// ---------------------------------------------------------------------------
// apply_audio_volume()

/// What a run of `apply_audio_volume()` did.
#[derive(Debug, Clone, PartialEq, Default)]
struct ApplyRecord {
    actions: bool,
    cleared: bool,
    clear_size: usize,
    /// (mix, channels, volume bits) per `multiply_output_audio()`.
    multiplied: Vec<(usize, usize, u32)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct ApplySource {
    v: VolumeSource,
    audio_ts: u64,
    audio_mixers: u32,
    actions: usize,
    action_ts: u64,
    rec: ApplyRecord,
}

/// `apply_audio_volume()` of `obs-source.c`, on the stand-in: the queue is
/// the first action's timestamp, `conv_frames_to_time()` is the duration
/// passed in, and `apply_audio_actions()`, the memset and
/// `multiply_output_audio()` are recorded.
fn apply_audio_volume(source: &mut ApplySource, mixers: u32, channels: usize, duration: u64) {
    let actions_pending = source.actions > 0;

    if actions_pending && action_due(source.action_ts, source.audio_ts, duration) {
        source.rec.actions = true;
        return;
    }

    let audio_ts = source.audio_ts;
    let vol = get_source_volume(&mut source.v, audio_ts);

    match step(vol, mixers) {
        Step::Skip => return,
        Step::Clear => {
            source.rec.cleared = true;
            source.rec.clear_size = FRAMES * size_of::<f32>() * CHANNELS * MIXES;
            return;
        }
        Step::Multiply => {}
    }

    for mix in 0..MIXES {
        if mixes(source.audio_mixers, mixers, mix) {
            source.rec.multiplied.push((mix, channels, vol.to_bits()));
        }
    }
}

fn open_source(volume: f32, audio_mixers: u32) -> ApplySource {
    ApplySource {
        v: VolumeSource {
            enabled: true,
            volume,
            ..VolumeSource::default()
        },
        audio_ts: T,
        audio_mixers,
        ..ApplySource::default()
    }
}

#[test]
fn steps_apply_due_action_first() {
    let mut s = open_source(0.5, 0x3f);
    s.actions = 1;
    s.action_ts = T + TICK - 1;
    apply_audio_volume(&mut s, 0x3f, 2, TICK);
    assert!(s.rec.actions);
    assert!(s.rec.multiplied.is_empty());
    assert!(!s.rec.cleared);

    let mut s = open_source(0.5, 0x3f);
    s.actions = 1;
    s.action_ts = T + TICK;
    apply_audio_volume(&mut s, 0x3f, 2, TICK);
    assert!(!s.rec.actions);
    assert_eq!(s.rec.multiplied.len(), 6);

    // the action's timestamp only counts when one is queued
    let mut s = open_source(0.5, 0x3f);
    s.action_ts = 0;
    apply_audio_volume(&mut s, 0x3f, 2, TICK);
    assert!(!s.rec.actions);
    assert_eq!(s.rec.multiplied.len(), 6);

    // a muted source with a due action: the actions run, nothing else
    let mut s = open_source(0.5, 0x3f);
    s.v.muted = true;
    s.actions = 2;
    s.action_ts = 0;
    apply_audio_volume(&mut s, 0, 2, TICK);
    assert!(s.rec.actions);
    assert!(!s.rec.cleared);
}

#[test]
fn steps_apply_each_step_and_mask() {
    for audio_mixers in MASK_EDGES {
        for mixers in MASK_EDGES {
            // 1, and a volume that snaps to it
            for one in [1.0, ONE_LOW_IN, ONE_HIGH_IN] {
                let mut s = open_source(one, audio_mixers);
                apply_audio_volume(&mut s, mixers, 2, TICK);
                assert!(!s.rec.cleared, "1: cleared");
                assert!(s.rec.multiplied.is_empty(), "1: multiplied");
            }

            // 0, -0, a volume that snaps to 0, and muted
            for (i, zero) in [0.0, -0.0, 0.0001, 0.5].into_iter().enumerate() {
                let mut s = open_source(zero, audio_mixers);
                s.v.muted = i == 3;
                apply_audio_volume(&mut s, mixers, 2, TICK);
                assert!(s.rec.cleared, "0: cleared");
                assert_eq!(
                    s.rec.clear_size,
                    FRAMES * size_of::<f32>() * CHANNELS * MIXES
                );
                assert!(s.rec.multiplied.is_empty(), "0: multiplied");
            }

            // anything else
            for other in [
                0.5,
                ONE_LOW_OUT,
                20.0,
                bitsf(0x7fc0_0000),
                f32::INFINITY,
                -1.0,
            ] {
                let mut s = open_source(other, audio_mixers);
                apply_audio_volume(&mut s, mixers, 3, TICK);
                assert_eq!(s.rec.cleared, mixers == 0, "other: cleared");
                let expected: Vec<(usize, usize, u32)> = (0..MIXES)
                    .filter(|&mix| {
                        let bit = 1_u32 << mix;
                        mixers != 0 && audio_mixers & bit != 0 && mixers & bit != 0
                    })
                    .map(|mix| (mix, 3, other.to_bits()))
                    .collect();
                assert_eq!(s.rec.multiplied, expected, "other: multiplied");
            }
        }
    }
}

#[test]
fn steps_apply_mixers_past_the_mixes() {
    for high in [0x40, 0x80, 0xc0, 0x8000_0000, 0xffff_ffc0] {
        let mut s = open_source(0.5, u32::MAX);
        apply_audio_volume(&mut s, high, 2, TICK);
        assert!(!s.rec.cleared, "cleared");
        assert!(s.rec.multiplied.is_empty(), "multiplied");
    }

    // a source in no mix: nothing multiplied, nothing cleared
    let mut s = open_source(0.5, 0);
    apply_audio_volume(&mut s, 0x3f, 2, TICK);
    assert!(!s.rec.cleared);
    assert!(s.rec.multiplied.is_empty());

    // every mix: all six, in order
    let mut s = open_source(0.25, u32::MAX);
    apply_audio_volume(&mut s, u32::MAX, 8, TICK);
    let expected: Vec<(usize, usize, u32)> =
        (0..MIXES).map(|mix| (mix, 8, 0.25_f32.to_bits())).collect();
    assert_eq!(s.rec.multiplied, expected);
}

// ---------------------------------------------------------------------------
// edge cases

/// The snap written another way: the distances in `f64`, where they are
/// exact. Within [0.5, 2] `v - 1.0` is exact in `f32` too (Sterbenz), and
/// outside it both distances are at least 0.5, so the rule is the same.
fn snap_rule(muted: bool, volume: f32) -> f32 {
    let eps = f64::from(SNAP_EPSILON);
    let v = f64::from(volume);
    if muted || v.abs() <= eps {
        0.0
    } else if (v - 1.0).abs() <= eps {
        1.0
    } else {
        volume
    }
}

#[track_caller]
fn check_snap_rule(volume: f32) {
    for muted in [false, true] {
        check_float(
            snap(muted, volume),
            snap_rule(muted, volume),
            &format!("snap({muted}, {:#010x})", volume.to_bits()),
        );
    }
}

#[test]
fn edge_snap_epsilon_is_the_c_literal() {
    // 0.0001f: the nearest float to 1e-4
    assert_eq!(SNAP_EPSILON.to_bits(), 0x38d1_b717);
}

#[test]
fn edge_snap_every_float_near_the_edges() {
    // every float within 2^14 ulps of +-0.0001, of 1 - 0.0001 and 1 + 0.0001,
    // and of 0 (the subnormals and the smallest normals)
    let centres = [
        SNAP_EPSILON,
        -SNAP_EPSILON,
        ONE_LOW_IN,
        ONE_HIGH_IN,
        bitsf(0),
        bitsf(0x8000_0000),
    ];
    let mut checked = 0_u32;
    for centre in centres {
        let bits = centre.to_bits();
        for d in 0..=(1_u32 << 14) {
            check_snap_rule(bitsf(bits.wrapping_add(d)));
            check_snap_rule(bitsf(bits.wrapping_sub(d)));
            checked += 2;
        }
    }
    assert_eq!(checked, 6 * 2 * ((1 << 14) + 1));
}

#[test]
fn edge_snap_stride_through_every_bit_pattern() {
    // 1,046,788 patterns spread over all 2^32, every exponent and both signs
    let mut bits = 0_u32;
    loop {
        check_snap_rule(bitsf(bits));
        match bits.checked_add(4096 + 7) {
            Some(next) => bits = next,
            None => break,
        }
    }
}

/// The step written another way: `total_cmp` cannot see -0 as 0, so the
/// compares go through the classes.
fn step_rule(vol: f32, mixers: u32) -> Step {
    if !vol.is_nan() && vol.to_bits() == 1.0_f32.to_bits() {
        Step::Skip
    } else if (!vol.is_nan() && vol.abs().to_bits() == 0) || mixers == 0 {
        Step::Clear
    } else {
        Step::Multiply
    }
}

/// The due compare in `u128`: the sum wraps at 2^64.
fn due_rule(action_ts: u64, audio_ts: u64, duration: u64) -> bool {
    let sum = u128::from(audio_ts) + u128::from(duration);
    let end = sum % (1_u128 << 64);
    u128::from(action_ts) < end
}

#[test]
fn edge_mixes_every_index() {
    for a in MASK_EDGES {
        for b in MASK_EDGES {
            for mix in 0..32 {
                let bit = 1_u32 << mix;
                assert_eq!(mixes(a, b, mix), a & bit != 0 && b & bit != 0);
            }
            // intentional difference: past the u32's width no mix is
            // chosen (the C shifts an int past its width, undefined;
            // libobs passes mix < MAX_AUDIO_MIXES)
            for mix in [32, 33, 63, 64, usize::MAX] {
                assert!(!mixes(a, b, mix), "mixes({a:#x}, {b:#x}, {mix})");
            }
        }
    }
}

#[test]
fn edge_due_special_values() {
    let centres = [0, 1, TICK, T, u64::MAX / 2, u64::MAX - TICK, u64::MAX];
    let mut values = Vec::new();
    for c in centres {
        for d in 0..=4_u64 {
            values.push(c.wrapping_add(d).wrapping_sub(2));
        }
    }
    for &a in &values {
        for &t in &values {
            for &d in &values {
                assert_eq!(action_due(a, t, d), due_rule(a, t, d), "{a} {t} {d}");
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 20_000, ..ProptestConfig::default() })]

    #[test]
    fn edge_snap_any_bit_pattern(bits in any::<u32>(), muted in any::<bool>()) {
        let volume = bitsf(bits);
        prop_assert_eq!(snap(muted, volume).to_bits(), snap_rule(muted, volume).to_bits());
    }

    #[test]
    fn edge_step_any_bit_pattern(
        bits in prop_oneof![
            any::<u32>(),
            Just(0_u32),
            Just(0x8000_0000_u32),
            Just(1.0_f32.to_bits()),
            Just(0x3f7f_ffff_u32),
            Just(0x3f80_0001_u32),
            Just(0x7fc0_0000_u32),
        ],
        mixers in prop_oneof![any::<u32>(), Just(0_u32), Just(0x40_u32), Just(u32::MAX)],
    ) {
        let vol = bitsf(bits);
        prop_assert_eq!(step(vol, mixers), step_rule(vol, mixers));
    }

    #[test]
    fn edge_due_full_range(
        action_ts in any::<u64>(),
        audio_ts in any::<u64>(),
        duration in prop_oneof![any::<u64>(), 0..=100_000_000_u64],
    ) {
        prop_assert_eq!(action_due(action_ts, audio_ts, duration), due_rule(action_ts, audio_ts, duration));
    }

    #[test]
    fn edge_due_near_the_end_of_the_tick(
        audio_ts in any::<u64>(),
        duration in 0..=100_000_000_u64,
        delta in -3_i64..=3,
    ) {
        let action_ts = audio_ts.wrapping_add(duration).wrapping_add_signed(delta);
        prop_assert_eq!(action_due(action_ts, audio_ts, duration), due_rule(action_ts, audio_ts, duration));
    }

    #[test]
    fn edge_mixes_any_masks(a in any::<u32>(), b in any::<u32>(), mix in 0..MIXES) {
        let bit = 1_u32 << mix;
        prop_assert_eq!(mixes(a, b, mix), a & bit != 0 && b & bit != 0);
    }

    #[test]
    fn edge_get_source_volume_any_source(
        bits in 0_u8..64,
        stops in (any::<u64>(), any::<u64>()),
        delays in (any::<u64>(), any::<u64>()),
        volume_bits in any::<u32>(),
        os_time in any::<u64>(),
    ) {
        let mut s = source_of(bits, stops, bitsf(volume_bits));
        s.push_to_mute_delay = delays.0;
        s.push_to_talk_delay = delays.1;
        let before = s;
        let vol = get_source_volume(&mut s, os_time);

        // the gate written out, as in source_muted()'s Tier 1 rule
        let wide = |delay: u64| {
            u64::try_from(u128::from(os_time) + u128::from(delay) * u128::from(MS)).unwrap_or(u64::MAX)
        };
        let ptm_stop = if before.push_to_mute_enabled && before.push_to_mute_pressed {
            wide(before.push_to_mute_delay)
        } else {
            before.push_to_mute_stop_time
        };
        let ptt_stop = if before.push_to_talk_enabled && before.push_to_talk_pressed {
            wide(before.push_to_talk_delay)
        } else {
            before.push_to_talk_stop_time
        };
        let muted = !before.enabled
            || before.muted
            || (before.push_to_mute_enabled && (before.push_to_mute_pressed || os_time < ptm_stop))
            || (before.push_to_talk_enabled && !(before.push_to_talk_pressed || os_time < ptt_stop));
        prop_assert_eq!(vol.to_bits(), snap_rule(muted, before.volume).to_bits());
        prop_assert_eq!(s.push_to_mute_stop_time, ptm_stop);
        prop_assert_eq!(s.push_to_talk_stop_time, ptt_stop);
    }
}
