//! Tier 3: the `obs_source_audio_place_*` shims and the safe core against
//! the original C (`obs-c-oracle`, `oracle/source_audio_place.c`: the
//! unmodified `libobs/obs-source-audio-place.c.h` with its functions
//! renamed).
//!
//! The eleven functions take and return `uint64_t`, `uint32_t`, `size_t`,
//! `int` and `bool` by value, so a run compares each `bool` as the byte the
//! C returns (it must be 0 or 1, and the same on both sides) and each
//! number exactly. Each input is given to the oracle, to the shim (the
//! `extern "C"` function the hybrid links) and to the core.
//!
//! Runs:
//! - the header's values and `sizeof(size_t)`;
//! - the reset rules over every pair of 30 special timestamps, and the
//!   placement and the action frame over every pair at 16 rates (0, 1,
//!   8000 to 384000 Hz, 1e9, 1e9 + 1 and `u32::MAX`);
//! - the first and last nanosecond of every frame from 0 to 2048 at eleven
//!   rates, and every 65,521st difference at five;
//! - the size over every frame count to 100,000 and every 65,521st
//!   `uint32_t`; the fit, the fill's end and the stop over every pair, and
//!   the pop over every triple, of 17 special sizes; the stop over every
//!   frame to 10,000;
//! - every monitoring type from -100,000 to 100,000, every 65,521st `int`
//!   and the ends;
//! - the mix rule for every low 16-bit mask under eight high words at every
//!   mix from 0 to 30;
//! - the callers (`source_output_audio_place()`,
//!   `source_output_audio_push_back()`, the last step of
//!   `source_output_audio_data()` and `apply_audio_actions()`, as the C
//!   test's stand-in source) on the oracle and on the shims: 36,864 packet
//!   pairs over rates, starts, channels, monitoring types, push backs,
//!   timestamps and sizes around `MAX_BUF_SIZE`, and 21,316 action lists,
//!   comparing every trace, buffer, volume array and mix;
//! - proptests (20,000 cases each) of every function over full-range values
//!   and of the callers over packet and action sequences from any state.
//!
//! Not compared: on MSVC x64 a placement or frame that does not fit in 64
//! bits, where `util_mul_div64()` is `_umul128()` and `_udiv128()`, whose
//! division faults (the process dies); the core saturates there, an
//! intentional difference for a rate above 1e9 Hz. And the mix rule at 31
//! and up, where the C shifts an `int` past its sign bit (undefined); the
//! core says no mix from 32 on, the other intentional difference. libobs
//! asks only mixes 0 to 5. Otherwise no input is excluded.

use std::os::raw::c_int;

use obs_c_oracle::source_audio_place as oracle;
use obs_source::audio_place::{self as core, FRAMES, MAX_BUF_SIZE, MONITOR_ONLY};
use obs_source::ffi::source_audio_place as shim;
use proptest::prelude::*;

const SEC: u64 = 1_000_000_000;
const MIXES: usize = 6;

/// Whether `util_mul_div64()` is the 128-bit math that faults on overflow.
const UDIV128: bool = cfg!(all(target_env = "msvc", target_arch = "x86_64"));

/// Whether the C faults for this conversion: on MSVC x64 only, a quotient
/// past 64 bits.
fn c_faults(timestamp: u64, audio_ts: u64, rate: u64) -> bool {
    let q = u128::from(timestamp.wrapping_sub(audio_ts)) * u128::from(rate) / u128::from(SEC);
    UDIV128 && q > u128::from(u64::MAX)
}

#[track_caller]
fn c_bool(byte: u8, what: &str) -> bool {
    assert!(byte <= 1, "{what}: the C returned the byte {byte}");
    byte == 1
}

// one call on all three sides

#[track_caller]
fn started(a: u64) -> bool {
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_audio_place_started(a) },
        "started",
    );
    assert_eq!(
        shim::obs_source_audio_place_started(a),
        c,
        "shim started({a})"
    );
    assert_eq!(core::started(a), c, "core started({a})");
    c
}

#[track_caller]
fn older(a: u64, b: u64) -> bool {
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_audio_place_older(a, b) },
        "older",
    );
    assert_eq!(
        shim::obs_source_audio_place_older(a, b),
        c,
        "shim older({a}, {b})"
    );
    assert_eq!(core::older(a, b), c, "core older({a}, {b})");
    c
}

