//! Tier 3: the `obs_source_audio_input_*` shims against the original C
//! (`obs-c-oracle`, `oracle/source_audio_input.c`: the unmodified
//! `libobs/obs-source-audio-input.c.h` with its functions renamed).
//!
//! The decisions return `bool`, compared as such. The two loops write the
//! planes they are given, so a run fills two copies of the same planes (and
//! guard samples past the frames, and planes past the ones the C reads),
//! runs the C on one and the shim on the other through `float **`, and
//! compares every sample's bits, NaN payloads included, with one exception:
//! where a multiply or an add of the C had two NaN operands, which of the
//! two comes out depends on the operand order the compiler picks (x86
//! returns the first operand's NaN), and C builds differ there as well (the
//! same C at -O0 and -O2 gives different NaNs for the same two). There both
//! sides must give a NaN. A single NaN operand, a NaN made from finite or
//! infinite values, and every other value must match bit for bit.
//!
//! Runs:
//! - the format's layout and the constants against the C header;
//! - the gates exhaustively over their flags: the discontinuity over both
//!   resampler states and every pair of 14 special timestamps (around 0,
//!   the threshold, 2^63 and `u64::MAX`); the reset condition over every
//!   combination of the three bools and of each format field equal or not
//!   (with values outside the enums); the resampler rule likewise; the
//!   balance gate over both output kinds, eleven speaker values and 44
//!   special balances; the mono gate over both output kinds and every
//!   single bit, none and all;
//! - proptests over full-range `u64` timestamps, any formats, any flags and
//!   balances drawn from bit patterns (finite, ±0, ±inf, NaN);
//! - the laws, every law and five unknown ones, over random samples drawn
//!   from bit patterns, 0 to 64 frames and balances drawn from bit
//!   patterns; and the downmix over 0 to 8 channels (0 reads plane 0
//!   alone), the same samples and 0 to 64 frames. No sample or balance
//!   comes from a wide `Range<f32>` (#1951): the bits are drawn;
//! - the sine law's gains (a plane of ones) over every 4099th `u32` bit
//!   pattern and every 257th one in [0, 1] (about 5.2 million balances);
//!   `exhaustive_sine_gains` (ignored: run it with `--ignored --release`)
//!   covers all 2^32;
//! - zero frames and unknown laws read nothing on either side, with a null
//!   `data`.
//!
//! Intentional difference, checked on the shim only: a null `data` or a
//! null plane does nothing (the C dereferences it).

use std::ffi::c_int;
use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::source_audio_input::{self as oracle, OracleAudioInputFormat};
use obs_source::audio_input::{
    FORCE_MONO, LINEAR, SINE_LAW, SMOOTHING_THRESHOLD, SPEAKERS_STEREO, SQUARE_LAW,
};
use obs_source::ffi::source_audio_input::{self as shim, obs_source_audio_input_format};
use proptest::prelude::*;

#[test]
fn layout_matches_the_c_header() {
    type F = obs_source_audio_input_format;
    let rust = [
        size_of::<F>(),
        align_of::<F>(),
        offset_of!(F, format),
        offset_of!(F, samples_per_sec),
        offset_of!(F, speakers),
        usize::try_from(SMOOTHING_THRESHOLD).unwrap(),
        usize::try_from(SPEAKERS_STEREO).unwrap(),
        usize::try_from(FORCE_MONO).unwrap(),
        usize::try_from(SINE_LAW).unwrap(),
        usize::try_from(SQUARE_LAW).unwrap(),
        usize::try_from(LINEAR).unwrap(),
        size_of::<c_int>(),
        size_of::<bool>(),
    ];
    for (which, (name, value)) in oracle::LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: a pure lookup.
        let c = unsafe { oracle::oracle_source_audio_input_layout(which) };
        assert_eq!(value, c, "{name}");
    }
    assert_eq!(size_of::<OracleAudioInputFormat>(), size_of::<F>());
    assert_eq!(align_of::<OracleAudioInputFormat>(), align_of::<F>());
}

// --------------------------------------------------------------------------
// The decisions, one call on each side

#[derive(Debug, Clone, Copy)]
struct Fmt(i32, u32, i32);

