//! Tier 3: the `obs_source_audio_render_*` shims and the safe core against
//! the original C (`obs-c-oracle`, `oracle/source_audio_render.c`: the
//! unmodified `libobs/obs-source-audio-render.c.h` with its functions
//! renamed).
//!
//! The thirteen functions take and return plain values, so a run compares
//! the route as the `u32` the C returns, the timestamp as a `u64`, and each
//! `bool` as the byte the C returns (it must be 0 or 1, and the same on
//! both sides). Each input is given to the oracle, to the shim (the
//! `extern "C"` function the hybrid links) and to the core.
//!
//! Runs:
//! - the enum's values and size against the C header;
//! - the route over all eight inputs;
//! - every mask decision exhaustively over every pair of the low seven mask
//!   bits (2^14 pairs), with no high bits and with all of them, for every
//!   mix index of 0 to 30, submix on and off (the submix copy and the stop
//!   also for any index up to 2^16 and `usize::MAX`, where the C does not
//!   shift);
//! - the timestamp, pending and size decisions over every pair of 30
//!   special `u64` and `usize` values, at and around the tick sizes and the
//!   ends;
//! - the three callers of `obs-source.c`, copied onto a stand-in source and
//!   run once over the oracle and once over the shims, with the lock, the
//!   peeks, every memset (with its length) and memcpy, the callbacks and the
//!   volume recorded: every combination of the six source flags with every
//!   pair of the low seven mask bits and edge sizes, and a proptest over
//!   full-range masks, timestamps and sizes; the recorded calls, `audio_ts`
//!   and `audio_pending` must match;
//! - proptests over any masks, any `u64` timestamps and any `usize` sizes
//!   for each decision.
//!
//! MSVC x64: nothing here divides or reaches `util_mul_div64()`, so neither
//! `_udiv128` nor `INT_MIN / -1` can trap; no input is guarded.
//!
//! Intentional difference (tested in Tier 1 only): a mix of 31 or more for
//! the shifting decisions, where the C's `int` shift is undefined.

use obs_c_oracle::source_audio_render as oracle;
use obs_source::audio_render_plan::{self as core, Route};
use obs_source::ffi::source_audio_render as shim;
use proptest::prelude::*;

const MIXES: usize = 6;
const CHANNELS: usize = 8;
const TICK_BYTES: usize = 4096;
/// The C shifts an `int` by the mix: defined up to 30.
const DEFINED_MIXES: usize = 31;

#[track_caller]
fn c_bool(byte: u8, what: &str) -> bool {
    assert!(byte <= 1, "{what}: the C returned the byte {byte}");
    byte == 1
}

fn route_u32(route: Route) -> u32 {
    u32::from(route)
}

/// One side of the comparison: the C oracle or the Rust shims.
struct Side {
    route: fn(bool, bool, bool) -> u32,
    custom_clear_before: fn(u32, u32, usize) -> bool,
    custom_ts: fn(bool, u64) -> u64,
    custom_pending: fn(bool) -> bool,
    custom_unmixed: fn(u64, u32) -> bool,
    custom_in_tick: fn(u32, usize) -> bool,
    custom_clear_after: fn(u32, usize) -> bool,
    tick_unset: fn(u64) -> bool,
    tick_short: fn(usize, usize) -> bool,
    tick_stop: fn(bool, usize) -> bool,
    tick_copy: fn(bool, u32, u32, usize) -> bool,
    tick_volume: fn(bool) -> bool,
    tick_clear_main: fn(u32, u32) -> bool,
}