#[track_caller]
fn size(f: u32) -> usize {
    let c = unsafe { oracle::oracle_obs_source_audio_place_size(f) };
    assert_eq!(shim::obs_source_audio_place_size(f), c, "shim size({f})");
    assert_eq!(core::size(f), c, "core size({f})");
    c
}

/// `None` where the C would fault (MSVC x64 only).
#[track_caller]
fn offset(a: u64, b: u64, rate: u32) -> Option<usize> {
    if c_faults(a, b, u64::from(rate)) {
        return None;
    }
    let c = unsafe { oracle::oracle_obs_source_audio_place_offset(a, b, rate) };
    assert_eq!(
        shim::obs_source_audio_place_offset(a, b, rate),
        c,
        "shim offset({a}, {b}, {rate})"
    );
    assert_eq!(core::offset(a, b, rate), c, "core offset({a}, {b}, {rate})");
    Some(c)
}

#[track_caller]
fn fits(a: usize, b: usize) -> bool {
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_audio_place_fits(a, b) },
        "fits",
    );
    assert_eq!(
        shim::obs_source_audio_place_fits(a, b),
        c,
        "shim fits({a}, {b})"
    );
    assert_eq!(core::fits(a, b), c, "core fits({a}, {b})");
    c
}

#[track_caller]
fn pop_back(a: usize, b: usize, s: usize) -> usize {
    let c = unsafe { oracle::oracle_obs_source_audio_place_pop_back(a, b, s) };
    assert_eq!(
        shim::obs_source_audio_place_pop_back(a, b, s),
        c,
        "shim pop_back({a}, {b}, {s})"
    );
    assert_eq!(core::pop_back(a, b, s), c, "core pop_back({a}, {b}, {s})");
    c
}

#[track_caller]
fn outputs(t: c_int) -> bool {
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_audio_place_outputs(t) },
        "outputs",
    );
    assert_eq!(
        shim::obs_source_audio_place_outputs(t),
        c,
        "shim outputs({t})"
    );
    assert_eq!(core::outputs(t), c, "core outputs({t})");
    c
}

/// `None` where the C would fault (MSVC x64 only).
#[track_caller]
fn action_frame(a: u64, b: u64, rate: usize) -> Option<usize> {
    if c_faults(a, b, u64::try_from(rate).unwrap()) {
        return None;
    }
    let c = unsafe { oracle::oracle_obs_source_audio_place_action_frame(a, b, rate) };
    assert_eq!(
        shim::obs_source_audio_place_action_frame(a, b, rate),
        c,
        "shim action_frame({a}, {b}, {rate})"
    );
    assert_eq!(
        core::action_frame(a, b, rate),
        c,
        "core action_frame({a}, {b}, {rate})"
    );
    Some(c)
}

#[track_caller]
fn action_stops(f: usize) -> bool {
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_audio_place_action_stops(f) },
        "stops",
    );
    assert_eq!(
        shim::obs_source_audio_place_action_stops(f),
        c,
        "shim action_stops({f})"
    );
    assert_eq!(core::action_stops(f), c, "core action_stops({f})");
    c
}

#[track_caller]
fn fill_end(a: usize, b: usize) -> usize {
    let c = unsafe { oracle::oracle_obs_source_audio_place_fill_end(a, b) };
    assert_eq!(
        shim::obs_source_audio_place_fill_end(a, b),
        c,
        "shim fill_end({a}, {b})"
    );
    assert_eq!(core::fill_end(a, b), c, "core fill_end({a}, {b})");
    c
}

/// Only below 31, where the C's `int` shift is defined.
#[track_caller]
fn mix_on(mask: u32, mix: usize) -> bool {
    assert!(mix < 31);
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_audio_place_mix_on(mask, mix) },
        "mix_on",
    );
    assert_eq!(
        shim::obs_source_audio_place_mix_on(mask, mix),
        c,
        "shim mix_on({mask:#x}, {mix})"
    );
    assert_eq!(core::mix_on(mask, mix), c, "core mix_on({mask:#x}, {mix})");
    c
}

