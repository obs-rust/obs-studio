//! Tier 1: 1:1 port of `test/cmocka/test_platform_time.c` against the safe
//! core. Each test names the cmocka case it mirrors.
//!
//! The cmocka test's process-wide base becomes a fresh [`BaseOnce`] per test,
//! set up the way its `group_setup` does: the first conversion is of a
//! timestamp read just before it, which is at or below the base, so the
//! result gives `K = wall - base timestamp` exactly, and every check predicts
//! its result from `K`.

use std::time::{SystemTime, UNIX_EPOCH};

use obs_c_oracle as _; // links the test allocator
use obs_util::platform;
use obs_util::platform_time::{
    Base, BaseOnce, NSEC_PER_SEC, Timespec, base, nstime_to_timespec, os_nstime_to_timespec,
};

const NS: u64 = NSEC_PER_SEC as u64;

fn wall_ns() -> i128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970")
        .as_nanos() as i128
}

/// `group_setup`: the first conversion on a fresh base, bracketed by the
/// clocks.
struct Fixture {
    once: BaseOnce,
    mono_before: u64,
    mono_after: u64,
    wall_before: i128,
    wall_after: i128,
    first: Timespec,
    /// `K` as (seconds, nanoseconds), `0 <= nanoseconds < NSEC_PER_SEC`.
    k: (i64, i64),
}

impl Fixture {
    fn new() -> Self {
        let once = BaseOnce::new();
        let wall_before = wall_ns();
        let mono_before = platform::gettime_ns();
        let first = nstime_to_timespec(once.get(), mono_before);
        let mono_after = platform::gettime_ns();
        let wall_after = wall_ns();

        let mut k = (
            first.sec - (mono_before / NS) as i64,
            first.nsec - (mono_before % NS) as i64,
        );
        if k.1 < 0 {
            k.1 += NSEC_PER_SEC;
            k.0 -= 1;
        }
        Self {
            once,
            mono_before,
            mono_after,
            wall_before,
            wall_after,
            first,
            k,
        }
    }

    fn convert(&self, timestamp: u64) -> Timespec {
        nstime_to_timespec(self.once.get(), timestamp)
    }

    /// `K + timestamp`, normalized.
    fn expected(&self, timestamp: u64) -> Timespec {
        let mut out = Timespec {
            sec: self.k.0 + (timestamp / NS) as i64,
            nsec: self.k.1 + (timestamp % NS) as i64,
        };
        if out.nsec >= NSEC_PER_SEC {
            out.nsec -= NSEC_PER_SEC;
            out.sec += 1;
        }
        out
    }

    /// `check`: above the base, a whole second is the previous second plus
    /// `NSEC_PER_SEC`; below it, the result is normalized.
    fn check(&self, timestamp: u64, above: bool) {
        let want = self.expected(timestamp);
        let want = if above && want.nsec == 0 {
            Timespec {
                sec: want.sec - 1,
                nsec: NSEC_PER_SEC,
            }
        } else {
            want
        };
        assert_eq!(self.convert(timestamp), want, "timestamp {timestamp}");
    }

    /// `with_nsec`: the smallest timestamp >= `from` whose result has `nsec`.
    fn with_nsec(&self, from: u64, nsec: i64) -> u64 {
        let now = self.expected(from).nsec;
        from + ((nsec - now + NSEC_PER_SEC) % NSEC_PER_SEC) as u64
    }
}

/// `test_first_call_sets_the_base`
#[test]
fn first_call_sets_the_base() {
    let f = Fixture::new();

    assert!((0..NSEC_PER_SEC).contains(&f.first.nsec));

    let first_ns = i128::from(f.first.sec) * i128::from(NSEC_PER_SEC) + i128::from(f.first.nsec);
    let lo = f.wall_before - i128::from(f.mono_after - f.mono_before);
    assert!(first_ns >= lo, "{first_ns} < {lo}");
    assert!(first_ns <= f.wall_after, "{first_ns} > {}", f.wall_after);

    assert_eq!(f.convert(f.mono_before), f.first);
}

/// `test_null_storage`: a NULL storage is the shim's, so this checks the
/// part the safe core has: reading the base again does not change it.
#[test]
fn null_storage() {
    let f = Fixture::new();
    let once_base = f.once.get();
    assert_eq!(f.once.get(), once_base);
    f.check(f.mono_before, false);
}

/// `test_storage_contents_are_ignored`: the safe core has no storage; the
/// same timestamp gives the same result.
#[test]
fn storage_contents_are_ignored() {
    let f = Fixture::new();
    let t = f.with_nsec(f.mono_after, 123_456_789);

    let a = f.convert(t);
    assert_eq!(f.convert(t), a);
    assert_eq!(
        a,
        Timespec {
            sec: f.expected(t).sec,
            nsec: 123_456_789
        }
    );
}

/// `test_around_the_base`
#[test]
fn around_the_base() {
    let f = Fixture::new();
    let end = f.mono_after.min(f.mono_before + 100_000);
    for t in f.mono_before..=end {
        let want = f.expected(t);
        let got = f.convert(t);
        if want.nsec == 0 && got.nsec == NSEC_PER_SEC {
            assert_eq!(got.sec, want.sec - 1, "timestamp {t}");
        } else {
            assert_eq!(got, want, "timestamp {t}");
        }
    }
}

/// `test_above_the_base`
#[test]
fn above_the_base() {
    let f = Fixture::new();
    let t = f.with_nsec(f.mono_after, 500_000_000);

    f.check(t, true);
    f.check(t + 1, true);
    f.check(t + 499_999_999, true);
    f.check(t + 600_000_000, true);
    f.check(t + 999_999_999, true);

    f.check(t + NS, true);
    f.check(t + 5 * NS, true);
    f.check(t + 86_400 * NS, true);

    let a = f.convert(t);
    let b = f.convert(t + 3 * NS);
    assert_eq!(
        b,
        Timespec {
            sec: a.sec + 3,
            nsec: a.nsec
        }
    );

    for i in 0..64u64 {
        f.check(f.mono_after + i * 123_456_789, true);
    }
}

