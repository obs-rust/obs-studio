//! Tier 3: the `obs_source_volume_*` shims and the safe core against the
//! original C (`obs-c-oracle`, `oracle/source_volume.c`: the unmodified
//! `libobs/obs-source-volume.c.h` with its functions renamed).
//!
//! The four functions take and return plain values, so a run compares the
//! snapped volume by its bits (NaN payloads, -0 and the infinities count),
//! the step as the `u32` the C returns, and each `bool` as the byte the C
//! returns (it must be 0 or 1, and the same on both sides). Each input is
//! given to the oracle, to the shim (the `extern "C"` function the hybrid
//! links) and to the core.
//!
//! Runs:
//! - the enum's values and size against the C header;
//! - the snap and the step over 115 special floats (the snap edges and
//!   their float neighbours, -0, NaN payloads, +-inf, subnormals), muted
//!   and not, with 14 mixer masks; every float within 2^16 ulps of each
//!   snap edge; and proptests over any `f32` bit pattern and any mixers
//!   (an ignored test runs all 2^32 patterns in a release build);
//! - the mix choice exhaustively over every pair of the low seven mask bits
//!   (2^14 pairs), with no high bits and with all of them, and every mix
//!   index of 0 to 30; then any masks;
//! - the due compare over every triple of 35 special `u64` values and
//!   full-range proptests, near the tick's end and the wrap included;
//! - `get_source_volume()` of `obs-source.c`, copied onto a stand-in source
//!   and run once over the oracle's functions (the push-to-mute gate's
//!   oracle too) and once over the shims: every combination of its six
//!   flags (64), five stored stop times per gate around the clock and
//!   saturated, three pairs of delays, three clocks and 16 volumes (230,400
//!   runs), and a proptest over full-range delays, stop times, clocks and
//!   volume bits; the result's bits and both stored stop times must match;
//! - `apply_audio_volume()` copied the same way, with
//!   `apply_audio_actions()`, the memset and `multiply_output_audio()`
//!   recorded: every flag combination with mask edges, volumes, queued
//!   actions due and not due and the wrap, and a proptest; the recorded
//!   calls must match.
//!
//! Intentional difference (tested in Tier 1 only): `mixes()` for a mix of 31
//! or more, where the C's `int` shift is undefined.

use obs_c_oracle::source_ptm_gate as ptm_oracle;
use obs_c_oracle::source_volume as oracle;
use obs_source::ffi::source_ptm_gate as ptm_shim;
use obs_source::ffi::source_volume as shim;
use obs_source::volume_apply::{self as core, Step};
use proptest::prelude::*;

const MS: u64 = 1_000_000;
const SEC: u64 = 1000 * MS;
const T: u64 = 1000 * SEC;
const TICK: u64 = 21_333_333;
const MIXES: usize = 6;

#[track_caller]
fn c_bool(byte: u8, what: &str) -> bool {
    assert!(byte <= 1, "{what}: the C returned the byte {byte}");
    byte == 1
}

fn bitsf(bits: u32) -> f32 {
    f32::from_bits(bits)
}

fn oracle_snap(muted: bool, volume: f32) -> f32 {
    // SAFETY: by-value floats and bools in and out; the C touches no memory.
    unsafe { oracle::oracle_obs_source_volume_snap(muted, volume) }
}

#[track_caller]
fn oracle_due(action_ts: u64, audio_ts: u64, duration: u64) -> bool {
    // SAFETY: as above.
    c_bool(
        unsafe { oracle::oracle_obs_source_volume_action_due(action_ts, audio_ts, duration) },
        "action_due",
    )
}

fn oracle_step(vol: f32, mixers: u32) -> u32 {
    // SAFETY: as above.
    unsafe { oracle::oracle_obs_source_volume_step(vol, mixers) }
}

#[track_caller]
fn oracle_mixes(audio_mixers: u32, mixers: u32, mix: usize) -> bool {
    // SAFETY: as above; mix < 31, where the C's shift is defined.
    c_bool(
        unsafe { oracle::oracle_obs_source_volume_mixes(audio_mixers, mixers, mix) },
        "mixes",
    )
}

#[track_caller]
fn check_snap(muted: bool, volume: f32) {
    let c = oracle_snap(muted, volume).to_bits();
    let what = format!("snap({muted}, {:#010x})", volume.to_bits());
    assert_eq!(
        shim::obs_source_volume_snap(muted, volume).to_bits(),
        c,
        "shim {what}"
    );
    assert_eq!(core::snap(muted, volume).to_bits(), c, "core {what}");
}