const SPECIAL_TS: [u64; 30] = [
    0,
    1,
    2,
    2604,
    2605,
    20833,
    20834,
    21_333_333,
    21_333_334,
    SEC - 1,
    SEC,
    SEC + 1,
    (1 << 32) - 1,
    1 << 32,
    (1 << 32) + 1,
    1000 * SEC - 1,
    1000 * SEC,
    1000 * SEC + 1,
    1000 * SEC + 20834,
    1000 * SEC + 21_333_334,
    1000 * SEC + 21_312_000_000,
    (1 << 63) - 1,
    1 << 63,
    (1 << 63) + 1,
    u64::MAX - SEC,
    u64::MAX - 20834,
    u64::MAX - 2,
    u64::MAX - 1,
    u64::MAX,
    18_446_744_073,
];

const RATES: [u32; 16] = [
    0,
    1,
    8000,
    11025,
    16000,
    22050,
    32000,
    44100,
    48000,
    88200,
    96000,
    192_000,
    384_000,
    1_000_000_000,
    1_000_000_001,
    u32::MAX,
];

const SPECIAL_SIZES: [usize; 17] = [
    0,
    1,
    4,
    1023,
    1024,
    4095,
    4096,
    4097,
    MAX_BUF_SIZE - 4097,
    MAX_BUF_SIZE - 4096,
    MAX_BUF_SIZE - 1,
    MAX_BUF_SIZE,
    MAX_BUF_SIZE + 1,
    u32::MAX as usize,
    usize::MAX - MAX_BUF_SIZE,
    usize::MAX - 1,
    usize::MAX,
];

#[test]
fn values_match_header() {
    let expected = [
        i64::try_from(FRAMES).unwrap(),
        i64::try_from(MAX_BUF_SIZE).unwrap(),
        i64::from(MONITOR_ONLY),
        i64::try_from(size_of::<usize>()).unwrap(),
    ];
    for (i, (&name, want)) in oracle::VALUE_NAMES.iter().zip(expected).enumerate() {
        assert_eq!(
            unsafe { oracle::oracle_source_audio_place_value(i) },
            want,
            "{name}"
        );
    }
    assert_eq!(
        unsafe { oracle::oracle_source_audio_place_value(oracle::VALUE_NAMES.len()) },
        i64::MIN
    );
}

#[test]
fn timestamp_pairs_match() {
    for a in SPECIAL_TS {
        started(a);
        for b in SPECIAL_TS {
            older(a, b);
            for rate in RATES {
                offset(a, b, rate);
                action_frame(a, b, rate as usize);
            }
        }
    }
    // size_t rates past u32 for the action frame
    for a in SPECIAL_TS {
        for rate in [1_usize << 32, 1 << 40, usize::MAX] {
            action_frame(a, 0, rate);
            action_frame(a, a.wrapping_sub(SEC), rate);
        }
    }
}

#[test]
fn frame_boundaries_match() {
    for rate in [
        8000_u32, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 192_000, 384_000,
    ] {
        let base = 5 * SEC;
        for frame in 0..=2048_u64 {
            let first = (frame * SEC).div_ceil(u64::from(rate));
            for ns in [first, first.saturating_sub(1), first + 1] {
                assert_eq!(
                    action_frame(base + ns, base, rate as usize),
                    Some(core::action_frame(base + ns, base, rate as usize))
                );
                offset(base + ns, base, rate);
            }
        }
    }
    for rate in [8000_u32, 44100, 48000, 384_000, u32::MAX] {
        let mut d = 0_u64;
        for _ in 0..70_000 {
            offset(d, 0, rate);
            action_frame(d, 0, rate as usize);
            d = d.wrapping_add(65_521 * 4_294_967);
        }
    }
}

#[test]
fn sizes_match() {
    for f in 0..=100_000_u32 {
        size(f);
    }
    let mut f = 0_u32;
    for _ in 0..65_600 {
        size(f);
        f = f.wrapping_add(65_521);
    }
    size(u32::MAX);
    for a in SPECIAL_SIZES {
        action_stops(a);
        for b in SPECIAL_SIZES {
            fits(a, b);
            fill_end(a, b);
            for c in SPECIAL_SIZES {
                pop_back(a, b, c);
            }
        }
    }
    for frame in 0..=10_000 {
        action_stops(frame);
    }
}

#[test]
fn monitoring_types_match() {
    for t in -100_000..=100_000 {
        outputs(t);
    }
    let mut t = i32::MIN;
    for _ in 0..65_600 {
        outputs(t);
        t = t.wrapping_add(65_521);
    }
    outputs(i32::MAX);
}