// SAFETY (every oracle call below): plain values in and out; the C touches
// no memory.
const ORACLE: Side = Side {
    route: |b, c, d| unsafe { oracle::oracle_obs_source_audio_render_route(b, c, d) },
    custom_clear_before: |a, m, x| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_render_custom_clear_before(a, m, x) },
            "custom_clear_before",
        )
    },
    custom_ts: |s, t| unsafe { oracle::oracle_obs_source_audio_render_custom_ts(s, t) },
    custom_pending: |s| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_render_custom_pending(s) },
            "custom_pending",
        )
    },
    custom_unmixed: |t, m| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_render_custom_unmixed(t, m) },
            "custom_unmixed",
        )
    },
    custom_in_tick: |m, x| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_render_custom_in_tick(m, x) },
            "custom_in_tick",
        )
    },
    custom_clear_after: |a, x| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_render_custom_clear_after(a, x) },
            "custom_clear_after",
        )
    },
    tick_unset: |t| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_render_tick_unset(t) },
            "tick_unset",
        )
    },
    tick_short: |i, s| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_render_tick_short(i, s) },
            "tick_short",
        )
    },
    tick_stop: |s, x| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_render_tick_stop(s, x) },
            "tick_stop",
        )
    },
    tick_copy: |s, a, m, x| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_render_tick_copy(s, a, m, x) },
            "tick_copy",
        )
    },
    tick_volume: |s| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_render_tick_volume(s) },
            "tick_volume",
        )
    },
    tick_clear_main: |a, m| {
        c_bool(
            unsafe { oracle::oracle_obs_source_audio_render_tick_clear_main(a, m) },
            "tick_clear_main",
        )
    },
};

const SHIM: Side = Side {
    route: |a, b, c| shim::obs_source_audio_render_route(a, b, c),
    custom_clear_before: |a, b, c| shim::obs_source_audio_render_custom_clear_before(a, b, c),
    custom_ts: |a, b| shim::obs_source_audio_render_custom_ts(a, b),
    custom_pending: |a| shim::obs_source_audio_render_custom_pending(a),
    custom_unmixed: |a, b| shim::obs_source_audio_render_custom_unmixed(a, b),
    custom_in_tick: |a, b| shim::obs_source_audio_render_custom_in_tick(a, b),
    custom_clear_after: |a, b| shim::obs_source_audio_render_custom_clear_after(a, b),
    tick_unset: |a| shim::obs_source_audio_render_tick_unset(a),
    tick_short: |a, b| shim::obs_source_audio_render_tick_short(a, b),
    tick_stop: |a, b| shim::obs_source_audio_render_tick_stop(a, b),
    tick_copy: |a, b, c, d| shim::obs_source_audio_render_tick_copy(a, b, c, d),
    tick_volume: |a| shim::obs_source_audio_render_tick_volume(a),
    tick_clear_main: |a, b| shim::obs_source_audio_render_tick_clear_main(a, b),
};

const CORE: Side = Side {
    route: |b, c, d| route_u32(core::route(b, c, d)),
    custom_clear_before: core::custom_clear_before,
    custom_ts: core::custom_ts,
    custom_pending: core::custom_pending,
    custom_unmixed: core::custom_unmixed,
    custom_in_tick: core::custom_in_tick,
    custom_clear_after: core::custom_clear_after,
    tick_unset: core::tick_unset,
    tick_short: core::tick_short,
    tick_stop: core::tick_stop,
    tick_copy: core::tick_copy,
    tick_volume: core::tick_volume,
    tick_clear_main: core::tick_clear_main,
};

/// Every 7-bit mask, alone and with every high bit set.
fn masks7() -> Vec<u32> {
    (0..128_u32)
        .chain((0..128_u32).map(|m| m | 0xffff_ff80))
        .collect()
}

const U64S: [u64; 30] = [
    0,
    1,
    2,
    3,
    0x3f,
    0x40,
    1000,
    1 << 31,
    (1 << 32) - 1,
    1 << 32,
    (1 << 32) + 1,
    21_333_333,
    1_000_000_000,
    1 << 53,
    (1 << 63) - 1,
    1 << 63,
    (1 << 63) + 1,
    u64::MAX - 2,
    u64::MAX - 1,
    u64::MAX,
    4095,
    4096,
    4097,
    8191,
    8192,
    32768,
    32769,
    0x5555_5555_5555_5555,
    0xaaaa_aaaa_aaaa_aaaa,
    0xdead_beef_cafe_f00d,
];