/// `test_nanosecond_carry_above_the_base`
#[test]
fn nanosecond_carry_above_the_base() {
    let f = Fixture::new();

    let t = f.with_nsec(f.mono_after, 1);
    assert_eq!(
        f.convert(t),
        Timespec {
            sec: f.expected(t).sec,
            nsec: 1
        }
    );

    let t = f.with_nsec(f.mono_after, 999_999_999);
    assert_eq!(
        f.convert(t),
        Timespec {
            sec: f.expected(t).sec,
            nsec: 999_999_999
        }
    );
}

/// `test_whole_second_above_the_base`. Characterized, not endorsed: a sum
/// of exactly one second is not carried.
#[test]
fn whole_second_above_the_base() {
    let f = Fixture::new();
    let t = f.with_nsec(f.mono_after, 0);

    assert_eq!(
        f.convert(t),
        Timespec {
            sec: f.expected(t).sec - 1,
            nsec: NSEC_PER_SEC
        }
    );
    assert_eq!(
        f.convert(t + 7 * NS),
        Timespec {
            sec: f.expected(t).sec + 6,
            nsec: NSEC_PER_SEC
        }
    );

    f.check(t - 1, true);
    f.check(t + 1, true);
}

/// `test_below_the_base`
#[test]
fn below_the_base() {
    let f = Fixture::new();

    f.check(0, false);
    f.check(1, false);
    f.check(999_999_999, false);
    f.check(NS, false);
    f.check(f.mono_before - 1, false);
    if f.mono_before > 3 * NS {
        f.check(f.mono_before - NS, false);
        f.check(f.with_nsec(f.mono_before - 3 * NS, 0), false);
    }

    for i in 1..=64u64 {
        let offset = i * 98_765_431;
        if offset <= f.mono_before {
            f.check(f.mono_before - offset, false);
        }
    }
}

/// `test_large_timestamps`
#[test]
fn large_timestamps() {
    let f = Fixture::new();

    f.check(u64::MAX, true);
    f.check(u64::MAX - 1, true);
    f.check(u64::MAX - NS, true);
    f.check(1 << 63, true);
    f.check((1 << 63) - 1, true);

    assert!(f.convert(u64::MAX).sec > f.first.sec + 18_000_000_000);
}

fn at(sec: i64, nsec: i64, timestamp: u64) -> Base {
    Base {
        wall: Timespec { sec, nsec },
        timestamp,
    }
}

/// New edge case: the timestamp equal to the base gives the base's wall time.
#[test]
fn equal_to_the_base_is_the_wall_time() {
    for nsec in [0, 1, 500_000_000, 999_999_999] {
        let b = at(1_700_000_000, nsec, 5 * NS);
        assert_eq!(nstime_to_timespec(b, 5 * NS), b.wall);
    }
}

/// New edge case: with a base on a whole second (which the cmocka test
/// cannot rule out), nothing above the base hits the uncarried second.
#[test]
fn whole_second_base_always_normalizes() {
    let b = at(100, 0, 10 * NS);
    assert_eq!(
        nstime_to_timespec(b, 10 * NS + 3 * NS),
        Timespec { sec: 103, nsec: 0 }
    );
    assert_eq!(
        nstime_to_timespec(b, 10 * NS + 999_999_999),
        Timespec {
            sec: 100,
            nsec: 999_999_999
        }
    );
}

/// New edge case: the exact sum, the carry one past it, and the borrow.
#[test]
fn carry_and_borrow_with_a_fixed_base() {
    let b = at(100, 600_000_000, 10 * NS);
    // characterized, not endorsed
    assert_eq!(
        nstime_to_timespec(b, 10 * NS + 400_000_000),
        Timespec {
            sec: 100,
            nsec: NSEC_PER_SEC
        }
    );
    assert_eq!(
        nstime_to_timespec(b, 10 * NS + 400_000_001),
        Timespec { sec: 101, nsec: 1 }
    );
    // below: borrow when the offset's nanoseconds pass the base's
    assert_eq!(
        nstime_to_timespec(b, 10 * NS - 600_000_001),
        Timespec {
            sec: 99,
            nsec: 999_999_999
        }
    );
    assert_eq!(
        nstime_to_timespec(b, 10 * NS - 600_000_000),
        Timespec { sec: 100, nsec: 0 }
    );
    assert_eq!(
        nstime_to_timespec(b, 10 * NS - 2 * NS - 1),
        Timespec {
            sec: 98,
            nsec: 599_999_999
        }
    );
    assert_eq!(
        nstime_to_timespec(b, 0),
        Timespec {
            sec: 90,
            nsec: 600_000_000
        }
    );
}

/// New edge case: a wall clock before 1970 (negative seconds).
#[test]
fn negative_wall_seconds() {
    let b = at(-5, 250_000_000, NS);
    assert_eq!(
        nstime_to_timespec(b, NS + 750_000_000),
        Timespec {
            sec: -5,
            nsec: NSEC_PER_SEC
        }
    );
    assert_eq!(
        nstime_to_timespec(b, 0),
        Timespec {
            sec: -6,
            nsec: 250_000_000
        }
    );
}

/// New edge case: the process-wide base is set once; every later call uses it.
#[test]
fn process_base_is_stable() {
    let first = base();
    let t = platform::gettime_ns();
    assert_eq!(base(), first);
    assert_eq!(os_nstime_to_timespec(t), nstime_to_timespec(first, t));
}