#[test]
fn mixer_masks_match() {
    for high in [
        0_u32,
        1 << 16,
        1 << 20,
        1 << 30,
        1 << 31,
        0x5555_0000,
        0xaaaa_0000,
        0xffff_0000,
    ] {
        for low in 0_u32..=0xffff {
            for mix in 0..31 {
                mix_on(high | low, mix);
            }
        }
    }
}

// the callers on a stand-in source, over the oracle and over the shims

/// The eleven functions of one side.
struct Api {
    started: fn(u64) -> bool,
    older: fn(u64, u64) -> bool,
    size: fn(u32) -> usize,
    offset: fn(u64, u64, u32) -> usize,
    fits: fn(usize, usize) -> bool,
    pop_back: fn(usize, usize, usize) -> usize,
    outputs: fn(c_int) -> bool,
    action_frame: fn(u64, u64, usize) -> usize,
    action_stops: fn(usize) -> bool,
    fill_end: fn(usize, usize) -> usize,
    mix_on: fn(u32, usize) -> bool,
}

const ORACLE: Api = Api {
    started: |a| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_place_started(a) },
            "started",
        )
    },
    older: |a, b| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_place_older(a, b) },
            "older",
        )
    },
    size: |f| unsafe { oracle::oracle_obs_source_audio_place_size(f) },
    offset: |a, b, r| unsafe { oracle::oracle_obs_source_audio_place_offset(a, b, r) },
    fits: |a, b| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_place_fits(a, b) },
            "fits",
        )
    },
    pop_back: |a, b, s| unsafe { oracle::oracle_obs_source_audio_place_pop_back(a, b, s) },
    outputs: |t| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_place_outputs(t) },
            "outputs",
        )
    },
    action_frame: |a, b, r| unsafe { oracle::oracle_obs_source_audio_place_action_frame(a, b, r) },
    action_stops: |f| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_place_action_stops(f) },
            "stops",
        )
    },
    fill_end: |a, b| unsafe { oracle::oracle_obs_source_audio_place_fill_end(a, b) },
    mix_on: |m, x| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_place_mix_on(m, x) },
            "mix_on",
        )
    },
};

const SHIMS: Api = Api {
    started: |a| shim::obs_source_audio_place_started(a),
    older: |a, b| shim::obs_source_audio_place_older(a, b),
    size: |f| shim::obs_source_audio_place_size(f),
    offset: |a, b, r| shim::obs_source_audio_place_offset(a, b, r),
    fits: |a, b| shim::obs_source_audio_place_fits(a, b),
    pop_back: |a, b, s| shim::obs_source_audio_place_pop_back(a, b, s),
    outputs: |t| shim::obs_source_audio_place_outputs(t),
    action_frame: |a, b, r| shim::obs_source_audio_place_action_frame(a, b, r),
    action_stops: |f| shim::obs_source_audio_place_action_stops(f),
    fill_end: |a, b| shim::obs_source_audio_place_fill_end(a, b),
    mix_on: |m, x| shim::obs_source_audio_place_mix_on(m, x),
};

#[derive(Clone, Debug, PartialEq)]
struct Standin {
    audio_ts: u64,
    buf: [usize; 8],
    channels: usize,
    sample_rate: u32,
    monitoring_type: c_int,
    trace: Vec<(u8, usize, usize)>,
}

impl Standin {
    fn new(rate: u32, audio_ts: u64, channels: usize, buf: usize) -> Self {
        Self {
            audio_ts,
            buf: [buf; 8],
            channels,
            sample_rate: rate,
            monitoring_type: 0,
            trace: Vec::new(),
        }
    }

    /// `source_output_audio_place()` with `reset_audio_data()`; the deque
    /// grows to the end of the place and saturates where a real one could
    /// not hold the bytes.
    fn place(&mut self, api: &Api, timestamp: u64, frames: u32) {
        let size = (api.size)(frames);
        if !(api.started)(self.audio_ts) || (api.older)(timestamp, self.audio_ts) {
            self.buf = [0; 8];
            self.audio_ts = timestamp;
            self.trace.push((b'r', 0, 0));
        }
        let placement = (api.offset)(timestamp, self.audio_ts, self.sample_rate);
        if !(api.fits)(placement, size) {
            self.trace.push((b'd', placement, size));
            return;
        }
        for i in 0..self.channels {
            let end = placement.wrapping_add(size);
            if self.buf[i] < end {
                self.buf[i] = end;
            }
            let pop = (api.pop_back)(self.buf[i], placement, size);
            self.buf[i] = self.buf[i].saturating_sub(pop);
            self.trace.push((b'c', i, pop));
        }
        self.trace.push((b'p', placement, size));
    }

