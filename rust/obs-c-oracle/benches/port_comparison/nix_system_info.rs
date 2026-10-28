//! obs-core: the Linux and BSD system info parsing
//! (`libobs/obs-nix-system-info.c.h`), which `obs-nix.c` runs over each
//! `/proc/cpuinfo` and `dmesg.boot` line, and over the memory sizes, when it
//! writes the startup log. Each measurement runs 1024 lines or sizes that
//! reach every branch, and folds the results into a checksum that both sides
//! must match:
//! - `cpuinfo`: the three key checks of one line, its value, the physical id
//!   of a `physical id` line and the print rule, over lines of a
//!   two-socket x86 `/proc/cpuinfo` with odd keys and values mixed in;
//! - `dmesg`: the `CPU: ` check and the speed of one `dmesg.boot` line;
//! - `memory`: the total, free and OpenBSD sizes.
//!
//! Neither side allocates while it runs, and the lines are freed with the
//! module's fixtures, so the module leaves no allocation live.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! shared machine at a load average of about 16; criterion's median per
//! 1024 lines or sizes): cpuinfo 29.1 us Rust, 24.8 us C; dmesg 21.9 us
//! Rust, 20.2 us C; memory 8.8 us Rust, 8.6 us C. The Rust side reads each
//! line's length up to its NUL once in the shim, where the C stops at the
//! first byte that differs, so a cpuinfo line costs a few ns more.

use std::ffi::{CString, c_char};
use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::nix_system_info as c;
use obs_core::ffi::nix_system_info as rs;

const CALLS: usize = 1024;

/// A small deterministic generator, so both sides see the same calls.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }
}

const CPUINFO: [&str; 12] = [
    "processor\t: 0\n",
    "model name\t: Intel(R) Xeon(R) CPU E5-2680 v4 @ 2.40GHz\n",
    "cpu MHz\t\t: 2394.454\n",
    "physical id\t: 0\n",
    "physical id\t: 1\n",
    "flags\t\t: fpu vme de pse tsc msr pae mce cx8 apic sep mtrr pge mca\n",
    "\n",
    "model names\t:\n",
    "physical id : x\n",
    "cpu MHz dynamic:",
    "physical id\t: -2147483648\n",
    "BogoMIPS\t: 48.00\n",
];

const DMESG: [&str; 6] = [
    "CPU: AMD Ryzen 7 5800X 8-Core Processor (3800.11-MHz K8-class CPU)\n",
    "CPU: a (1-b (2-c\n",
    "CPU: none\n",
    "CPU: x (\n",
    "CPU: x (500)\n",
    "Copyright (c) 1992-2023 The FreeBSD Project.\n",
];

struct Fixtures {
    cpuinfo: Vec<CString>,
    dmesg: Vec<CString>,
    memory: Vec<(u64, u64, u64, u32, i64)>,
}

fn fixtures() -> Fixtures {
    let mut rng = Lcg(2719);
    let pick = |rng: &mut Lcg, lines: &[&str]| {
        let i = usize::try_from(rng.next() % lines.len() as u64).unwrap();
        CString::new(lines[i]).unwrap()
    };
    let cpuinfo = (0..CALLS).map(|_| pick(&mut rng, &CPUINFO)).collect();
    let dmesg = (0..CALLS).map(|_| pick(&mut rng, &DMESG)).collect();
    let memory = (0..CALLS)
        .map(|_| {
            let units = [0u32, 1, 4096, u32::MAX];
            (
                rng.next(),
                rng.next() >> 20,
                rng.next() >> 20,
                units[usize::try_from(rng.next() % 4).unwrap()],
                rng.next().cast_signed() - (1 << 52),
            )
        })
        .collect();
    Fixtures {
        cpuinfo,
        dmesg,
        memory,
    }
}

type Line = *const c_char;

struct Side {
    is_name: fn(Line) -> u8,
    is_physical_id: fn(Line) -> u8,
    is_speed: fn(Line) -> u8,
    value: fn(Line) -> Line,
    physical_id: fn(Line) -> i32,
    prints: fn(Line, i32, i32) -> u8,
    is_dmesg_cpu: fn(Line) -> u8,
    dmesg_speed: fn(Line, &mut usize) -> Line,
    total_mb: fn(u64, u32) -> u64,
    free_mb: fn(u64, u64, u32) -> u64,
    physmem_mb: fn(i64) -> i64,
}

