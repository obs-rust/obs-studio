//! obs-core: the Windows crash report rules
//! (`libobs/obs-win-crash-report.c.h`), which `obs-win-crash-handler.c` runs
//! for each loaded module, each thread and each stack frame when it writes a
//! crash report. Each measurement runs 1024 calls that reach every branch,
//! and folds the results into a checksum that both sides must match:
//! - `modules`: the module rule over addresses at and around each module's
//!   edges;
//! - `frames`: the thread rules, the frame line, both masks and the name
//!   prefix of one frame.
//!
//! Neither side allocates while it runs, and the fixtures are freed with the
//! module, so it leaves no allocation live.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! shared machine at a load average of about 2; criterion's median per 1024
//! calls): modules 2.76 us Rust, 1.83 us C; frames 13.1 us Rust, 10.2 us C.
//! Each call is a compare or a mask on integers, so both sides cost little
//! more than the call itself, about a nanosecond apart.

use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::win_crash_report as c;
use obs_core::ffi::win_crash_report as rs;

const CALLS: usize = 1024;

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

struct Fixtures {
    modules: Vec<(u64, u64, u32)>,
    frames: Vec<(u32, u32, u32, u64, usize, bool)>,
}

fn fixtures() -> Fixtures {
    let mut rng = Lcg(2836);
    let modules = (0..CALLS)
        .map(|_| {
            let base = match rng.next() % 3 {
                0 => 0x7FF6_0000_0000 + (rng.next() >> 40),
                1 => u64::MAX - (rng.next() >> 32),
                _ => rng.next(),
            };
            let size = u32::try_from(rng.next() >> 40).unwrap();
            let end = base.wrapping_add(u64::from(size));
            let addr = match rng.next() % 4 {
                0 => base,
                1 => end.wrapping_sub(1),
                2 => end,
                _ => base.wrapping_add(rng.next() >> 44),
            };
            (addr, base, size)
        })
        .collect();
    let frames = (0..CALLS)
        .map(|_| {
            let flags = [0u32, 0x200, 0x800, 0xA00, u32::MAX];
            (
                u32::try_from(rng.next() % 4).unwrap(),
                u32::try_from(rng.next() % 2).unwrap(),
                flags[usize::try_from(rng.next() % 5).unwrap()],
                rng.next(),
                usize::try_from(rng.next() % 3).unwrap(),
                rng.next().is_multiple_of(2),
            )
        })
        .collect();
    Fixtures { modules, frames }
}

struct Side {
    contains: fn(u64, u64, u32) -> u8,
    is_crashed: fn(u32, u32) -> u8,
    in_pass: fn(bool, bool) -> u8,
    other_process: fn(u32, u32) -> u8,
    names_symbol: fn(u32) -> u8,
    mask_address: fn(u64) -> u64,
    mask_param: fn(u64) -> u64,
    name_prefix: fn(usize) -> u8,
}

const RUST: Side = Side {
    contains: |a, b, s| u8::from(rs::obs_win_crash_report_contains(a, b, s)),
    is_crashed: |a, b| u8::from(rs::obs_win_crash_report_is_crashed(a, b)),
    in_pass: |f, c| u8::from(rs::obs_win_crash_report_in_pass(f, c)),
    other_process: |a, b| u8::from(rs::obs_win_crash_report_other_process(a, b)),
    names_symbol: |f| u8::from(rs::obs_win_crash_report_names_symbol(f)),
    mask_address: |v| rs::obs_win_crash_report_mask_address(v),
    mask_param: |v| rs::obs_win_crash_report_mask_param(v),
    name_prefix: |l| u8::from(rs::obs_win_crash_report_name_prefix(l)),
};

// SAFETY (each): plain values.
const C: Side = Side {
    contains: |a, b, s| unsafe { c::oracle_obs_win_crash_report_contains(a, b, s) },
    is_crashed: |a, b| unsafe { c::oracle_obs_win_crash_report_is_crashed(a, b) },
    in_pass: |f, cr| unsafe { c::oracle_obs_win_crash_report_in_pass(f, cr) },
    other_process: |a, b| unsafe { c::oracle_obs_win_crash_report_other_process(a, b) },
    names_symbol: |f| unsafe { c::oracle_obs_win_crash_report_names_symbol(f) },
    mask_address: |v| unsafe { c::oracle_obs_win_crash_report_mask_address(v) },
    mask_param: |v| unsafe { c::oracle_obs_win_crash_report_mask_param(v) },
    name_prefix: |l| unsafe { c::oracle_obs_win_crash_report_name_prefix(l) },
};

fn fold(sum: u64, value: u64) -> u64 {
    sum.wrapping_mul(31).wrapping_add(value)
}

fn modules(side: &Side, f: &Fixtures) -> u64 {
    let mut sum = 0u64;
    for &m in &f.modules {
        let (addr, base, size) = black_box(m);
        sum = fold(sum, u64::from((side.contains)(addr, base, size)));
    }
    sum
}

fn frames(side: &Side, f: &Fixtures) -> u64 {
    let mut sum = 0u64;
    for &frame in &f.frames {
        let (id, owner, flags, value, len, first) = black_box(frame);
        let crashed = (side.is_crashed)(id, 1) != 0;
        sum = fold(sum, u64::from((side.in_pass)(first, crashed)));
        sum = fold(sum, u64::from((side.other_process)(owner, 0)));
        sum = fold(sum, u64::from((side.names_symbol)(flags)));
        sum = fold(sum, (side.mask_address)(value));
        sum = fold(sum, (side.mask_param)(value));
        sum = fold(sum, u64::from((side.name_prefix)(len)));
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let fixtures = fixtures();
    for (name, run) in [
        ("modules", modules as fn(&Side, &Fixtures) -> u64),
        ("frames", frames),
    ] {
        assert_eq!(
            run(&RUST, &fixtures),
            run(&C, &fixtures),
            "benchmark fixture differs: {name}"
        );
        let mut group = criterion.benchmark_group(format!("win_crash_report/{name}"));
        group.throughput(Throughput::Elements(CALLS as u64));
        group.bench_function("rust", |b| b.iter(|| black_box(run(&RUST, &fixtures))));
        group.bench_function("c", |b| b.iter(|| black_box(run(&C, &fixtures))));
        group.finish();
    }
}
