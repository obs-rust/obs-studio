//! Tier 2 layout and Tier 3 parity: the shims of
//! `obs_output::ffi::encoder_packet_ts` (and the safe core under them)
//! against the original `libobs/obs-encoder-packet-ts.c.h`, compiled as
//! `oracle_obs_encoder_packet_ts_*` (obs-c-oracle,
//! oracle/encoder_packet_ts.c).
//!
//! Every call runs the shim, the C and the safe core, and compares the
//! results exactly.
//!
//! Runs: the layout of `struct obs_encoder_packet_timebase`, the part of
//! `struct encoder_packet_time` the walk reads, and the "not found" value;
//! exhaustive over timebases, DTS values, start times, offsets and pause
//! offsets at 0, 1, common rates and the integer edges; every list of up to
//! 6 entries over 3 PTS values; a realistic stream at 25, 29.97, 30 and 60
//! fps and 48 kHz with B-frames and a pause; and proptests over the full
//! ranges.
//!
//! The C is undefined in `obs_encoder_packet_ts_dts_usec()` for a
//! `timebase_den` of 0, `dts * 1000000` beyond `int64_t` and
//! `INT64_MIN / -1`. Those inputs are not given to the C; there the shim is
//! compared with the core, which gives the intentional differences (0, the
//! wrapped product, `i64::MIN`). `test_encoder_packet_ts_steps` runs the C
//! test against the C; the real-libobs encoder tests run against the hybrid
//! libobs, where these shims replace the C.

use std::mem::{align_of, offset_of, size_of};

use obs_c_oracle::encoder_packet_ts::{
    LAYOUT_NAMES, OraclePacketTime, oracle_encoder_packet_ts_layout,
    oracle_obs_encoder_packet_ts_dts_usec, oracle_obs_encoder_packet_ts_find,
    oracle_obs_encoder_packet_ts_paused, oracle_obs_encoder_packet_ts_system,
    oracle_obs_encoder_packet_ts_timebase,
};
use obs_output::encoder_packet_ts::{self as ts, MICROSECOND_DEN, NOT_FOUND};
use obs_output::ffi::encoder_packet_ts::{
    obs_encoder_packet_timebase, obs_encoder_packet_ts_dts_usec, obs_encoder_packet_ts_find,
    obs_encoder_packet_ts_paused, obs_encoder_packet_ts_system, obs_encoder_packet_ts_timebase,
};
use obs_output::ffi::output_delay::encoder_packet_time;
use proptest::prelude::*;

const U32S: [u32; 12] = [
    0,
    1,
    2,
    3,
    25,
    30,
    1001,
    30000,
    48000,
    65536,
    0x8000_0000,
    u32::MAX,
];
const I32S: [i32; 13] = [
    i32::MIN,
    -48000,
    -30000,
    -2,
    -1,
    0,
    1,
    2,
    25,
    30,
    30000,
    48000,
    i32::MAX,
];
const I64S: [i64; 15] = [
    i64::MIN,
    i64::MIN / MICROSECOND_DEN,
    -66733,
    -2002,
    -1001,
    -1,
    0,
    1,
    1001,
    21333,
    5_000_000,
    1_000_000_000_000,
    i64::MAX / MICROSECOND_DEN,
    i64::MAX / 2,
    i64::MAX,
];
const U64S: [u64; 11] = [
    0,
    1,
    999,
    1000,
    1001,
    2_500_000_000,
    5_000_000_123,
    1_000_000_000_000_000,
    0x7fff_ffff_ffff_ffff,
    u64::MAX - 999,
    u64::MAX,
];

// ---------------------------------------------------------------------------
// one call on all three sides

fn check_timebase(num: u32, den: u32, divisor: u32) {
    let shim = obs_encoder_packet_ts_timebase(num, den, divisor);
    // SAFETY: plain values in, a struct out; the oracle keeps no state.
    let c = unsafe { oracle_obs_encoder_packet_ts_timebase(num, den, divisor) };
    let core = ts::timebase(num, den, divisor);
    let what = format!("num {num} den {den} divisor {divisor}");
    assert_eq!((shim.num, shim.den), (c.num, c.den), "shim vs C: {what}");
    assert_eq!(
        (shim.num, shim.den),
        (core.num, core.den),
        "shim vs core: {what}"
    );
}