    /// `source_output_audio_push_back()`.
    fn push_back(&mut self, api: &Api, frames: u32) {
        let size = (api.size)(frames);
        if !(api.fits)(self.buf[0], size) {
            self.trace.push((b'd', self.buf[0], size));
            return;
        }
        for i in 0..self.channels {
            self.buf[i] = self.buf[i].saturating_add(size);
        }
        self.trace.push((b'b', size, 0));
    }

    /// The last step of `source_output_audio_data()`.
    fn output(&mut self, api: &Api, push_back: bool, timestamp: u64, frames: u32) {
        if (api.outputs)(self.monitoring_type) {
            if push_back && (api.started)(self.audio_ts) {
                self.push_back(api, frames);
            } else {
                self.place(api, timestamp, frames);
            }
        } else {
            self.trace.push((b's', 0, 0));
        }
    }
}

/// `apply_audio_actions()`: the actions applied, the volume array (the
/// volume after action i is i + 2, before the first 1) and the mixes.
fn actions(
    api: &Api,
    audio_ts: u64,
    rate: usize,
    list: &[u64],
    mixers: u32,
) -> (usize, Vec<f32>, Vec<bool>) {
    let mut vol_data = vec![0.0_f32; FRAMES];
    let mut cur_vol = 1.0_f32;
    let mut frame_num = 0;
    let mut applied = 0;
    for (i, &action_ts) in list.iter().enumerate() {
        let mut timestamp = action_ts;
        if (api.older)(timestamp, audio_ts) {
            timestamp = audio_ts;
        }
        let new_frame_num = (api.action_frame)(timestamp, audio_ts, rate);
        if (api.action_stops)(new_frame_num) {
            break;
        }
        applied += 1;
        let end = (api.fill_end)(frame_num, new_frame_num);
        while frame_num < end {
            vol_data[frame_num] = cur_vol;
            frame_num += 1;
        }
        cur_vol = (i + 2) as f32;
    }
    while frame_num < FRAMES {
        vol_data[frame_num] = cur_vol;
        frame_num += 1;
    }
    let mixes = (0..MIXES).map(|mix| (api.mix_on)(mixers, mix)).collect();
    (applied, vol_data, mixes)
}

/// Whether the callers can reach the C's fault (MSVC x64): only with a
/// rate above 1e9, since a quotient of `diff * rate / 1e9` is otherwise at
/// most `diff`.
fn callers_fault(rate: u64) -> bool {
    UDIV128 && rate > SEC
}

fn run_packets(
    rate: u32,
    start: u64,
    channels: usize,
    buf: usize,
    packets: &[(bool, c_int, u64, u32)],
) {
    if callers_fault(u64::from(rate)) {
        return;
    }
    let mut o = Standin::new(rate, start, channels, buf);
    let mut s = o.clone();
    for &(push_back, monitoring_type, ts, frames) in packets {
        o.monitoring_type = monitoring_type;
        s.monitoring_type = monitoring_type;
        o.output(&ORACLE, push_back, ts, frames);
        s.output(&SHIMS, push_back, ts, frames);
        assert_eq!(
            o, s,
            "rate {rate} start {start} channels {channels} buf {buf} {packets:?}"
        );
    }
}

fn run_actions(audio_ts: u64, rate: usize, list: &[u64], mixers: u32) {
    if callers_fault(u64::try_from(rate).unwrap()) {
        return;
    }
    assert_eq!(
        actions(&ORACLE, audio_ts, rate, list, mixers),
        actions(&SHIMS, audio_ts, rate, list, mixers),
        "audio_ts {audio_ts} rate {rate} {list:?} {mixers:#x}"
    );
}