#[track_caller]
fn check_step(vol: f32, mixers: u32) {
    let c = oracle_step(vol, mixers);
    let what = format!("step({:#010x}, {mixers:#x})", vol.to_bits());
    assert!(c <= 2, "{what}: the C returned {c}");
    assert_eq!(shim::obs_source_volume_step(vol, mixers), c, "shim {what}");
    assert_eq!(u32::from(core::step(vol, mixers)), c, "core {what}");
}

#[track_caller]
fn check_due(action_ts: u64, audio_ts: u64, duration: u64) {
    let c = oracle_due(action_ts, audio_ts, duration);
    let what = format!("action_due({action_ts}, {audio_ts}, {duration})");
    assert_eq!(
        shim::obs_source_volume_action_due(action_ts, audio_ts, duration),
        c,
        "shim {what}"
    );
    assert_eq!(
        core::action_due(action_ts, audio_ts, duration),
        c,
        "core {what}"
    );
}

#[track_caller]
fn check_mixes(audio_mixers: u32, mixers: u32, mix: usize) {
    let c = oracle_mixes(audio_mixers, mixers, mix);
    let what = format!("mixes({audio_mixers:#x}, {mixers:#x}, {mix})");
    assert_eq!(
        shim::obs_source_volume_mixes(audio_mixers, mixers, mix),
        c,
        "shim {what}"
    );
    assert_eq!(core::mixes(audio_mixers, mixers, mix), c, "core {what}");
}

/// The snap edges and the floats that matter to the step, each with its
/// four float neighbours on each side, and the specials.
fn special_floats() -> Vec<f32> {
    let centres: [f32; 12] = [
        0.0,
        -0.0,
        core::SNAP_EPSILON,
        -core::SNAP_EPSILON,
        1.0,
        1.0 - 1677.0 / 16_777_216.0,
        1.0 + 838.0 / 8_388_608.0,
        -1.0,
        0.5,
        20.0,
        f32::MAX,
        f32::MIN_POSITIVE,
    ];
    let mut bits = Vec::new();
    for c in centres {
        let b = c.to_bits();
        for d in 0..=4_u32 {
            bits.push(b.wrapping_add(d));
            bits.push(b.wrapping_sub(d));
        }
    }
    bits.extend([
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
        0x7fc0_0000,
        0xffc0_0000,
        0x7fc0_0001,
        0x7f80_0001,
        0xff80_0001,
        0x7fff_ffff,
        0xffff_ffff,
        0x7fa0_0000,
        1,
        0x8000_0001,
        0x007f_ffff,
        0x807f_ffff,
        0.9999_f32.to_bits(),
        1.0001_f32.to_bits(),
    ]);
    bits.sort_unstable();
    bits.dedup();
    bits.into_iter().map(bitsf).collect()
}

const MASKS: [u32; 14] = [
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
    0x7fff_ffff,
    0x8000_0000,
    0xffff_ffc0,
    u32::MAX,
];

#[test]
fn step_enum_matches_the_c_header() {
    for (which, value) in [
        (0, shim::OBS_SOURCE_VOLUME_SKIP),
        (1, shim::OBS_SOURCE_VOLUME_CLEAR),
        (2, shim::OBS_SOURCE_VOLUME_MULTIPLY),
    ] {
        // SAFETY: returns a constant.
        assert_eq!(
            unsafe { oracle::oracle_source_volume_step_value(which) },
            value
        );
    }
    assert_eq!(u32::from(Step::Skip), shim::OBS_SOURCE_VOLUME_SKIP);
    assert_eq!(u32::from(Step::Clear), shim::OBS_SOURCE_VOLUME_CLEAR);
    assert_eq!(u32::from(Step::Multiply), shim::OBS_SOURCE_VOLUME_MULTIPLY);
    // SAFETY: returns a constant.
    assert_eq!(unsafe { oracle::oracle_source_volume_step_size() }, 4);
}

#[test]
fn snap_and_step_special_floats() {
    let floats = special_floats();
    assert_eq!(floats.len(), 115);
    for &f in &floats {
        check_snap(false, f);
        check_snap(true, f);
        for mixers in MASKS {
            check_step(f, mixers);
        }
    }
}