/// Where C defines `dts * 1000000 / timebase_den`.
fn c_defined(dts: i64, den: i32) -> bool {
    den != 0
        && dts
            .checked_mul(MICROSECOND_DEN)
            .is_some_and(|p| !(p == i64::MIN && den == -1))
}

fn check_dts_usec(dts: i64, den: i32) {
    let shim = obs_encoder_packet_ts_dts_usec(dts, den);
    let core = ts::dts_usec(dts, den);
    assert_eq!(shim, core, "shim vs core: dts {dts} den {den}");
    if c_defined(dts, den) {
        // SAFETY: plain values; C is defined for them (checked above).
        let c = unsafe { oracle_obs_encoder_packet_ts_dts_usec(dts, den) };
        assert_eq!(shim, c, "shim vs C: dts {dts} den {den}");
    }
}

fn check_system(start: u64, dts_usec: i64, offset: i64) {
    let shim = obs_encoder_packet_ts_system(start, dts_usec, offset);
    // SAFETY: plain values; unsigned arithmetic, defined for every input.
    let c = unsafe { oracle_obs_encoder_packet_ts_system(start, dts_usec, offset) };
    let core = ts::system(start, dts_usec, offset);
    let what = format!("start {start} dts_usec {dts_usec} offset {offset}");
    assert_eq!(shim, c, "shim vs C: {what}");
    assert_eq!(shim, core, "shim vs core: {what}");
}

fn check_paused(sys: i64, offset: u64) {
    let shim = obs_encoder_packet_ts_paused(sys, offset);
    // SAFETY: plain values; unsigned arithmetic, defined for every input.
    let c = unsafe { oracle_obs_encoder_packet_ts_paused(sys, offset) };
    let core = ts::paused(sys, offset);
    assert_eq!(shim, c, "shim vs C: sys {sys} offset {offset}");
    assert_eq!(shim, core, "shim vs core: sys {sys} offset {offset}");
}

fn check_find(list: &[i64], pts: i64) {
    let shim_times: Vec<encoder_packet_time> = list
        .iter()
        .enumerate()
        .map(|(i, &pts)| encoder_packet_time {
            pts,
            cts: u64::try_from(i).unwrap_or(u64::MAX),
            ..encoder_packet_time::default()
        })
        .collect();
    let c_times: Vec<OraclePacketTime> = shim_times
        .iter()
        .map(|t| OraclePacketTime {
            pts: t.pts,
            cts: t.cts,
            fer: t.fer,
            ferc: t.ferc,
            pir: t.pir,
        })
        .collect();
    let ptr = if list.is_empty() {
        std::ptr::null()
    } else {
        shim_times.as_ptr()
    };
    // SAFETY: `list.len()` initialized entries (or NULL with 0), read only.
    let shim = unsafe { obs_encoder_packet_ts_find(ptr, list.len(), pts) };
    let c_ptr = if list.is_empty() {
        std::ptr::null()
    } else {
        c_times.as_ptr()
    };
    // SAFETY: the same, for the oracle's mirror.
    let c = unsafe { oracle_obs_encoder_packet_ts_find(c_ptr, list.len(), pts) };
    let core = ts::find(&shim_times, pts).unwrap_or(NOT_FOUND);
    assert_eq!(shim, c, "shim vs C: {list:?} pts {pts}");
    assert_eq!(shim, core, "shim vs core: {list:?} pts {pts}");
}

// ---------------------------------------------------------------------------
// layout

#[test]
fn parity_layout() {
    let rust = [
        size_of::<obs_encoder_packet_timebase>(),
        align_of::<obs_encoder_packet_timebase>(),
        offset_of!(obs_encoder_packet_timebase, num),
        offset_of!(obs_encoder_packet_timebase, den),
        size_of::<encoder_packet_time>(),
        align_of::<encoder_packet_time>(),
        offset_of!(encoder_packet_time, pts),
        offset_of!(encoder_packet_time, cts),
        NOT_FOUND,
    ];
    for (i, (name, want)) in LAYOUT_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: an index into the oracle's table; out of range gives -1.
        let c = unsafe { oracle_encoder_packet_ts_layout(i) };
        assert_eq!(want, c, "{name}");
    }
    // SAFETY: as above, one past the end.
    assert_eq!(
        unsafe { oracle_encoder_packet_ts_layout(LAYOUT_NAMES.len()) },
        usize::MAX
    );
}

// ---------------------------------------------------------------------------
// exhaustive over the edges

