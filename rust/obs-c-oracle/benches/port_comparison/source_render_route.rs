//! obs-source: the source and filter render routing
//! (`libobs/obs-source-render-route.c.h`), which `source_render_video()`
//! runs for every source and filter drawn, every frame, and each filter's
//! `obs_source_process_filter_begin_with_color_space()`,
//! `obs_source_process_filter_tech_end()` and, when it skips,
//! `obs_source_skip_video_filter()`. Each measurement runs 1024 calls that
//! reach every branch and folds the results into a checksum that both sides
//! must match:
//! - `render_video`: the decisions of one `source_render_video()` in its
//!   order (the "no video" check, the async update, the inactive check, the
//!   draw walk and, for a main render, `obs_source_main_render()`'s rules),
//!   over every type, the flags the rules read and every combination of the
//!   source's booleans;
//! - `filter`: one filter's begin (the target and parent check, the bypass
//!   with its space read, the zero size skip, the texrender format check and
//!   the draw into it) and tech end (the linear sRGB flag and bypass or
//!   texture), over every wiring, flag pattern, direct render choice and
//!   space pair;
//! - `skip`: a skipped filter's check and draw walk.
//!
//! The fixtures are plain arrays on the Rust heap: nothing is allocated with
//! bmalloc on either side.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! machine at a load average of 13 to 24; median per 1024 calls, two runs):
//! render_video 47.0 and 31.4 us Rust, 79.6 and 33.6 us C; filter 8.3 and
//! 7.2 us Rust, 9.3 and 12.3 us C; skip 4.3 and 13.9 us Rust, 4.2 and 8.9 us
//! C. Every pair is within the noise of that machine: a few ns per source or
//! filter either way, the calls being a handful of compares each.

use std::hint::black_box;
use std::os::raw::c_int;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_render_route as c;
use obs_source::ffi::source_render_route as rs;

const CALLS: usize = 1024;

/// A small deterministic generator, so both sides see the same calls.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 16
    }

    fn pick<T: Copy>(&mut self, from: &[T]) -> T {
        let i = usize::try_from(self.next() % from.len() as u64).unwrap();
        from[i]
    }
}

/// The functions one side links, as plain Rust function pointers.
struct Side {
    filter: fn(bool, bool) -> u32,
    video_reads_flags: fn(c_int) -> bool,
    video_none: fn(c_int, u32) -> bool,
    async_reads_flags: fn(c_int) -> bool,
    async_reads_rendering_filter: fn(c_int, u32) -> bool,
    async_update: fn(c_int, u32, bool) -> bool,
    reads_enabled: fn(bool) -> bool,
    inactive: fn(bool, bool) -> bool,
    draw: fn(bool) -> u32,
    draw_next: fn(u32, bool) -> u32,
    main_custom_draw: fn(u32) -> bool,
    main_srgb_aware: fn(u32) -> bool,
    main_reads_filters: fn(bool) -> bool,
    main_default_effect: fn(u32, bool, bool) -> bool,
    bypass_reads_space: fn(bool, c_int, u32, u32) -> bool,
    bypass: fn(bool, c_int, u32, u32, c_int, c_int) -> bool,
    size_skip: fn(c_int, c_int) -> bool,
    texrender_stale: fn(c_int, c_int) -> bool,
    filter_render: fn(bool, u32) -> u32,
    tech_end_linear_srgb: fn(u32) -> bool,
    tech_end: fn(bool) -> u32,
    skip: fn(bool, u32) -> u32,
}

const RUST: Side = Side {
    filter: |t, p| rs::obs_source_render_route_filter(t, p),
    video_reads_flags: |t| rs::obs_source_render_route_video_reads_flags(t),
    video_none: |t, f| rs::obs_source_render_route_video_none(t, f),
    async_reads_flags: |t| rs::obs_source_render_route_async_reads_flags(t),
    async_reads_rendering_filter: |t, f| {
        rs::obs_source_render_route_async_reads_rendering_filter(t, f)
    },
    async_update: |t, f, r| rs::obs_source_render_route_async_update(t, f, r),
    reads_enabled: |d| rs::obs_source_render_route_reads_enabled(d),
    inactive: |d, e| rs::obs_source_render_route_inactive(d, e),
    draw: |f| rs::obs_source_render_route_draw(f),
    draw_next: |s, v| rs::obs_source_render_route_draw_next(s, v),
    main_custom_draw: |f| rs::obs_source_render_route_main_custom_draw(f),
    main_srgb_aware: |f| rs::obs_source_render_route_main_srgb_aware(f),
    main_reads_filters: |p| rs::obs_source_render_route_main_reads_filters(p),
    main_default_effect: |f, p, h| rs::obs_source_render_route_main_default_effect(f, p, h),
    bypass_reads_space: |s, a, ff, pf| rs::obs_source_render_route_bypass_reads_space(s, a, ff, pf),
    bypass: |s, a, ff, pf, sp, cs| rs::obs_source_render_route_bypass(s, a, ff, pf, sp, cs),
    size_skip: |x, y| rs::obs_source_render_route_size_skip(x, y),
    texrender_stale: |x, y| rs::obs_source_render_route_texrender_stale(x, y),
    filter_render: |s, f| rs::obs_source_render_route_filter_render(s, f),
    tech_end_linear_srgb: |f| rs::obs_source_render_route_tech_end_linear_srgb(f),
    tech_end: |b| rs::obs_source_render_route_tech_end(b),
    skip: |s, f| rs::obs_source_render_route_skip(s, f),
};

