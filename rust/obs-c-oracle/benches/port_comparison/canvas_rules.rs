//! obs-core: the canvas flag rules and setter checks
//! (`libobs/obs-canvas-rules.c.h`), which `obs-canvas.c` asks when a canvas
//! is created, saved, reset, renamed or removed, and each time a scene joins
//! or leaves one or a channel is set. Each measurement runs 1024 calls that
//! reach every branch, and folds the results into a checksum that both sides
//! must match:
//! - `create`: the constructor's flags (the main canvas's or the public
//!   mask), the view type, `mix_audio` and the public canvas rule, over every
//!   combination of the known flags, `REMOVED` and an unknown bit;
//! - `setters`: one setter's checks in their order (the save, the reset, the
//!   channel bound and global signal, the name refusals and the public rename,
//!   the remove and the `REMOVED` write, the scene reference rule), over the
//!   same flags, channels around the bound and empty and set names.
//!
//! Neither side allocates, so the module leaves no allocation live.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! shared machine at a load average of about 3; criterion's median per 1024 calls): create 7.8 us
//! Rust, 5.4 us C; setters 16.7 us Rust, 11.4 us C. About 2 to 5 ns a
//! decision more on the Rust side, each through its exported shim.

use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::canvas_rules as c;
use obs_core::ffi::canvas_rules as rs;

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

const BITS: [u32; 7] = [1 << 0, 1 << 1, 1 << 2, 1 << 3, 1 << 4, 1 << 31, 1 << 5];

#[derive(Clone, Copy)]
struct Call {
    flags: u32,
    main: bool,
    private: bool,
    channel: u32,
    name: [u8; 2],
    has_name: bool,
}

fn calls() -> Vec<Call> {
    let mut rng = Lcg(2544);
    (0..CALLS)
        .map(|_| {
            let r = rng.next();
            let flags = BITS
                .iter()
                .enumerate()
                .filter(|(i, _)| (r >> i) & 1 != 0)
                .fold(0, |f, (_, bit)| f | bit);
            Call {
                flags,
                main: (r >> 7).is_multiple_of(8),
                private: (r >> 10).is_multiple_of(4),
                channel: u32::try_from((r >> 12) % 70).unwrap(),
                name: [if (r >> 19).is_multiple_of(4) { 0 } else { b'A' }, 0],
                has_name: !(r >> 21).is_multiple_of(8),
            }
        })
        .collect()
}

struct Side {
    main_flags: fn() -> u32,
    public_flags: fn(u32) -> u32,
    main_view: fn(u32) -> u8,
    mix_audio: fn(u32) -> u8,
    public: fn(bool) -> u8,
    holds_scenes: fn(u32) -> u8,
    saves: fn(u32) -> u8,
    resets: fn(u32) -> u8,
    global_signals: fn(u32) -> u8,
    channel_ok: fn(u32) -> u8,
    name_empty: fn(*const u8) -> u8,
    renames: fn(u32) -> u8,
    removable: fn(u32) -> u8,
    mark_removed: fn(u32) -> u32,
}

const RUST: Side = Side {
    main_flags: || rs::obs_canvas_rules_main_flags(),
    public_flags: |f| rs::obs_canvas_rules_public_flags(f),
    main_view: |f| u8::from(rs::obs_canvas_rules_main_view(f)),
    mix_audio: |f| u8::from(rs::obs_canvas_rules_mix_audio(f)),
    public: |p| u8::from(rs::obs_canvas_rules_public(p)),
    holds_scenes: |f| u8::from(rs::obs_canvas_rules_holds_scenes(f)),
    saves: |f| u8::from(rs::obs_canvas_rules_saves(f)),
    resets: |f| u8::from(rs::obs_canvas_rules_resets(f)),
    global_signals: |f| u8::from(rs::obs_canvas_rules_global_signals(f)),
    channel_ok: |ch| u8::from(rs::obs_canvas_rules_channel_ok(ch)),
    // SAFETY: NULL or a NUL-terminated name from `calls()`.
    name_empty: |n| u8::from(unsafe { rs::obs_canvas_rules_name_empty(n.cast()) }),
    renames: |f| u8::from(rs::obs_canvas_rules_renames(f)),
    removable: |f| u8::from(rs::obs_canvas_rules_removable(f)),
    mark_removed: |f| rs::obs_canvas_rules_mark_removed(f),
};