#[test]
fn parity_timebase_exhaustive() {
    for num in U32S {
        for den in U32S {
            for divisor in U32S {
                check_timebase(num, den, divisor);
            }
        }
    }
}

#[test]
fn parity_dts_usec_exhaustive() {
    for dts in I64S {
        for den in I32S {
            check_dts_usec(dts, den);
        }
    }
}

#[test]
fn parity_dts_usec_where_c_is_undefined() {
    // Compared with the core only: the intentional differences.
    assert_eq!(obs_encoder_packet_ts_dts_usec(1001, 0), 0);
    assert_eq!(
        obs_encoder_packet_ts_dts_usec(i64::MAX, 1),
        i64::MAX.wrapping_mul(MICROSECOND_DEN)
    );
    assert_eq!(
        obs_encoder_packet_ts_dts_usec(144_115_188_075_855_872, -1),
        i64::MIN
    );
}

#[test]
fn parity_system_and_paused_exhaustive() {
    for start in U64S {
        for dts in I64S {
            for offset in I64S {
                check_system(start, dts, offset);
            }
        }
    }
    for sys in I64S {
        for offset in U64S {
            check_paused(sys, offset);
        }
    }
}

#[test]
fn parity_find_every_short_list() {
    // Every list of up to 6 entries over the PTS values 0..3, for each PTS
    // in -1..4.
    for len in 0..=6_u32 {
        for code in 0..3_u32.pow(len) {
            let mut list = Vec::new();
            let mut c = code;
            for _ in 0..len {
                list.push(i64::from(c % 3));
                c /= 3;
            }
            for pts in -1..4 {
                check_find(&list, pts);
            }
        }
    }
}

/// A stream frame by frame: B-frames (the first DTS below 0), the first
/// packet's offset, each packet on the system clock, a pause, and the
/// timing entries matched and removed as obs-encoder.c does.
#[test]
fn parity_stream() {
    for (num, den, divisor) in [
        (1_u32, 25_u32, 1_u32),
        (1001, 30000, 1),
        (1, 30, 1),
        (1, 60, 2),
        (1, 48000, 1),
    ] {
        check_timebase(num, den, divisor);
        let tb = ts::timebase(num, den, divisor);
        let step = i64::from(tb.num);
        let first = -2 * step;
        let offset = ts::dts_usec(first, tb.den);
        check_dts_usec(first, tb.den);
        let mut pending: Vec<i64> = Vec::new();
        for frame in 0..600_i64 {
            let dts = first + frame * step;
            let pts = frame * step;
            pending.push(pts);
            check_dts_usec(dts, tb.den);
            let usec = ts::dts_usec(dts, tb.den);
            check_system(5_000_000_123, usec, offset);
            let sys = ts::system(5_000_000_123, usec, offset);
            check_paused(sys, u64::try_from(frame).unwrap_or(0) * 7_000_000);
            // The packet for the frame two back arrives now (B-frame delay).
            if frame >= 2 {
                let arrived = (frame - 2) * step;
                check_find(&pending, arrived);
                if let Some(i) = ts::find(
                    &pending
                        .iter()
                        .map(|&p| encoder_packet_time {
                            pts: p,
                            ..encoder_packet_time::default()
                        })
                        .collect::<Vec<_>>(),
                    arrived,
                ) {
                    pending.remove(i);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8192))]

    #[test]
    fn parity_timebase_any(num in any::<u32>(), den in any::<u32>(), divisor in any::<u32>()) {
        check_timebase(num, den, divisor);
    }

    #[test]
    fn parity_dts_usec_any(
        dts in prop_oneof![any::<i64>(), i64::MIN / MICROSECOND_DEN..=i64::MAX / MICROSECOND_DEN],
        den in any::<i32>(),
    ) {
        check_dts_usec(dts, den);
    }

    #[test]
    fn parity_system_any(start in any::<u64>(), dts in any::<i64>(), offset in any::<i64>()) {
        check_system(start, dts, offset);
    }

    #[test]
    fn parity_paused_any(sys in any::<i64>(), offset in any::<u64>()) {
        check_paused(sys, offset);
    }

    #[test]
    fn parity_find_any(list in prop::collection::vec(prop_oneof![any::<i64>(), -2_i64..3], 0..64), pts in prop_oneof![any::<i64>(), -2_i64..3]) {
        check_find(&list, pts);
    }
}