fn usizes() -> Vec<usize> {
    U64S.iter().map(|&v| v as usize).collect()
}

#[test]
fn parity_enum_values() {
    // SAFETY: plain values.
    unsafe {
        assert_eq!(oracle::oracle_source_audio_render_route_value(0), 0);
        assert_eq!(oracle::oracle_source_audio_render_route_value(1), 1);
        assert_eq!(oracle::oracle_source_audio_render_route_value(2), 2);
        assert_eq!(oracle::oracle_source_audio_render_route_size(), 4);
        assert_eq!(
            oracle::oracle_source_audio_render_route_value(0),
            shim::OBS_SOURCE_AUDIO_RENDER_PENDING
        );
        assert_eq!(
            oracle::oracle_source_audio_render_route_value(1),
            shim::OBS_SOURCE_AUDIO_RENDER_CUSTOM
        );
        assert_eq!(
            oracle::oracle_source_audio_render_route_value(2),
            shim::OBS_SOURCE_AUDIO_RENDER_TICK
        );
    }
    assert_eq!(route_u32(Route::Pending), 0);
    assert_eq!(route_u32(Route::Custom), 1);
    assert_eq!(route_u32(Route::Tick), 2);
}

#[test]
fn parity_route_every_input() {
    for bits in 0..8_u8 {
        let (b, c, d) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
        let o = (ORACLE.route)(b, c, d);
        assert_eq!((SHIM.route)(b, c, d), o, "{bits}");
        assert_eq!((CORE.route)(b, c, d), o, "{bits}");
    }
}

#[test]
fn parity_mask_decisions_exhaustive() {
    let masks = masks7();
    let mut calls = 0_u64;
    for &a in &masks {
        for mix in 0..DEFINED_MIXES {
            let o = (ORACLE.custom_in_tick)(a, mix);
            assert_eq!((SHIM.custom_in_tick)(a, mix), o);
            assert_eq!((CORE.custom_in_tick)(a, mix), o);
            let o = (ORACLE.custom_clear_after)(a, mix);
            assert_eq!((SHIM.custom_clear_after)(a, mix), o);
            assert_eq!((CORE.custom_clear_after)(a, mix), o);
        }
        for &m in &masks {
            let o = (ORACLE.tick_clear_main)(a, m);
            assert_eq!((SHIM.tick_clear_main)(a, m), o);
            assert_eq!((CORE.tick_clear_main)(a, m), o);
            for mix in 0..DEFINED_MIXES {
                let o = (ORACLE.custom_clear_before)(a, m, mix);
                assert_eq!(
                    (SHIM.custom_clear_before)(a, m, mix),
                    o,
                    "{a:#x} {m:#x} {mix}"
                );
                assert_eq!(
                    (CORE.custom_clear_before)(a, m, mix),
                    o,
                    "{a:#x} {m:#x} {mix}"
                );
                for submix in [false, true] {
                    let o = (ORACLE.tick_copy)(submix, a, m, mix);
                    assert_eq!((SHIM.tick_copy)(submix, a, m, mix), o);
                    assert_eq!((CORE.tick_copy)(submix, a, m, mix), o);
                }
                calls += 4;
            }
        }
    }
    assert_eq!(calls, 256 * 256 * 31 * 4);
}