fn c_fmt(f: Fmt) -> OracleAudioInputFormat {
    OracleAudioInputFormat {
        format: f.0,
        samples_per_sec: f.1,
        speakers: f.2,
    }
}

fn r_fmt(f: Fmt) -> obs_source_audio_input_format {
    obs_source_audio_input_format {
        format: f.0,
        samples_per_sec: f.1,
        speakers: f.2,
    }
}

#[track_caller]
fn check_discontinuous(has_resampler: bool, next: u64, timestamp: u64) -> bool {
    // SAFETY: plain values.
    let c = unsafe {
        oracle::oracle_obs_source_audio_input_discontinuous(has_resampler, next, timestamp)
    };
    let r = shim::obs_source_audio_input_discontinuous(has_resampler, next, timestamp);
    assert_eq!(r, c, "discontinuous({has_resampler}, {next}, {timestamp})");
    c
}

#[track_caller]
fn check_reset(current: Fmt, packet: Fmt, last: bool, now: bool, disc: bool) -> bool {
    // SAFETY: plain values.
    let c = unsafe {
        oracle::oracle_obs_source_audio_input_reset_needed(
            c_fmt(current),
            c_fmt(packet),
            last,
            now,
            disc,
        )
    };
    let r =
        shim::obs_source_audio_input_reset_needed(r_fmt(current), r_fmt(packet), last, now, disc);
    assert_eq!(
        r, c,
        "reset_needed({current:?}, {packet:?}, {last}, {now}, {disc})"
    );
    c
}

#[track_caller]
fn check_resampler(source: Fmt, output: Fmt, compensation: bool) -> bool {
    // SAFETY: plain values.
    let c = unsafe {
        oracle::oracle_obs_source_audio_input_resampler_needed(
            c_fmt(source),
            c_fmt(output),
            compensation,
        )
    };
    let r =
        shim::obs_source_audio_input_resampler_needed(r_fmt(source), r_fmt(output), compensation);
    assert_eq!(
        r, c,
        "resampler_needed({source:?}, {output:?}, {compensation})"
    );
    c
}

#[track_caller]
fn check_balance_gate(mono_output: bool, speakers: i32, balance: f32) -> bool {
    // SAFETY: plain values.
    let c = unsafe {
        oracle::oracle_obs_source_audio_input_balance_needed(mono_output, speakers, balance)
    };
    let r = shim::obs_source_audio_input_balance_needed(mono_output, speakers, balance);
    assert_eq!(
        r,
        c,
        "balance_needed({mono_output}, {speakers}, {balance:e} = {:#010x})",
        balance.to_bits()
    );
    c
}

#[track_caller]
fn check_mono_gate(mono_output: bool, flags: u32) -> bool {
    // SAFETY: plain values.
    let c = unsafe { oracle::oracle_obs_source_audio_input_mono_needed(mono_output, flags) };
    let r = shim::obs_source_audio_input_mono_needed(mono_output, flags);
    assert_eq!(r, c, "mono_needed({mono_output}, {flags:#x})");
    c
}

const T: u64 = SMOOTHING_THRESHOLD;

const TIMESTAMPS: [u64; 14] = [
    0,
    1,
    T - 1,
    T,
    T + 1,
    2 * T,
    3_600_000_000_000,
    3_600_000_000_000 + T,
    1 << 63,
    (1 << 63) + T,
    u64::MAX - T - 1,
    u64::MAX - T,
    u64::MAX - 1,
    u64::MAX,
];

#[test]
fn discontinuity_over_every_special_pair() {
    let mut counts = [0usize; 2];
    for has_resampler in [false, true] {
        for next in TIMESTAMPS {
            for timestamp in TIMESTAMPS {
                counts[usize::from(check_discontinuous(has_resampler, next, timestamp))] += 1;
            }
        }
    }
    assert!(counts[0] > 50 && counts[1] > 50, "{counts:?}");
}

/// Each field of a format: a base value and other values (outside the
/// enums too).
const FORMATS: [i32; 4] = [8, 2, -1, i32::MAX];
const RATES: [u32; 4] = [48000, 44100, 0, u32::MAX];
const SPEAKERS: [i32; 4] = [2, 6, 0, -1];