// SAFETY (each): every line is a live NUL-terminated string from
// `fixtures()`, a value points into one, and `len` is a writable usize.
const RUST: Side = Side {
    is_name: |l| u8::from(unsafe { rs::obs_nix_system_info_is_name(l) }),
    is_physical_id: |l| u8::from(unsafe { rs::obs_nix_system_info_is_physical_id(l) }),
    is_speed: |l| u8::from(unsafe { rs::obs_nix_system_info_is_speed(l) }),
    value: |l| unsafe { rs::obs_nix_system_info_value(l) },
    physical_id: |v| unsafe { rs::obs_nix_system_info_physical_id(v) },
    prints: |l, id, last| u8::from(unsafe { rs::obs_nix_system_info_prints(l, id, last) }),
    is_dmesg_cpu: |l| u8::from(unsafe { rs::obs_nix_system_info_is_dmesg_cpu(l) }),
    dmesg_speed: |l, len| unsafe { rs::obs_nix_system_info_dmesg_speed(l, len) },
    total_mb: |t, u| rs::obs_nix_system_info_total_mb(t, u),
    free_mb: |f, b, u| rs::obs_nix_system_info_free_mb(f, b, u),
    physmem_mb: |m| rs::obs_nix_system_info_physmem_mb(m),
};

// SAFETY (each): as for RUST; the size functions take plain values.
const C: Side = Side {
    is_name: |l| unsafe { c::oracle_obs_nix_system_info_is_name(l) },
    is_physical_id: |l| unsafe { c::oracle_obs_nix_system_info_is_physical_id(l) },
    is_speed: |l| unsafe { c::oracle_obs_nix_system_info_is_speed(l) },
    value: |l| unsafe { c::oracle_obs_nix_system_info_value(l) },
    physical_id: |v| unsafe { c::oracle_obs_nix_system_info_physical_id(v) },
    prints: |l, id, last| unsafe { c::oracle_obs_nix_system_info_prints(l, id, last) },
    is_dmesg_cpu: |l| unsafe { c::oracle_obs_nix_system_info_is_dmesg_cpu(l) },
    dmesg_speed: |l, len| unsafe { c::oracle_obs_nix_system_info_dmesg_speed(l, len) },
    total_mb: |t, u| unsafe { c::oracle_obs_nix_system_info_total_mb(t, u) },
    free_mb: |f, b, u| unsafe { c::oracle_obs_nix_system_info_free_mb(f, b, u) },
    physmem_mb: |m| unsafe { c::oracle_obs_nix_system_info_physmem_mb(m) },
};

fn fold(sum: u64, value: u64) -> u64 {
    sum.wrapping_mul(31).wrapping_add(value)
}

/// The offset of a returned pointer in its line, or `u64::MAX` for NULL.
fn offset(line: Line, p: Line) -> u64 {
    if p.is_null() {
        u64::MAX
    } else {
        (p as usize - line as usize) as u64
    }
}

fn cpuinfo(side: &Side, f: &Fixtures) -> u64 {
    let mut sum = 0u64;
    let mut physical_id = -1;
    let mut last = -1;
    for line in &f.cpuinfo {
        let l = black_box(line.as_ptr());
        if (side.is_name)(l) != 0 {
            sum = fold(sum, offset(l, (side.value)(l)));
        }
        if (side.is_physical_id)(l) != 0 {
            let v = (side.value)(l);
            if !v.is_null() {
                physical_id = (side.physical_id)(v);
            }
        }
        if (side.is_speed)(l) != 0 {
            sum = fold(sum, offset(l, (side.value)(l)));
        }
        if (side.prints)(l, physical_id, last) != 0 {
            last = physical_id;
            sum = fold(sum, u64::from(physical_id.cast_unsigned()));
        }
    }
    sum
}

fn dmesg(side: &Side, f: &Fixtures) -> u64 {
    let mut sum = 0u64;
    for line in &f.dmesg {
        let l = black_box(line.as_ptr());
        if (side.is_dmesg_cpu)(l) != 0 {
            let mut len = usize::MAX;
            sum = fold(sum, offset(l, (side.dmesg_speed)(l, &mut len)));
            sum = fold(sum, len as u64);
        }
    }
    sum
}

fn memory(side: &Side, f: &Fixtures) -> u64 {
    let mut sum = 0u64;
    for &m in &f.memory {
        let (total, free, buffer, unit, mem) = black_box(m);
        sum = fold(sum, (side.total_mb)(total, unit));
        sum = fold(sum, (side.free_mb)(free, buffer, unit));
        sum = fold(sum, (side.physmem_mb)(mem).cast_unsigned());
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let fixtures = fixtures();
    for (name, run) in [
        ("cpuinfo", cpuinfo as fn(&Side, &Fixtures) -> u64),
        ("dmesg", dmesg),
        ("memory", memory),
    ] {
        assert_eq!(
            run(&RUST, &fixtures),
            run(&C, &fixtures),
            "benchmark fixture differs: {name}"
        );
        let mut group = criterion.benchmark_group(format!("nix_system_info/{name}"));
        group.throughput(Throughput::Elements(CALLS as u64));
        group.bench_function("rust", |b| b.iter(|| black_box(run(&RUST, &fixtures))));
        group.bench_function("c", |b| b.iter(|| black_box(run(&C, &fixtures))));
        group.finish();
    }
}
