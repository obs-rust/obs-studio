//! obs-core: the Windows system info log rules
//! (`libobs/obs-windows-system-info.c.h`), which `obs-windows.c` runs once
//! per startup log. One measurement runs 1024 logs, each the version, both
//! memory sizes, the gaming gate, the Game Mode retry, the five feature
//! lines (put together as `obs-windows.c` does) and a product's state and type labels, and folds the results into a
//! checksum that both sides must match (each label as its length and first
//! byte, since the two sides' strings live at different addresses).
//!
//! Neither side allocates while it runs, and the fixtures are freed with the
//! module, so it leaves no allocation live.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! shared machine at a load average of about 6; criterion's median per 1024
//! logs): 43.7 us Rust, 79.4 us C. Each call is a compare, a shift, a divide
//! by a constant or a table lookup, so both sides cost little more than the
//! call itself; most of each log is the checksum's strlen of every label.

use std::ffi::{CStr, c_char};
use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::win_system_info as c;
use obs_core::ffi::win_system_info as rs;

const LOGS: usize = 1024;

/// A small deterministic generator, so both sides see the same calls.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }
}

struct Log {
    major: u32,
    minor: u32,
    total: u64,
    avail: u64,
    build: u32,
    reads: [(i32, u32); 5],
    state: i32,
    kind: i32,
}

fn fixtures() -> Vec<Log> {
    let mut rng = Lcg(3470);
    let mut pick = |n: u64| usize::try_from(rng.next() % n).unwrap();
    let statuses = [0, 0, 2, 5];
    let values = [0, 1, 0xFFFF_FFFF];
    let versions = [(6, 1), (6, 3), (10, 0)];
    let builds = [7601, 19041, 19042, 22631];
    (0..LOGS)
        .map(|_| {
            let (major, minor) = versions[pick(3)];
            let mut reads = [(0, 0); 5];
            for read in &mut reads {
                *read = (statuses[pick(4)], values[pick(3)]);
            }
            Log {
                major,
                minor,
                total: u64::try_from(pick(64)).unwrap() << 30,
                avail: u64::try_from(pick(1 << 20)).unwrap() << 14,
                build: builds[pick(4)],
                reads,
                state: i32::try_from(pick(5)).unwrap(),
                kind: [1, 4, 8, 2][pick(4)],
            }
        })
        .collect()
}

struct Side {
    version: fn(u32, u32) -> u32,
    mb: fn(u64) -> u32,
    gaming_logged: fn(u32) -> u8,
    game_mode_retry: fn(i32) -> u8,
    gaming_found: fn(i32) -> u8,
    gaming_value_line: fn(i32, u32, &mut *const c_char, &mut *const c_char) -> u8,
    gaming_fallback: fn(i32) -> u8,
    gaming_fallback_line: fn(i32, u32, &mut *const c_char, &mut *const c_char) -> u8,
    product_state: fn(i32) -> *const c_char,
    product_type: fn(i32) -> *const c_char,
}

const RUST: Side = Side {
    version: |a, b| rs::obs_windows_system_info_version(a, b),
    mb: |b| rs::obs_windows_system_info_mb(b),
    gaming_logged: |v| u8::from(rs::obs_windows_system_info_gaming_logged(v)),
    game_mode_retry: |s| u8::from(rs::obs_windows_system_info_game_mode_retry(s)),
    gaming_found: |s| u8::from(rs::obs_windows_system_info_gaming_found(s)),
    // SAFETY: both out-pointers are valid for a write.
    gaming_value_line: |f, v, l, t| unsafe {
        u8::from(rs::obs_windows_system_info_gaming_value_line(f, v, l, t))
    },
    gaming_fallback: |f| u8::from(rs::obs_windows_system_info_gaming_fallback(f)),
    // SAFETY: both out-pointers are valid for a write.
    gaming_fallback_line: |f, b, l, t| unsafe {
        u8::from(rs::obs_windows_system_info_gaming_fallback_line(f, b, l, t))
    },
    product_state: |s| rs::obs_windows_system_info_product_state(s),
    product_type: |k| rs::obs_windows_system_info_product_type(k),
};

// SAFETY (each): plain values, and out-pointers valid for a write.
const C: Side = Side {
    version: |a, b| unsafe { c::oracle_obs_windows_system_info_version(a, b) },
    mb: |b| unsafe { c::oracle_obs_windows_system_info_mb(b) },
    gaming_logged: |v| unsafe { c::oracle_obs_windows_system_info_gaming_logged(v) },
    game_mode_retry: |s| unsafe { c::oracle_obs_windows_system_info_game_mode_retry(s) },
    gaming_found: |s| unsafe { c::oracle_obs_windows_system_info_gaming_found(s) },
    gaming_value_line: |f, v, l, t| unsafe {
        c::oracle_obs_windows_system_info_gaming_value_line(f, v, l, t)
    },
    gaming_fallback: |f| unsafe { c::oracle_obs_windows_system_info_gaming_fallback(f) },
    gaming_fallback_line: |f, b, l, t| unsafe {
        c::oracle_obs_windows_system_info_gaming_fallback_line(f, b, l, t)
    },
    product_state: |s| unsafe { c::oracle_obs_windows_system_info_product_state(s) },
    product_type: |k| unsafe { c::oracle_obs_windows_system_info_product_type(k) },
};

fn fold(sum: u64, value: u64) -> u64 {
    sum.wrapping_mul(31).wrapping_add(value)
}

fn fold_str(sum: u64, s: *const c_char) -> u64 {
    // SAFETY: both sides return 'static NUL-terminated strings.
    let bytes = unsafe { CStr::from_ptr(s) }.to_bytes();
    fold(fold(sum, bytes.len() as u64), u64::from(bytes[0]))
}

fn logs(side: &Side, logs: &[Log]) -> u64 {
    let mut sum = 0u64;
    for log in logs {
        let log = black_box(log);
        let win_ver = (side.version)(log.major, log.minor);
        sum = fold(sum, u64::from(win_ver));
        sum = fold(sum, u64::from((side.mb)(log.total)));
        sum = fold(sum, u64::from((side.mb)(log.avail)));
        sum = fold(sum, u64::from((side.gaming_logged)(win_ver)));
        sum = fold(sum, u64::from((side.game_mode_retry)(log.reads[4].0)));
        for (feature, &(status, value)) in (0..).zip(&log.reads) {
            let mut label: *const c_char = c"".as_ptr();
            let mut text: *const c_char = c"".as_ptr();
            let logged = if (side.gaming_found)(status) != 0 {
                (side.gaming_value_line)(feature, value, &mut label, &mut text)
            } else if (side.gaming_fallback)(feature) != 0 {
                (side.gaming_fallback_line)(feature, log.build, &mut label, &mut text)
            } else {
                0
            };
            if logged != 0 {
                sum = fold_str(fold_str(sum, label), text);
            }
        }
        sum = fold_str(sum, (side.product_state)(log.state));
        sum = fold_str(sum, (side.product_type)(log.kind));
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let fixtures = fixtures();
    assert_eq!(
        logs(&RUST, &fixtures),
        logs(&C, &fixtures),
        "benchmark fixture differs: logs"
    );
    let mut group = criterion.benchmark_group("win_system_info/logs");
    group.throughput(Throughput::Elements(LOGS as u64));
    group.bench_function("rust", |b| b.iter(|| black_box(logs(&RUST, &fixtures))));
    group.bench_function("c", |b| b.iter(|| black_box(logs(&C, &fixtures))));
    group.finish();
}