#[test]
fn callers_match_on_packets() {
    let base = 1000 * SEC;
    let starts = [0, base, base - 1, u64::MAX];
    let stamps = [
        base,
        base + 1,
        base - 1,
        base + 20834,
        base + SEC,
        base + 21_312_000_000,
        base + 21_312_020_834,
        0,
    ];
    let frames = [0, 1, 1024, 1_024_000, 1_024_001, u32::MAX];
    let mut runs = 0;
    for rate in [8000, 44100, 48000, 384_000] {
        for start in starts {
            for (channels, buf) in [(0, 0), (2, 0), (8, MAX_BUF_SIZE - 4096)] {
                for push_back in [false, true] {
                    for monitoring_type in [0, MONITOR_ONLY, 2, 7] {
                        for &a in &stamps {
                            for &fa in &frames[..3] {
                                for &b in &stamps[..4] {
                                    for &fb in &[frames[2], frames[4]] {
                                        run_packets(
                                            rate,
                                            start,
                                            channels,
                                            buf,
                                            &[
                                                (push_back, monitoring_type, a, fa),
                                                (!push_back, 0, b, fb),
                                            ],
                                        );
                                        runs += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(runs, 4 * 4 * 3 * 2 * 4 * 8 * 3 * 4 * 2);
}

#[test]
fn callers_match_on_actions() {
    let base = 20 * SEC;
    let near = [
        0,
        base - SEC,
        base - 1,
        base,
        base + 1,
        base + 20834,
        base + 21_333_333 / 2,
        base + 21_333_333,
        base + 21_333_334,
        base + 23_219_955,
        base + 128_000_000,
        u64::MAX,
    ];
    let mut runs = 0;
    for rate in [8000_usize, 44100, 48000, 384_000] {
        for &a in &near {
            for &b in &near {
                for &c in &near {
                    for mixers in [0x3f, 0x15, 0] {
                        run_actions(base, rate, &[a, b, c], mixers);
                        runs += 1;
                    }
                }
                run_actions(base, rate, &[a, b], 0x3f);
                runs += 1;
            }
        }
        run_actions(base, rate, &[], 0x3f);
        runs += 1;
    }
    assert_eq!(runs, 4 * (12 * 12 * (12 * 3 + 1) + 1));
}

fn near(base: u64) -> impl Strategy<Value = u64> {
    prop_oneof![
        Just(base),
        (0_u64..4).prop_map(move |d| base.wrapping_add(d)),
        (0_u64..4).prop_map(move |d| base.wrapping_sub(d)),
        (0_u64..2_000_000).prop_map(move |n| base.wrapping_add(n * 20833)),
        (0_u64..1100).prop_map(move |n| base.wrapping_add(n * 21_333_333)),
        (0_u64..30).prop_map(move |n| base.wrapping_sub(n * SEC)),
        any::<u64>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn functions_match_full_range(
        a in any::<u64>(),
        b in any::<u64>(),
        rate in any::<u32>(),
        wide_rate in any::<usize>(),
        f in any::<u32>(),
        x in any::<usize>(),
        y in any::<usize>(),
        z in any::<usize>(),
        t in any::<i32>(),
        mask in any::<u32>(),
        mix in 0_usize..31,
    ) {
        started(a);
        older(a, b);
        size(f);
        offset(a, b, rate);
        action_frame(a, b, rate as usize);
        action_frame(a, b, wide_rate);
        fits(x, y);
        pop_back(x, y, z);
        outputs(t);
        action_stops(x);
        fill_end(x, y);
        mix_on(mask, mix);
    }

    #[test]
    fn callers_match_on_packet_sequences(
        rate in prop_oneof![prop::sample::select(RATES.to_vec()), any::<u32>()],
        start in prop_oneof![Just(0_u64), near(1000 * SEC), any::<u64>()],
        channels in 0_usize..=8,
        buf in prop_oneof![Just(0_usize), 0..=MAX_BUF_SIZE + 8192, any::<usize>()],
        packets in prop::collection::vec(
            (any::<bool>(), prop_oneof![0..4, any::<i32>()], near(1000 * SEC),
             prop_oneof![prop::sample::select(vec![0_u32, 1, 480, 1024, 1_024_000, 1_024_001]), any::<u32>()]),
            1..24,
        ),
    ) {
        run_packets(rate, start, channels, buf, &packets);
    }

    #[test]
    fn callers_match_on_action_sequences(
        rate in prop_oneof![prop::sample::select(vec![8000_usize, 44100, 48000, 384_000]), any::<u32>().prop_map(|r| r as usize)],
        audio_ts in prop_oneof![near(20 * SEC), any::<u64>()],
        list in prop::collection::vec(prop_oneof![near(20 * SEC), any::<u64>()], 0..12),
        mixers in any::<u32>(),
    ) {
        run_actions(audio_ts, rate, &list, mixers);
    }
}