#[test]
fn parity_submix_any_index() {
    // A submix source's copy does not shift and the stop only compares, so
    // the C is defined for every index.
    let indexes: Vec<usize> = (0..=(1 << 16))
        .chain([usize::MAX - 1, usize::MAX, 1 << 31, 1 << 32])
        .collect();
    for &mix in &indexes {
        for submix in [false, true] {
            let o = (ORACLE.tick_stop)(submix, mix);
            assert_eq!((SHIM.tick_stop)(submix, mix), o, "{submix} {mix}");
            assert_eq!((CORE.tick_stop)(submix, mix), o, "{submix} {mix}");
        }
        for a in [0, 1, 2, 3, 0xfe, u32::MAX] {
            for m in [0, 1, 2, u32::MAX] {
                let o = (ORACLE.tick_copy)(true, a, m, mix);
                assert_eq!((SHIM.tick_copy)(true, a, m, mix), o);
                assert_eq!((CORE.tick_copy)(true, a, m, mix), o);
            }
        }
    }
    for submix in [false, true] {
        let o = (ORACLE.tick_volume)(submix);
        assert_eq!((SHIM.tick_volume)(submix), o);
        assert_eq!((CORE.tick_volume)(submix), o);
    }
}

#[test]
fn parity_value_decisions_special() {
    let sizes = usizes();
    for &ts in &U64S {
        let o = (ORACLE.tick_unset)(ts);
        assert_eq!((SHIM.tick_unset)(ts), o);
        assert_eq!((CORE.tick_unset)(ts), o);
        for success in [false, true] {
            let o = (ORACLE.custom_ts)(success, ts);
            assert_eq!((SHIM.custom_ts)(success, ts), o);
            assert_eq!((CORE.custom_ts)(success, ts), o);
        }
        for m in masks7() {
            let o = (ORACLE.custom_unmixed)(ts, m);
            assert_eq!((SHIM.custom_unmixed)(ts, m), o);
            assert_eq!((CORE.custom_unmixed)(ts, m), o);
        }
    }
    for success in [false, true] {
        let o = (ORACLE.custom_pending)(success);
        assert_eq!((SHIM.custom_pending)(success), o);
        assert_eq!((CORE.custom_pending)(success), o);
    }
    for &input in &sizes {
        for &size in &sizes {
            let o = (ORACLE.tick_short)(input, size);
            assert_eq!((SHIM.tick_short)(input, size), o, "{input} {size}");
            assert_eq!((CORE.tick_short)(input, size), o, "{input} {size}");
        }
    }
}