#[test]
fn reset_and_resampler_over_every_combination() {
    let mut counts = [0usize; 4];
    for &cf in &FORMATS {
        for &cr in &RATES {
            for &cs in &SPEAKERS {
                let current = Fmt(cf, cr, cs);
                // the packet: each field equal to the current one or one of
                // the others
                for &pf in &FORMATS {
                    for &pr in &RATES {
                        for &ps in &SPEAKERS {
                            let packet = Fmt(pf, pr, ps);
                            for bits in 0u8..8 {
                                let (last, now, disc) =
                                    (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
                                counts
                                    [usize::from(check_reset(current, packet, last, now, disc))] +=
                                    1;
                            }
                            for compensation in [false, true] {
                                counts[2 + usize::from(check_resampler(
                                    current,
                                    packet,
                                    compensation,
                                ))] += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    // [reset not needed, needed, resampler not needed, needed]
    assert!(counts.iter().all(|&n| n >= 64), "{counts:?}");
}

fn special_balances() -> Vec<f32> {
    let mut values = vec![
        0.0f32,
        -0.0,
        0.25,
        0.5,
        0.75,
        1.0,
        -1.0,
        2.0,
        0.49,
        0.51,
        0.495,
        0.1,
        0.9,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        -f32::NAN,
        f32::from_bits(0x7f80_0001),
        f32::from_bits(0xff80_0001),
        f32::from_bits(0x7fc0_0001),
        f32::from_bits(0xffff_ffff),
        f32::from_bits(1),
        f32::from_bits(0x8000_0001),
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::MIN,
        f32::EPSILON,
        1.0 - f32::EPSILON,
        1.0 + f32::EPSILON,
        f32::from_bits(0x3a91_7d6b),
    ];
    for edge in [0.49f32, 0.51] {
        for delta in [1u32, 2, 1000] {
            values.push(f32::from_bits(edge.to_bits() - delta));
            values.push(f32::from_bits(edge.to_bits() + delta));
        }
    }
    values.push(f32::from_bits(0x3f80_0001));
    values.push(f32::from_bits(0x3f7f_ffff));
    values
}

#[test]
fn balance_gate_over_every_special_value() {
    let balances = special_balances();
    assert_eq!(balances.len(), 44);
    let mut counts = [0usize; 2];
    for mono_output in [false, true] {
        for speakers in [0, 1, 2, 3, 4, 5, 6, 7, 8, -1, i32::MIN] {
            for &balance in &balances {
                counts[usize::from(check_balance_gate(mono_output, speakers, balance))] += 1;
            }
        }
    }
    assert!(counts[0] > 100 && counts[1] > 20, "{counts:?}");
}

#[test]
fn mono_gate_over_every_bit() {
    let mut counts = [0usize; 2];
    for mono_output in [false, true] {
        let mut flags: Vec<u32> = (0..32).map(|bit| 1u32 << bit).collect();
        flags.extend([0, u32::MAX, !FORCE_MONO, FORCE_MONO | 1]);
        for flag in flags {
            counts[usize::from(check_mono_gate(mono_output, flag))] += 1;
        }
    }
    assert!(counts[0] > 10 && counts[1] > 2, "{counts:?}");
}

/// Any `f32` bit pattern, with the special values often: finite, ±0,
/// ±inf and NaNs come from the bits, never from a float range.
fn any_f32_bits() -> impl Strategy<Value = f32> {
    prop_oneof![
        4 => any::<u32>().prop_map(f32::from_bits),
        1 => prop::sample::select(special_balances()),
        // [0, 1] by its bits
        2 => (0u32..=0x3f80_0000).prop_map(f32::from_bits),
        // [-1, 1] by its bits
        1 => (0u32..=0x3f80_0000, any::<bool>())
            .prop_map(|(bits, negative)| f32::from_bits(bits | if negative { 1 << 31 } else { 0 })),
    ]
}

fn any_u64() -> impl Strategy<Value = u64> {
    prop_oneof![
        3 => any::<u64>(),
        1 => prop::sample::select(TIMESTAMPS.to_vec()),
    ]
}

fn any_fmt() -> impl Strategy<Value = Fmt> {
    (
        prop_oneof![prop::sample::select(FORMATS.to_vec()), any::<i32>()],
        prop_oneof![prop::sample::select(RATES.to_vec()), any::<u32>()],
        prop_oneof![prop::sample::select(SPEAKERS.to_vec()), any::<i32>()],
    )
        .prop_map(|(f, r, s)| Fmt(f, r, s))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20000))]

    #[test]
    fn discontinuity_full_range(
        has_resampler in any::<bool>(),
        next in any_u64(),
        timestamp in prop_oneof![any_u64(), (any_u64(), 0u64..=3 * T)
            .prop_map(|(base, delta)| base.wrapping_add(delta))],
        near in any::<bool>(),
    ) {
        // half the time the timestamp is within a few thresholds of the end
        let timestamp = if near { next.wrapping_add(timestamp % (3 * T)).wrapping_sub(T) } else { timestamp };
        check_discontinuous(has_resampler, next, timestamp);
    }

    #[test]
    fn decisions_any_values(
        current in any_fmt(),
        packet in any_fmt(),
        same in any::<bool>(),
        bools in any::<[bool; 3]>(),
        mono_output in any::<bool>(),
        speakers in prop_oneof![Just(SPEAKERS_STEREO), any::<i32>()],
        balance in any_f32_bits(),
        flags in any::<u32>(),
    ) {
        let packet = if same { current } else { packet };
        check_reset(current, packet, bools[0], bools[1], bools[2]);
        check_resampler(current, packet, bools[0]);
        check_balance_gate(mono_output, speakers, balance);
        check_mono_gate(mono_output, flags);
    }
}

// --------------------------------------------------------------------------
// The loops: two copies of the same planes, the C on one, the shim on the
// other

const PLANES: usize = 9;
const GUARD: usize = 3;

/// `PLANES` planes of `frames + GUARD` samples, the samples given cycled.
fn planes_of(samples: &[f32], frames: usize) -> Vec<Vec<f32>> {
    let mut planes = Vec::with_capacity(PLANES);
    let mut next = 0usize;
    for _ in 0..PLANES {
        let mut plane = Vec::with_capacity(frames + GUARD);
        for _ in 0..frames + GUARD {
            plane.push(if samples.is_empty() {
                0.0
            } else {
                samples[next % samples.len()]
            });
            next += 1;
        }
        planes.push(plane);
    }
    planes
}

fn pointers(planes: &mut [Vec<f32>]) -> Vec<*mut f32> {
    planes.iter_mut().map(|p| p.as_mut_ptr()).collect()
}

/// Every sample's bits must match, with one exception: where one multiply
/// or add of the C had two NaN operands (`two_nans(plane, frame)`), which
/// NaN comes out depends on the operand order the compiler picks (x86
/// returns the first operand's), and C builds differ there too (the oracle
/// at -O0 and libobs at -O2 give different NaNs for the same two). There
/// both sides must give a NaN.
#[track_caller]
fn check_planes(
    c: &[Vec<f32>],
    r: &[Vec<f32>],
    what: &str,
    two_nans: impl Fn(usize, usize) -> bool,
) {
    for (index, (cp, rp)) in c.iter().zip(r).enumerate() {
        for (frame, (cs, rs)) in cp.iter().zip(rp).enumerate() {
            if rs.to_bits() != cs.to_bits() && rs.is_nan() && cs.is_nan() && two_nans(index, frame)
            {
                continue;
            }
            assert_eq!(
                rs.to_bits(),
                cs.to_bits(),
                "{what}: plane {index} frame {frame}: Rust {rs:e} ({:#010x}), C {cs:e} ({:#010x})",
                rs.to_bits(),
                cs.to_bits()
            );
        }
    }
}

/// The two gains of a known law, as the C computes them, to tell where a
/// product had two NaN operands.
fn gains_of(balance: f32, law: c_int) -> Option<(f32, f32)> {
    match law {
        SINE_LAW => Some(obs_source::audio_input::sine_gains(balance)),
        SQUARE_LAW => Some(((1.0f32 - balance).sqrt(), balance.sqrt())),
        LINEAR => Some((1.0f32 - balance, balance)),
        _ => None,
    }
}

fn run_balance(samples: &[f32], frames: usize, balance: f32, law: c_int) {
    let frames_u32 = u32::try_from(frames).unwrap();
    let mut c = planes_of(samples, frames);
    let mut r = c.clone();
    let mut cp = pointers(&mut c);
    let mut rp = pointers(&mut r);
    // SAFETY: two (and more) planes of frames + GUARD floats each.
    unsafe {
        oracle::oracle_obs_source_audio_input_balance(cp.as_mut_ptr(), frames_u32, balance, law);
        shim::obs_source_audio_input_balance(rp.as_mut_ptr(), frames_u32, balance, law);
    }
    let input = planes_of(samples, frames);
    let gains = gains_of(balance, law);
    check_planes(
        &c,
        &r,
        &format!(
            "balance({frames}, {balance:e} = {:#010x}, law {law})",
            balance.to_bits()
        ),
        |plane, frame| {
            let Some((left, right)) = gains else {
                return false;
            };
            let gain = match plane {
                0 => left,
                1 => right,
                _ => return false,
            };
            frame < frames && input[plane][frame].is_nan() && gain.is_nan()
        },
    );
}

fn run_downmix(samples: &[f32], frames: usize, channels: usize) {
    let frames_u32 = u32::try_from(frames).unwrap();
    let mut c = planes_of(samples, frames);
    let mut r = c.clone();
    // the C reads `channels` plane pointers, or one for 0: pass exactly those
    let count = channels.max(1);
    let mut cp: Vec<*mut f32> = pointers(&mut c).into_iter().take(count).collect();
    let mut rp: Vec<*mut f32> = pointers(&mut r).into_iter().take(count).collect();
    // SAFETY: `count` planes of frames + GUARD floats each.
    unsafe {
        oracle::oracle_obs_source_audio_input_downmix(cp.as_mut_ptr(), channels, frames_u32);
        shim::obs_source_audio_input_downmix(rp.as_mut_ptr(), channels, frames_u32);
    }
    // Replay the C's adds frame by frame: did one add two NaNs?
    let input = planes_of(samples, frames);
    let two_nan_frames: Vec<bool> = (0..frames)
        .map(|frame| {
            let mut sum = input[0][frame];
            let mut two = false;
            for plane in input.iter().take(channels).skip(1) {
                two |= sum.is_nan() && plane[frame].is_nan();
                sum += plane[frame];
            }
            two
        })
        .collect();
    check_planes(
        &c,
        &r,
        &format!("downmix({frames}, {channels} channels)"),
        |plane, frame| plane < count && frame < frames && two_nan_frames[frame],
    );
}

const LAWS: [c_int; 8] = [SINE_LAW, SQUARE_LAW, LINEAR, 3, -1, 100, i32::MIN, i32::MAX];

#[test]
fn laws_over_the_special_balances() {
    let samples: Vec<f32> = special_balances()
        .into_iter()
        .chain([0.3, -0.7, 1e-38, -1e-45, 12345.678])
        .collect();
    for balance in special_balances() {
        for law in LAWS {
            for frames in [0, 1, 7, 64] {
                run_balance(&samples, frames, balance, law);
            }
        }
    }
}

#[test]
fn downmix_over_every_channel_count() {
    let samples: Vec<f32> = special_balances()
        .into_iter()
        .chain([0.3, -0.7, 1e-38, -1e-45, 12345.678, 1e8, -1e8, 1.0])
        .collect();
    for channels in 0..=8 {
        for frames in [0, 1, 7, 64] {
            run_downmix(&samples, frames, channels);
        }
    }
}

/// The sine law's gains, through a plane of ones: 1 times the gain is the
/// gain's bits (a NaN gain is quieted the same way on both sides).
fn check_sine_gains(balance: f32) {
    let mut c = [1.0f32, 1.0];
    let mut r = [1.0f32, 1.0];
    let mut cp = [&raw mut c[0], &raw mut c[1]];
    let mut rp = [&raw mut r[0], &raw mut r[1]];
    // SAFETY: two planes of one float each.
    unsafe {
        oracle::oracle_obs_source_audio_input_balance(cp.as_mut_ptr(), 1, balance, SINE_LAW);
        shim::obs_source_audio_input_balance(rp.as_mut_ptr(), 1, balance, SINE_LAW);
    }
    assert_eq!(
        (r[0].to_bits(), r[1].to_bits()),
        (c[0].to_bits(), c[1].to_bits()),
        "sine gains of {balance:e} ({:#010x})",
        balance.to_bits()
    );
}

#[test]
fn sine_gains_over_strided_bit_patterns() {
    let mut checked = 0usize;
    for bits in (0..=u32::MAX).step_by(4099) {
        check_sine_gains(f32::from_bits(bits));
        checked += 1;
    }
    for bits in (0..=0x3f80_0000u32).step_by(257) {
        check_sine_gains(f32::from_bits(bits));
        checked += 1;
    }
    assert!(checked > 5_000_000, "{checked}");
}

/// Every `f32` balance. Slow in a debug build: run it once per platform
/// with `cargo test --release ... -- --ignored exhaustive_sine_gains`.
#[test]
#[ignore = "all 2^32 balances; run with --release --ignored"]
fn exhaustive_sine_gains() {
    for bits in 0..=u32::MAX {
        check_sine_gains(f32::from_bits(bits));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20000))]

    #[test]
    fn laws_random(
        samples in prop::collection::vec(any_f32_bits(), 1..80),
        frames in 0usize..=64,
        balance in any_f32_bits(),
        law in prop_oneof![
            4 => prop::sample::select(vec![SINE_LAW, SQUARE_LAW, LINEAR]),
            1 => any::<c_int>(),
        ],
    ) {
        run_balance(&samples, frames, balance, law);
    }

    #[test]
    fn downmix_random(
        samples in prop::collection::vec(any_f32_bits(), 1..80),
        frames in 0usize..=64,
        channels in 0usize..=8,
    ) {
        run_downmix(&samples, frames, channels);
    }
}

#[test]
fn nothing_is_read_without_frames_or_a_law() {
    for law in LAWS {
        // SAFETY: zero frames read nothing on either side.
        unsafe {
            oracle::oracle_obs_source_audio_input_balance(std::ptr::null_mut(), 0, 0.25, law);
            shim::obs_source_audio_input_balance(std::ptr::null_mut(), 0, 0.25, law);
        }
    }
    for law in [3, -1, 100, i32::MIN, i32::MAX] {
        // SAFETY: an unknown law reads nothing on either side.
        unsafe {
            oracle::oracle_obs_source_audio_input_balance(std::ptr::null_mut(), 64, 0.25, law);
            shim::obs_source_audio_input_balance(std::ptr::null_mut(), 64, 0.25, law);
        }
    }
    for channels in [0, 1, 2, 8, 1000] {
        // SAFETY: zero frames read nothing on either side.
        unsafe {
            oracle::oracle_obs_source_audio_input_downmix(std::ptr::null_mut(), channels, 0);
            shim::obs_source_audio_input_downmix(std::ptr::null_mut(), channels, 0);
        }
    }
}

/// The intentional difference, on the shim only: a null `data` or plane.
#[test]
fn null_planes_do_nothing_in_the_shim() {
    // SAFETY: the shim checks the pointers.
    unsafe {
        shim::obs_source_audio_input_balance(std::ptr::null_mut(), 8, 0.25, SINE_LAW);
        shim::obs_source_audio_input_downmix(std::ptr::null_mut(), 2, 8);
    }
    let mut plane = [1.0f32; 8];
    let mut data = [plane.as_mut_ptr(), std::ptr::null_mut()];
    // SAFETY: the shim checks the plane pointers before writing.
    unsafe {
        shim::obs_source_audio_input_balance(data.as_mut_ptr(), 8, 0.25, SINE_LAW);
        shim::obs_source_audio_input_downmix(data.as_mut_ptr(), 2, 8);
    }
    assert!(plane.iter().all(|&s| s.to_bits() == 1.0f32.to_bits()));
}