// SAFETY (every closure below): the oracle functions take and return plain
// values.
const C: Side = Side {
    filter: |t, p| unsafe { c::oracle_obs_source_render_route_filter(t, p) },
    video_reads_flags: |t| unsafe { c::oracle_obs_source_render_route_video_reads_flags(t) != 0 },
    video_none: |t, f| unsafe { c::oracle_obs_source_render_route_video_none(t, f) != 0 },
    async_reads_flags: |t| unsafe { c::oracle_obs_source_render_route_async_reads_flags(t) != 0 },
    async_reads_rendering_filter: |t, f| unsafe {
        c::oracle_obs_source_render_route_async_reads_rendering_filter(t, f) != 0
    },
    async_update: |t, f, r| unsafe { c::oracle_obs_source_render_route_async_update(t, f, r) != 0 },
    reads_enabled: |d| unsafe { c::oracle_obs_source_render_route_reads_enabled(d) != 0 },
    inactive: |d, e| unsafe { c::oracle_obs_source_render_route_inactive(d, e) != 0 },
    draw: |f| unsafe { c::oracle_obs_source_render_route_draw(f) },
    draw_next: |s, v| unsafe { c::oracle_obs_source_render_route_draw_next(s, v) },
    main_custom_draw: |f| unsafe { c::oracle_obs_source_render_route_main_custom_draw(f) != 0 },
    main_srgb_aware: |f| unsafe { c::oracle_obs_source_render_route_main_srgb_aware(f) != 0 },
    main_reads_filters: |p| unsafe { c::oracle_obs_source_render_route_main_reads_filters(p) != 0 },
    main_default_effect: |f, p, h| unsafe {
        c::oracle_obs_source_render_route_main_default_effect(f, p, h) != 0
    },
    bypass_reads_space: |s, a, ff, pf| unsafe {
        c::oracle_obs_source_render_route_bypass_reads_space(s, a, ff, pf) != 0
    },
    bypass: |s, a, ff, pf, sp, cs| unsafe {
        c::oracle_obs_source_render_route_bypass(s, a, ff, pf, sp, cs) != 0
    },
    size_skip: |x, y| unsafe { c::oracle_obs_source_render_route_size_skip(x, y) != 0 },
    texrender_stale: |x, y| unsafe { c::oracle_obs_source_render_route_texrender_stale(x, y) != 0 },
    filter_render: |s, f| unsafe { c::oracle_obs_source_render_route_filter_render(s, f) },
    tech_end_linear_srgb: |f| unsafe {
        c::oracle_obs_source_render_route_tech_end_linear_srgb(f) != 0
    },
    tech_end: |b| unsafe { c::oracle_obs_source_render_route_tech_end(b) },
    skip: |s, f| unsafe { c::oracle_obs_source_render_route_skip(s, f) },
};

/// `OBS_SOURCE_VIDEO`, `_ASYNC`, `_CUSTOM_DRAW`, `_SRGB` and `_AUDIO`.
const FLAGS: [u32; 10] = [
    1,
    1 | 4,
    1 | 8,
    1 | 1 << 15,
    1 | 4 | 1 << 15,
    2,
    4,
    8 | 1 << 15,
    0,
    1 | 2,
];
/// The draw steps that read a value, in the C's numbers.
const READ_RENDERING_FILTER: u32 = 6;
const READ_VIDEO_RENDER: u32 = 7;
const READ_FILTER_TARGET: u32 = 8;
const READ_DEINTERLACING: u32 = 9;
const READ_TARGET_VIDEO_RENDER: u32 = 10;
const READ_TARGET_DEINTERLACING: u32 = 11;
const MAIN: u32 = 2;

#[derive(Clone, Copy)]
struct Call {
    source_type: c_int,
    flags: u32,
    parent_flags: u32,
    bits: u32,
    allow: c_int,
    space: c_int,
    current_space: c_int,
    cx: c_int,
    cy: c_int,
    format: c_int,
}

fn calls() -> Vec<Call> {
    let mut g = Lcg(0x2189);
    (0..CALLS)
        .map(|_| Call {
            source_type: g.pick(&[0, 0, 0, 1, 1, 2, 3]),
            flags: g.pick(&FLAGS),
            parent_flags: g.pick(&FLAGS),
            bits: u32::try_from(g.next() & 0xffff).unwrap(),
            allow: g.pick(&[1, 1, 0]),
            space: g.pick(&[0, 0, 1, 2, 3]),
            current_space: g.pick(&[0, 0, 2, 3]),
            cx: g.pick(&[1920, 1280, 0]),
            cy: g.pick(&[1080, 720, 1080, 0]),
            format: g.pick(&[3, 3, 9]),
        })
        .collect()
}