#[test]
fn snap_every_float_near_the_edges() {
    let centres = [
        core::SNAP_EPSILON,
        -core::SNAP_EPSILON,
        1.0 - 1677.0 / 16_777_216.0,
        1.0 + 838.0 / 8_388_608.0,
        0.0,
        -0.0,
    ];
    for c in centres {
        let b = c.to_bits();
        for d in 0..=(1_u32 << 16) {
            check_snap(false, bitsf(b.wrapping_add(d)));
            check_snap(false, bitsf(b.wrapping_sub(d)));
        }
    }
}

/// All 2^32 bit patterns through the snap (muted and not) and the step
/// (with and without mixers): run with
/// `soldr cargo test --release -p obs-data --test obs-tests-core
/// source::volume_apply_parity::every_bit_pattern -- --ignored`.
#[test]
#[ignore = "4 x 2^32 oracle calls: run in a release build"]
fn every_bit_pattern() {
    for bits in 0..=u32::MAX {
        let f = bitsf(bits);
        let c = oracle_snap(false, f).to_bits();
        assert_eq!(core::snap(false, f).to_bits(), c, "{bits:#010x}");
        assert_eq!(
            shim::obs_source_volume_snap(false, f).to_bits(),
            c,
            "{bits:#010x}"
        );
        assert_eq!(
            core::snap(true, f).to_bits(),
            oracle_snap(true, f).to_bits()
        );
        assert_eq!(
            u32::from(core::step(f, 1)),
            oracle_step(f, 1),
            "{bits:#010x}"
        );
        assert_eq!(
            u32::from(core::step(f, 0)),
            oracle_step(f, 0),
            "{bits:#010x}"
        );
    }
}

#[test]
fn mixes_every_low_mask_pair() {
    let mut runs = 0_u32;
    for a in 0..128_u32 {
        for b in 0..128_u32 {
            for high in [0, 0xffff_ff80] {
                for mix in 0..=30 {
                    check_mixes(a | high, b | high, mix);
                    runs += 1;
                }
            }
        }
    }
    assert_eq!(runs, 128 * 128 * 2 * 31);
}