// SAFETY (each): the oracle functions take and return plain values, and the
// name is NULL or NUL-terminated.
const C: Side = Side {
    main_flags: || unsafe { c::oracle_obs_canvas_rules_main_flags() },
    public_flags: |f| unsafe { c::oracle_obs_canvas_rules_public_flags(f) },
    main_view: |f| unsafe { c::oracle_obs_canvas_rules_main_view(f) },
    mix_audio: |f| unsafe { c::oracle_obs_canvas_rules_mix_audio(f) },
    public: |p| unsafe { c::oracle_obs_canvas_rules_public(p) },
    holds_scenes: |f| unsafe { c::oracle_obs_canvas_rules_holds_scenes(f) },
    saves: |f| unsafe { c::oracle_obs_canvas_rules_saves(f) },
    resets: |f| unsafe { c::oracle_obs_canvas_rules_resets(f) },
    global_signals: |f| unsafe { c::oracle_obs_canvas_rules_global_signals(f) },
    channel_ok: |ch| unsafe { c::oracle_obs_canvas_rules_channel_ok(ch) },
    name_empty: |n| unsafe { c::oracle_obs_canvas_rules_name_empty(n.cast()) },
    renames: |f| unsafe { c::oracle_obs_canvas_rules_renames(f) },
    removable: |f| unsafe { c::oracle_obs_canvas_rules_removable(f) },
    mark_removed: |f| unsafe { c::oracle_obs_canvas_rules_mark_removed(f) },
};

fn fold(sum: u64, value: u64) -> u64 {
    sum.wrapping_mul(31).wrapping_add(value)
}

fn create(side: &Side, calls: &[Call]) -> u64 {
    let mut sum = 0u64;
    for call in calls {
        let call = black_box(*call);
        let flags = if call.main {
            (side.main_flags)()
        } else {
            (side.public_flags)(call.flags)
        };
        sum = fold(sum, u64::from(flags));
        sum = fold(sum, u64::from((side.main_view)(flags)));
        sum = fold(sum, u64::from((side.mix_audio)(flags)));
        sum = fold(sum, u64::from((side.public)(call.private)));
    }
    sum
}

fn setters(side: &Side, calls: &[Call]) -> u64 {
    let mut sum = 0u64;
    for call in calls {
        let call = black_box(*call);
        let f = call.flags;
        sum = fold(sum, u64::from((side.saves)(f)));
        sum = fold(sum, u64::from((side.resets)(f)));
        if (side.channel_ok)(call.channel) != 0 {
            sum = fold(sum, u64::from((side.global_signals)(f)));
        }
        let name = if call.has_name {
            call.name.as_ptr()
        } else {
            std::ptr::null()
        };
        if (side.name_empty)(name) == 0 && (side.renames)(f) != 0 {
            sum = fold(sum, u64::from((side.public)(call.private)));
        }
        if (side.removable)(f) != 0 {
            sum = fold(sum, u64::from((side.mark_removed)(f)));
        }
        sum = fold(sum, u64::from((side.holds_scenes)(f)));
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let calls = calls();
    for (name, run) in [
        ("create", create as fn(&Side, &[Call]) -> u64),
        ("setters", setters),
    ] {
        assert_eq!(
            run(&RUST, &calls),
            run(&C, &calls),
            "benchmark fixture differs: {name}"
        );
        let mut group = criterion.benchmark_group(format!("canvas_rules/{name}"));
        group.throughput(Throughput::Elements(CALLS as u64));
        group.bench_function("rust", |b| b.iter(|| black_box(run(&RUST, &calls))));
        group.bench_function("c", |b| b.iter(|| black_box(run(&C, &calls))));
        group.finish();
    }
}
