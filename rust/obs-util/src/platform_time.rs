//! Safe core for `os_nstime_to_timespec` from `libobs/util/platform-time.c`
//! (obs-rust/obs-studio#262 slice 6): an `os_gettime_ns` timestamp as wall
//! clock time, from a base pair recorded on the first call.

use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::platform;

/// Nanoseconds per second.
pub const NSEC_PER_SEC: i64 = 1_000_000_000;

/// A `struct timespec` value at full width. The C ABI form, whose `tv_nsec`
/// is a C `long`, is [`crate::ffi::platform_time::timespec`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timespec {
    /// `tv_sec`.
    pub sec: i64,
    /// `tv_nsec`.
    pub nsec: i64,
}

/// The pair the first call records: the wall clock (`timespec_get` with
/// `TIME_UTC`), then `os_gettime_ns()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Base {
    /// The wall clock time.
    pub wall: Timespec,
    /// The `os_gettime_ns()` timestamp read right after it.
    pub timestamp: u64,
}

impl Base {
    /// Reads the wall clock, then the monotonic clock, in the C's order.
    pub fn now() -> Self {
        let wall = wall_now();
        Self {
            wall,
            timestamp: platform::gettime_ns(),
        }
    }
}

/// A base recorded once, on the first [`BaseOnce::get`]: the C's
/// `pthread_once(&once, init_timespec_offset)`. A `static` one is set by the
/// first conversion, not when the library loads.
#[derive(Debug, Default)]
pub struct BaseOnce(OnceLock<Base>);

impl BaseOnce {
    /// An empty cell.
    pub const fn new() -> Self {
        Self(OnceLock::new())
    }

    /// The base, recorded now if this is the first call.
    pub fn get(&self) -> Base {
        *self.0.get_or_init(Base::now)
    }
}

static BASE: BaseOnce = BaseOnce::new();

/// The process-wide base that `os_nstime_to_timespec` uses.
pub fn base() -> Base {
    BASE.get()
}

/// `os_nstime_to_timespec(timestamp, storage)` against the process-wide base.
pub fn os_nstime_to_timespec(timestamp: u64) -> Timespec {
    nstime_to_timespec(base(), timestamp)
}

/// `os_nstime_to_timespec` for a given base: the base's wall time plus the
/// distance of `timestamp` from the base's timestamp.
///
/// A timestamp at or above the base adds the nanoseconds and carries when
/// their sum reaches [`NSEC_PER_SEC`]; below the base it borrows. Either way
/// the result is normalized (`0 <= nsec < NSEC_PER_SEC`, #1505). The C does
/// this arithmetic in `uint64_t` above the base, which wraps; so does this.
pub fn nstime_to_timespec(base: Base, timestamp: u64) -> Timespec {
    const NS: u64 = NSEC_PER_SEC as u64;

    let Timespec {
        sec: mut tv_sec,
        nsec: mut tv_nsec,
    } = base.wall;
    let (mut secs, mut nsecs);
    if timestamp >= base.timestamp {
        let offset = timestamp - base.timestamp;
        nsecs = tv_nsec.wrapping_add((offset % NS) as i64);
        secs = tv_sec.wrapping_add((offset / NS) as i64);
    } else {
        let offset = base.timestamp - timestamp;
        let nsec_offset = (offset % NS) as i64;
        let sec_offset = (offset / NS) as i64;
        if nsec_offset > tv_nsec {
            tv_sec = tv_sec.wrapping_sub(1);
            tv_nsec = tv_nsec.wrapping_add(NSEC_PER_SEC);
        }
        nsecs = tv_nsec.wrapping_sub(nsec_offset);
        secs = tv_sec.wrapping_sub(sec_offset);
    }

    if nsecs >= NSEC_PER_SEC {
        nsecs -= NSEC_PER_SEC;
        secs = secs.wrapping_add(1);
    }
    Timespec {
        sec: secs,
        nsec: nsecs,
    }
}

/// `timespec_get(&ts, TIME_UTC)`: seconds and nanoseconds since the Unix
/// epoch, with `0 <= nsec < NSEC_PER_SEC` before 1970 too.
fn wall_now() -> Timespec {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(since) => Timespec {
            sec: since.as_secs() as i64,
            nsec: i64::from(since.subsec_nanos()),
        },
        Err(before) => {
            let before = before.duration();
            let mut sec = -(before.as_secs() as i64);
            let mut nsec = i64::from(before.subsec_nanos());
            if nsec > 0 {
                sec -= 1;
                nsec = NSEC_PER_SEC - nsec;
            }
            Timespec { sec, nsec }
        }
    }
}