fn bit(c: &Call, n: u32) -> bool {
    c.bits & (1 << n) != 0
}

fn walk(side: &Side, mut route: u32, values: [bool; 6]) -> u32 {
    loop {
        let value = match route {
            READ_RENDERING_FILTER => values[0],
            READ_VIDEO_RENDER | READ_TARGET_VIDEO_RENDER => values[1],
            READ_FILTER_TARGET => values[2],
            READ_DEINTERLACING | READ_TARGET_DEINTERLACING => values[3],
            _ => return route,
        };
        route = (side.draw_next)(route, value);
    }
}

fn render_video(side: &Side, calls: &[Call]) -> u64 {
    let mut sum = 0u64;
    for c in calls {
        let flags = if (side.video_reads_flags)(c.source_type) {
            c.flags
        } else {
            0
        };
        if (side.video_none)(c.source_type, flags) {
            sum = sum.wrapping_mul(31).wrapping_add(1);
            continue;
        }
        let flags = if (side.async_reads_flags)(c.source_type) {
            c.flags
        } else {
            0
        };
        let rf = (side.async_reads_rendering_filter)(c.source_type, flags) && bit(c, 0);
        let update = (side.async_update)(c.source_type, flags, rf);
        let has_data = bit(c, 1) || bit(c, 2);
        let enabled = (side.reads_enabled)(has_data) && (bit(c, 3) || bit(c, 4));
        if (side.inactive)(has_data, enabled) {
            sum = sum.wrapping_mul(31).wrapping_add(2 + u64::from(update));
            continue;
        }
        let values = [bit(c, 0), bit(c, 5), bit(c, 6), bit(c, 7), false, false];
        let route = walk(side, (side.draw)(bit(c, 8)), values);
        let mut main = 0u64;
        if route == MAIN {
            let reads = (side.main_reads_filters)(bit(c, 9));
            main = u64::from((side.main_custom_draw)(c.flags))
                | u64::from((side.main_srgb_aware)(c.flags)) << 1
                | u64::from((side.main_default_effect)(
                    c.flags,
                    bit(c, 9),
                    reads && bit(c, 8),
                )) << 2;
        }
        sum = sum
            .wrapping_mul(31)
            .wrapping_add(u64::from(route) << 8 | main << 4 | u64::from(update));
    }
    sum
}

fn filter(side: &Side, calls: &[Call]) -> u64 {
    let mut sum = 0u64;
    for c in calls {
        let (has_target, has_parent, same) = (!bit(c, 10), !bit(c, 11), bit(c, 12));
        let check = (side.filter)(has_target, has_parent);
        if check != 0 {
            sum = sum.wrapping_mul(31).wrapping_add(u64::from(check));
            continue;
        }
        let reads = (side.bypass_reads_space)(same, c.allow, c.flags, c.parent_flags);
        let current = if reads { c.current_space } else { 0 };
        let bypass = (side.bypass)(same, c.allow, c.flags, c.parent_flags, c.space, current);
        let mut begin = u64::from(bypass);
        if !bypass {
            if (side.size_skip)(c.cx, c.cy) {
                begin |= 2;
            } else {
                if bit(c, 13) {
                    begin |= u64::from((side.texrender_stale)(c.format, 3)) << 2;
                }
                begin |= u64::from((side.filter_render)(same, c.parent_flags)) << 3;
            }
        }
        let end = u64::from((side.tech_end_linear_srgb)(c.flags))
            | u64::from((side.tech_end)(bypass)) << 1;
        sum = sum.wrapping_mul(31).wrapping_add(begin << 4 | end);
    }
    sum
}

fn skip(side: &Side, calls: &[Call]) -> u64 {
    let mut sum = 0u64;
    for c in calls {
        let check = (side.filter)(!bit(c, 10), !bit(c, 11));
        let route = if check == 0 {
            walk(
                side,
                (side.skip)(bit(c, 12), c.parent_flags),
                [false, bit(c, 5), false, bit(c, 7), false, false],
            )
        } else {
            100 + check
        };
        sum = sum.wrapping_mul(31).wrapping_add(u64::from(route));
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let calls = calls();
    for (name, run) in [
        ("render_video", render_video as fn(&Side, &[Call]) -> u64),
        ("filter", filter),
        ("skip", skip),
    ] {
        assert_eq!(
            run(&RUST, &calls),
            run(&C, &calls),
            "benchmark fixture differs: {name}"
        );
        let mut group = criterion.benchmark_group(format!("source_render_route/{name}"));
        group.throughput(Throughput::Elements(CALLS as u64));
        group.bench_function("rust", |b| b.iter(|| black_box(run(&RUST, &calls))));
        group.bench_function("c", |b| b.iter(|| black_box(run(&C, &calls))));
        group.finish();
    }
}