// ---------------------------------------------------------------------------
// The callers, copied from obs-source.c onto a stand-in source

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Lock,
    Unlock,
    Peek(usize, usize),
    Memset(usize, usize),
    Memcpy(usize, usize),
    Render(u32, usize),
    Submix(usize),
    Volume(u32, usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StandIn {
    has_buffer: bool,
    custom: bool,
    has_data: bool,
    has_mix: bool,
    submix: bool,
    cb_success: bool,
    audio_ts: u64,
    audio_pending: bool,
    audio_mixers: u32,
    input_size: usize,
    cb_ts: u64,
    calls: Vec<Call>,
}

fn custom_audio_render(side: &Side, s: &mut StandIn, mixers: u32, channels: usize) {
    for mix in 0..MIXES {
        if (side.custom_clear_before)(s.audio_mixers, mixers, mix) {
            s.calls
                .push(Call::Memset(mix, TICK_BYTES.wrapping_mul(channels)));
        }
    }
    s.calls.push(Call::Render(mixers, channels));
    let success = s.cb_success;
    let ts = s.cb_ts;
    s.calls.push(Call::Lock);
    s.audio_ts = (side.custom_ts)(success, ts);
    s.audio_pending = (side.custom_pending)(success);
    if (side.custom_pending)(success) || (side.custom_unmixed)(s.audio_ts, mixers) {
        s.calls.push(Call::Unlock);
        return;
    }
    for mix in 0..MIXES {
        if !(side.custom_in_tick)(mixers, mix) {
            continue;
        }
        if (side.custom_clear_after)(s.audio_mixers, mix) {
            s.calls
                .push(Call::Memset(mix, TICK_BYTES.wrapping_mul(channels)));
        }
    }
    s.calls.push(Call::Volume(mixers, channels));
    s.calls.push(Call::Unlock);
}

fn process_audio_source_tick(
    side: &Side,
    s: &mut StandIn,
    mixers: u32,
    channels: usize,
    size: usize,
) {
    let audio_submix = s.submix;
    s.calls.push(Call::Lock);
    if (side.tick_unset)(s.audio_ts) || (side.tick_short)(s.input_size, size) {
        s.audio_pending = true;
        s.calls.push(Call::Unlock);
        return;
    }
    for ch in 0..channels {
        s.calls.push(Call::Peek(ch, size));
    }
    for mix in 1..MIXES {
        if (side.tick_stop)(audio_submix, mix) {
            break;
        }
        if !(side.tick_copy)(audio_submix, s.audio_mixers, mixers, mix) {
            s.calls.push(Call::Memset(mix, size.wrapping_mul(channels)));
            continue;
        }
        for ch in 0..channels {
            s.calls.push(Call::Memcpy(mix * CHANNELS + ch, size));
        }
    }
    if !(side.tick_volume)(audio_submix) {
        s.audio_pending = false;
        s.calls.push(Call::Unlock);
        return;
    }
    if (side.tick_clear_main)(s.audio_mixers, mixers) {
        s.calls.push(Call::Memset(0, size.wrapping_mul(channels)));
    }
    s.calls.push(Call::Volume(mixers, channels));
    s.audio_pending = false;
    s.calls.push(Call::Unlock);
}

fn audio_render(side: &Side, s: &mut StandIn, mixers: u32, channels: usize, size: usize) {
    let has_buffer = s.has_buffer;
    let custom = has_buffer && s.custom;
    let has_data = custom && s.has_data;
    match (side.route)(has_buffer, custom, has_data) {
        0 => {
            s.audio_pending = true;
            return;
        }
        1 => {
            custom_audio_render(side, s, mixers, channels);
            return;
        }
        _ => {}
    }
    if s.has_mix {
        s.calls.push(Call::Submix(channels));
    }
    process_audio_source_tick(side, s, mixers, channels, size);
}

#[allow(clippy::too_many_arguments)]
fn stand_in(bits: u8, audio_ts: u64, audio_mixers: u32, input_size: usize, cb_ts: u64) -> StandIn {
    StandIn {
        has_buffer: bits & 1 != 0,
        custom: bits & 2 != 0,
        has_data: bits & 4 != 0,
        has_mix: bits & 8 != 0,
        submix: bits & 16 != 0,
        cb_success: bits & 32 != 0,
        audio_ts,
        audio_pending: bits & 64 != 0,
        audio_mixers,
        input_size,
        cb_ts,
        calls: Vec::new(),
    }
}

#[track_caller]
fn compare_render(s: &StandIn, mixers: u32, channels: usize, size: usize) {
    let mut o = s.clone();
    audio_render(&ORACLE, &mut o, mixers, channels, size);
    let mut r = s.clone();
    audio_render(&SHIM, &mut r, mixers, channels, size);
    assert_eq!(
        r, o,
        "shim, mixers {mixers:#x} channels {channels} size {size}"
    );
    let mut c = s.clone();
    audio_render(&CORE, &mut c, mixers, channels, size);
    assert_eq!(
        c, o,
        "core, mixers {mixers:#x} channels {channels} size {size}"
    );
}

#[test]
fn parity_callers_every_flag_and_mask() {
    let mut runs = 0_u64;
    for bits in 0..128_u8 {
        for a in 0..128_u32 {
            for m in 0..128_u32 {
                let s = stand_in(bits, 1000, a, TICK_BYTES, 2000);
                compare_render(&s, m, 2, TICK_BYTES);
                runs += 1;
            }
        }
    }
    assert_eq!(runs, 128 * 128 * 128);
}

#[test]
fn parity_callers_edges() {
    let edges_ts = [0_u64, 1, u64::MAX];
    let edge_masks = [0_u32, 1, 0x3f, 0x40, 0x41, 0x8000_0000, u32::MAX];
    let edge_sizes = [
        (0_usize, 0_usize),
        (TICK_BYTES - 1, TICK_BYTES),
        (TICK_BYTES, TICK_BYTES),
        (TICK_BYTES + 1, TICK_BYTES),
        (usize::MAX, usize::MAX),
        (usize::MAX - 1, usize::MAX),
        (0, 1),
    ];
    for bits in 0..128_u8 {
        for ts in edges_ts {
            for cb_ts in edges_ts {
                for a in edge_masks {
                    for m in edge_masks {
                        for (input, size) in edge_sizes {
                            for channels in [0, 1, 2, CHANNELS] {
                                let s = stand_in(bits, ts, a, input, cb_ts);
                                compare_render(&s, m, channels, size);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn mask() -> impl Strategy<Value = u32> {
    prop_oneof![0..128_u32, any::<u32>(), Just(u32::MAX)]
}

fn timestamp() -> impl Strategy<Value = u64> {
    prop_oneof![Just(0_u64), Just(u64::MAX), any::<u64>()]
}

fn size() -> impl Strategy<Value = usize> {
    prop_oneof![
        Just(0_usize),
        Just(TICK_BYTES),
        Just(usize::MAX),
        any::<usize>()
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn parity_decisions_any_values(
        a in mask(),
        m in mask(),
        mix in 0..DEFINED_MIXES,
        any_mix: usize,
        ts in timestamp(),
        input in size(),
        want in size(),
        success: bool,
        submix: bool,
        bits in 0..8_u8,
    ) {
        let sides = [&SHIM, &CORE];
        let (b, c, d) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
        for side in sides {
            prop_assert_eq!((side.route)(b, c, d), (ORACLE.route)(b, c, d));
            prop_assert_eq!((side.custom_clear_before)(a, m, mix), (ORACLE.custom_clear_before)(a, m, mix));
            prop_assert_eq!((side.custom_ts)(success, ts), (ORACLE.custom_ts)(success, ts));
            prop_assert_eq!((side.custom_pending)(success), (ORACLE.custom_pending)(success));
            prop_assert_eq!((side.custom_unmixed)(ts, m), (ORACLE.custom_unmixed)(ts, m));
            prop_assert_eq!((side.custom_in_tick)(m, mix), (ORACLE.custom_in_tick)(m, mix));
            prop_assert_eq!((side.custom_clear_after)(a, mix), (ORACLE.custom_clear_after)(a, mix));
            prop_assert_eq!((side.tick_unset)(ts), (ORACLE.tick_unset)(ts));
            prop_assert_eq!((side.tick_short)(input, want), (ORACLE.tick_short)(input, want));
            prop_assert_eq!((side.tick_stop)(submix, any_mix), (ORACLE.tick_stop)(submix, any_mix));
            prop_assert_eq!((side.tick_copy)(submix, a, m, mix), (ORACLE.tick_copy)(submix, a, m, mix));
            prop_assert_eq!((side.tick_copy)(true, a, m, any_mix), (ORACLE.tick_copy)(true, a, m, any_mix));
            prop_assert_eq!((side.tick_volume)(submix), (ORACLE.tick_volume)(submix));
            prop_assert_eq!((side.tick_clear_main)(a, m), (ORACLE.tick_clear_main)(a, m));
        }
    }

    #[test]
    fn parity_callers_any_inputs(
        bits in 0..128_u8,
        ts in timestamp(),
        cb_ts in timestamp(),
        a in mask(),
        m in mask(),
        input in size(),
        want in size(),
        channels in 0..=CHANNELS,
    ) {
        let s = stand_in(bits, ts, a, input, cb_ts);
        compare_render(&s, m, channels, want);
    }
}
