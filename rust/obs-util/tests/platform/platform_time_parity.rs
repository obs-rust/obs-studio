//! Tier 3: the Rust `os_nstime_to_timespec` behaves exactly like the original
//! `libobs/util/platform-time.c`, compiled as `oracle_os_nstime_to_timespec`.
//!
//! The C base is one static, recorded on the first call. The oracle hook
//! `oracle_platform_time_set_base` replaces it, so both sides convert against
//! the same base:
//! - the safe core with an injected base, against the oracle seeded with that
//!   base, for random bases and timestamps;
//! - the C ABI shim with the process-wide Rust base, against the oracle
//!   seeded with that same base.
//!
//! The oracle's base is shared by every test here, so each seed-and-convert
//! holds [`ORACLE`].
//!
//! Not compared: the C's NULL return when `timespec_get` fails, which the
//! Rust wall clock cannot do. Bases keep `|sec| < 2^62`, where the C's
//! signed `tv_sec` arithmetic below the base cannot overflow (that would be
//! undefined behavior in C).
//!
//! Reminder: the mutation check (break the core, see this file fail) is done
//! once per port, not on every change.

use core::ffi::{c_long, c_void};
use std::sync::Mutex;

use obs_c_oracle::platform_time as c;
use obs_util::ffi::platform_time::{self as rs, timespec};
use obs_util::platform_time::{Base, NSEC_PER_SEC, Timespec, base, nstime_to_timespec};
use proptest::prelude::*;

static ORACLE: Mutex<()> = Mutex::new(());

const NS: u64 = NSEC_PER_SEC as u64;

/// Seeds the oracle with `base`, then converts each timestamp through it.
fn oracle(base: Base, timestamps: &[u64]) -> Vec<Option<timespec>> {
    let _guard = ORACLE.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: the hook takes plain values; ORACLE serializes the static.
    unsafe {
        c::oracle_platform_time_set_base(base.wall.sec, base.wall.nsec as c_long, base.timestamp)
    };
    timestamps
        .iter()
        .map(|&t| {
            let mut ts = timespec {
                tv_sec: -1,
                tv_nsec: -1,
            };
            let storage: *mut timespec = &mut ts;
            // SAFETY: `storage` is a writable `struct timespec`.
            let ret = unsafe { c::oracle_os_nstime_to_timespec(t, storage.cast::<c_void>()) };
            if ret.is_null() {
                None
            } else {
                assert_eq!(ret, storage.cast::<c_void>());
                Some(ts)
            }
        })
        .collect()
}

fn as_c(ts: Timespec) -> timespec {
    timespec {
        tv_sec: ts.sec as libc::time_t,
        tv_nsec: ts.nsec as c_long,
    }
}

/// The safe core and the oracle agree for `base` on every timestamp.
fn core_same(base: Base, timestamps: &[u64]) {
    let want = oracle(base, timestamps);
    for (&t, c_ts) in timestamps.iter().zip(want) {
        let got = nstime_to_timespec(base, t);
        assert_eq!(Some(as_c(got)), c_ts, "base {base:?}, timestamp {t}");
    }
}

fn at(sec: i64, nsec: i64, timestamp: u64) -> Base {
    Base {
        wall: Timespec { sec, nsec },
        timestamp,
    }
}

#[test]
fn fixed_bases() {
    let ts = [
        0,
        1,
        NS - 1,
        NS,
        10 * NS - 600_000_001,
        10 * NS - 600_000_000,
        10 * NS - 1,
        10 * NS,
        10 * NS + 1,
        10 * NS + 399_999_999,
        10 * NS + 400_000_000,
        10 * NS + 400_000_001,
        11 * NS,
        u64::MAX - 1,
        u64::MAX,
    ];
    for b in [
        at(100, 600_000_000, 10 * NS),
        at(100, 0, 10 * NS),
        at(100, 999_999_999, 10 * NS),
        at(-5, 250_000_000, 10 * NS),
        at(1_700_000_000, 123_456_789, 0),
        at(1_700_000_000, 123_456_789, u64::MAX),
    ] {
        core_same(b, &ts);
    }
}