#[test]
fn due_every_special_triple() {
    let centres = [0, 1, TICK, T, u64::MAX / 2, u64::MAX - TICK, u64::MAX];
    let mut values = Vec::new();
    for c in centres {
        for d in 0..=4_u64 {
            values.push(c.wrapping_add(d).wrapping_sub(2));
        }
    }
    assert_eq!(values.len(), 35);
    for &a in &values {
        for &t in &values {
            for &d in &values {
                check_due(a, t, d);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// the callers, over one side's functions

/// The functions `get_source_volume()` and `apply_audio_volume()` call,
/// from one side.
struct Side {
    stop_time: fn(u64, u64) -> u64,
    held: fn(u64, u64) -> bool,
    mutes: fn(bool, bool) -> bool,
    talk_mutes: fn(bool, bool) -> bool,
    snap: fn(bool, f32) -> f32,
    due: fn(u64, u64, u64) -> bool,
    step: fn(f32, u32) -> u32,
    mixes: fn(u32, u32, usize) -> bool,
}

const ORACLE: Side = Side {
    // SAFETY: by-value integers in and out; the C touches no memory.
    stop_time: |t, d| unsafe { ptm_oracle::oracle_obs_source_ptm_gate_stop_time(t, d) },
    // SAFETY: as above.
    held: |t, s| {
        c_bool(
            unsafe { ptm_oracle::oracle_obs_source_ptm_gate_held(t, s) },
            "held",
        )
    },
    // SAFETY: as above.
    mutes: |e, a| {
        c_bool(
            unsafe { ptm_oracle::oracle_obs_source_ptm_gate_mutes(e, a) },
            "mutes",
        )
    },
    talk_mutes: |e, a| {
        // SAFETY: as above.
        c_bool(
            unsafe { ptm_oracle::oracle_obs_source_ptm_gate_talk_mutes(e, a) },
            "talk_mutes",
        )
    },
    snap: oracle_snap,
    due: oracle_due,
    step: oracle_step,
    mixes: oracle_mixes,
};

const SHIM: Side = Side {
    stop_time: |t, d| ptm_shim::obs_source_ptm_gate_stop_time(t, d),
    held: |t, s| ptm_shim::obs_source_ptm_gate_held(t, s),
    mutes: |e, a| ptm_shim::obs_source_ptm_gate_mutes(e, a),
    talk_mutes: |e, a| ptm_shim::obs_source_ptm_gate_talk_mutes(e, a),
    snap: |m, v| shim::obs_source_volume_snap(m, v),
    due: |a, t, d| shim::obs_source_volume_action_due(a, t, d),
    step: |v, m| shim::obs_source_volume_step(v, m),
    mixes: |a, m, x| shim::obs_source_volume_mixes(a, m, x),
};

/// The fields of `struct obs_source` the two functions read and write.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Source {
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
    volume_bits: u32,
    audio_ts: u64,
    audio_mixers: u32,
    actions: usize,
    action_ts: u64,
}

/// `get_source_volume()` of `obs-source.c`, on the stand-in, over one
/// side's functions.
fn get_source_volume(side: &Side, source: &mut Source, os_time: u64) -> f32 {
    if source.push_to_mute_enabled && source.push_to_mute_pressed {
        source.push_to_mute_stop_time = (side.stop_time)(os_time, source.push_to_mute_delay);
    }

    if source.push_to_talk_enabled && source.push_to_talk_pressed {
        source.push_to_talk_stop_time = (side.stop_time)(os_time, source.push_to_talk_delay);
    }

    let push_to_mute_active =
        source.push_to_mute_pressed || (side.held)(os_time, source.push_to_mute_stop_time);
    let push_to_talk_active =
        source.push_to_talk_pressed || (side.held)(os_time, source.push_to_talk_stop_time);

    let muted = !source.enabled
        || source.muted
        || (side.mutes)(source.push_to_mute_enabled, push_to_mute_active)
        || (side.talk_mutes)(source.push_to_talk_enabled, push_to_talk_active);

    (side.snap)(muted, bitsf(source.volume_bits))
}

/// What a run of `apply_audio_volume()` called.
#[derive(Debug, Clone, PartialEq)]
enum Call {
    Actions(usize),
    Memset(usize),
    Multiply(usize, usize, u32),
}

/// `apply_audio_volume()` of `obs-source.c`, on the stand-in, over one
/// side's functions; `conv_frames_to_time()` is `duration`.
fn apply_audio_volume(
    side: &Side,
    source: &mut Source,
    mixers: u32,
    channels: usize,
    duration: u64,
) -> Vec<Call> {
    let mut calls = Vec::new();

    if source.actions > 0 && (side.due)(source.action_ts, source.audio_ts, duration) {
        calls.push(Call::Actions(channels));
        return calls;
    }

    let audio_ts = source.audio_ts;
    let vol = get_source_volume(side, source, audio_ts);

    match (side.step)(vol, mixers) {
        0 => return calls,
        1 => {
            calls.push(Call::Memset(1024 * 4 * 8 * MIXES));
            return calls;
        }
        _ => {}
    }

    for mix in 0..MIXES {
        if (side.mixes)(source.audio_mixers, mixers, mix) {
            calls.push(Call::Multiply(mix, channels, vol.to_bits()));
        }
    }
    calls
}

fn source_of(bits: u8, stops: (u64, u64), delays: (u64, u64), volume: f32) -> Source {
    Source {
        enabled: bits & 1 != 0,
        muted: bits & 2 != 0,
        push_to_mute_enabled: bits & 4 != 0,
        push_to_mute_pressed: bits & 8 != 0,
        push_to_talk_enabled: bits & 16 != 0,
        push_to_talk_pressed: bits & 32 != 0,
        push_to_mute_delay: delays.0,
        push_to_talk_delay: delays.1,
        push_to_mute_stop_time: stops.0,
        push_to_talk_stop_time: stops.1,
        volume_bits: volume.to_bits(),
        ..Source::default()
    }
}

#[track_caller]
fn check_get_source_volume(source: Source, os_time: u64) {
    let mut c = source;
    let mut rust = source;
    let c_vol = get_source_volume(&ORACLE, &mut c, os_time).to_bits();
    let rust_vol = get_source_volume(&SHIM, &mut rust, os_time).to_bits();
    assert_eq!(
        (rust_vol, rust),
        (c_vol, c),
        "get_source_volume({source:?}, {os_time})"
    );
}

#[track_caller]
fn check_apply(source: Source, mixers: u32, channels: usize, duration: u64) {
    let mut c = source;
    let mut rust = source;
    let c_calls = apply_audio_volume(&ORACLE, &mut c, mixers, channels, duration);
    let rust_calls = apply_audio_volume(&SHIM, &mut rust, mixers, channels, duration);
    assert_eq!(
        (rust_calls, rust),
        (c_calls, c),
        "apply_audio_volume({source:?}, {mixers:#x}, {channels}, {duration})"
    );
}

const VOLUMES: [f32; 16] = [
    0.0,
    -0.0,
    0.0001,
    0.000_100_000_01,
    0.5,
    1.0,
    0.9999,
    1.0001,
    0.99995,
    20.0,
    -1.0,
    f32::INFINITY,
    f32::NEG_INFINITY,
    f32::NAN,
    f32::MIN_POSITIVE,
    f32::MAX,
];

#[test]
fn get_source_volume_every_flag() {
    let mut runs = 0_u32;
    for os_time in [T, 551_616, u64::MAX - 1] {
        let stops = [
            0,
            os_time.wrapping_sub(1),
            os_time,
            os_time.wrapping_add(1),
            u64::MAX,
        ];
        for bits in 0_u8..64 {
            for &ptm in &stops {
                for &ptt in &stops {
                    for delays in [(0, 0), (300, 500), (18_446_744_073_710, u64::MAX)] {
                        for volume in VOLUMES {
                            check_get_source_volume(
                                source_of(bits, (ptm, ptt), delays, volume),
                                os_time,
                            );
                            runs += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(runs, 3 * 64 * 25 * 3 * 16);
}

#[test]
fn apply_audio_volume_every_flag_and_mask() {
    let mut runs = 0_u32;
    for bits in 0_u8..64 {
        for volume in VOLUMES {
            for audio_mixers in MASKS {
                for mixers in MASKS {
                    // no action, one not due, one due, and the wrapped end
                    for (actions, action_ts, audio_ts) in [
                        (0, 0, T),
                        (1, T + TICK, T),
                        (2, T + TICK - 1, T),
                        (1, u64::MAX - 5, u64::MAX - 10),
                    ] {
                        let mut s = source_of(bits, (T, T + 1), (300, 500), volume);
                        s.audio_mixers = audio_mixers;
                        s.actions = actions;
                        s.action_ts = action_ts;
                        s.audio_ts = audio_ts;
                        check_apply(s, mixers, 2, TICK);
                        runs += 1;
                    }
                }
            }
        }
    }
    assert_eq!(runs, 64 * 16 * 14 * 14 * 4);
}

fn any_mask() -> impl Strategy<Value = u32> {
    prop_oneof![any::<u32>(), prop::sample::select(MASKS.to_vec())]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn snap_any_bit_pattern(bits in any::<u32>(), muted in any::<bool>()) {
        check_snap(muted, bitsf(bits));
    }

    #[test]
    fn step_any_bit_pattern(bits in any::<u32>(), mixers in any_mask()) {
        check_step(bitsf(bits), mixers);
    }

    #[test]
    fn mixes_any_masks(a in any_mask(), b in any_mask(), mix in 0_usize..=30) {
        check_mixes(a, b, mix);
    }

    #[test]
    fn due_full_range(action_ts in any::<u64>(), audio_ts in any::<u64>(), duration in any::<u64>()) {
        check_due(action_ts, audio_ts, duration);
    }

    #[test]
    fn due_near_the_end_of_the_tick(
        audio_ts in any::<u64>(),
        duration in 0..=100_000_000_u64,
        delta in -3_i64..=3,
    ) {
        check_due(audio_ts.wrapping_add(duration).wrapping_add_signed(delta), audio_ts, duration);
    }

    #[test]
    fn get_source_volume_full_range(
        bits in 0_u8..64,
        stops in (any::<u64>(), any::<u64>()),
        delays in (any::<u64>(), any::<u64>()),
        volume_bits in any::<u32>(),
        os_time in any::<u64>(),
    ) {
        check_get_source_volume(source_of(bits, stops, delays, bitsf(volume_bits)), os_time);
    }

    #[test]
    fn apply_audio_volume_full_range(
        bits in 0_u8..64,
        stops in (any::<u64>(), any::<u64>()),
        volume_bits in any::<u32>(),
        audio_mixers in any_mask(),
        mixers in any_mask(),
        actions in 0_usize..3,
        times in (any::<u64>(), any::<u64>()),
        duration in prop_oneof![any::<u64>(), Just(TICK)],
        channels in 1_usize..=8,
    ) {
        let mut s = source_of(bits, stops, (300, 500), bitsf(volume_bits));
        s.audio_mixers = audio_mixers;
        s.actions = actions;
        s.action_ts = times.0;
        s.audio_ts = times.1;
        check_apply(s, mixers, channels, duration);
    }
}