#[test]
fn whole_second_edge() {
    // characterized, not endorsed: both sides return tv_nsec == 1000000000
    let b = at(100, 600_000_000, 10 * NS);
    core_same(b, &[10 * NS + 400_000_000, 17 * NS + 400_000_000]);
    let c_ts = oracle(b, &[10 * NS + 400_000_000]);
    assert_eq!(
        c_ts,
        vec![Some(timespec {
            tv_sec: 100,
            tv_nsec: 1_000_000_000
        })]
    );
}

/// The shim, with the process-wide Rust base, against the oracle seeded with
/// that base.
fn shim_same(timestamps: &[u64]) {
    let b = base();
    let want = oracle(b, timestamps);
    for (&t, c_ts) in timestamps.iter().zip(want) {
        let mut ts = timespec {
            tv_sec: -1,
            tv_nsec: -1,
        };
        // SAFETY: `ts` is a writable `struct timespec`.
        let ret = unsafe { rs::os_nstime_to_timespec(t, &mut ts) };
        assert_eq!(ret, &mut ts as *mut timespec);
        assert_eq!(Some(ts), c_ts, "timestamp {t}");
    }
}

#[test]
fn shim_matches_c() {
    let b = base();
    let mut ts = vec![0, 1, b.timestamp, u64::MAX, u64::MAX - NS];
    if b.timestamp > 0 {
        ts.push(b.timestamp - 1);
    }
    for k in 0..64u64 {
        ts.push(b.timestamp.wrapping_add(k * 123_456_789));
        ts.push(b.timestamp.saturating_sub(k * 98_765_431));
    }
    // the timestamps whose result lands on a whole second, above and below
    let to_second = (NSEC_PER_SEC - b.wall.nsec) as u64;
    ts.push(b.timestamp + to_second);
    ts.push(b.timestamp + to_second + 5 * NS);
    if b.timestamp > b.wall.nsec as u64 {
        ts.push(b.timestamp - b.wall.nsec as u64);
    }
    shim_same(&ts);
}

#[test]
fn null_storage_returns_null() {
    // SAFETY: the shim accepts NULL.
    assert!(unsafe { rs::os_nstime_to_timespec(0, core::ptr::null_mut()) }.is_null());
    // SAFETY: the shim accepts NULL.
    assert!(unsafe { rs::os_nstime_to_timespec(u64::MAX, core::ptr::null_mut()) }.is_null());

    let _guard = ORACLE.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: the oracle accepts NULL; ORACLE serializes the static.
    assert!(unsafe { c::oracle_os_nstime_to_timespec(0, core::ptr::null_mut()) }.is_null());
}

/// A base like a real one or anywhere in range, with normalized nanoseconds.
fn any_base() -> impl Strategy<Value = Base> {
    let sec = prop_oneof![
        1_000_000_000i64..4_000_000_000,
        -(1i64 << 62) + 1..(1i64 << 62),
    ];
    (sec, 0..NSEC_PER_SEC, any::<u64>()).prop_map(|(sec, nsec, timestamp)| at(sec, nsec, timestamp))
}

/// Timestamps anywhere, near the base, or on the whole-second edge.
fn timestamps(base: Base) -> impl Strategy<Value = Vec<u64>> {
    let near = (-(3 * NSEC_PER_SEC)..3 * NSEC_PER_SEC)
        .prop_map(move |d| base.timestamp.wrapping_add_signed(d));
    let edge = (0..1000u64).prop_map(move |s| {
        base.timestamp
            .wrapping_add((NSEC_PER_SEC - base.wall.nsec) as u64)
            .wrapping_add(s * NS)
    });
    prop::collection::vec(prop_oneof![any::<u64>(), near, edge], 1..16)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn random_bases_match(
        (b, ts) in any_base().prop_flat_map(|b| (Just(b), timestamps(b)))
    ) {
        core_same(b, &ts);
    }

    #[test]
    fn random_timestamps_match_through_the_shim(ts in prop::collection::vec(any::<u64>(), 1..16)) {
        shim_same(&ts);
    }
}
