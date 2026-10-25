//! obs-source: the async upload decisions
//! (`libobs/obs-source-async-upload.c.h`), which every async source runs once
//! a frame it shows: `obs_source_update_async_video()` (the BGR swap and the
//! re-anchor), `set_async_texture_size()` (the compare, and the GPU route
//! when the textures are rebuilt), `update_async_textures()` (the linear
//! alpha flag and the route) and, for a converted frame,
//! `update_async_texrender()` (the shader parameters). Each measurement runs
//! 1024 frames that reach every branch, and folds the results into a
//! checksum that both sides must match:
//! - `frame`: the decisions of one shown frame in their order, over frames
//!   whose size, format, range or transfer changes now and then, BGRX and
//!   BGRA textures, decoupled and unbuffered sources and both routes;
//! - `texrender`: the uniforms of one conversion draw in its order (the
//!   size uniforms, the HLG peak, the maximum nits, its ratio to the white
//!   level, `hdr_lw`, the range), over 1080p, 4K and odd sizes, the SRGB,
//!   PQ and HLG transfers, peaks of 400 to 10000 and white levels of 80 to
//!   480 nits.
//!
//! Neither side allocates, so the module leaves no allocation live.
//!
//! Measured when it was added (x86_64 Linux, GCC, release, not pinned to a
//! core, a machine at a load average of about 4; median per 1024 frames, two
//! runs): frame 23.2 and 22.6 us Rust, 14.8 and 15.0 us C (1.5x to 1.6x),
//! about 8 ns more per frame shown; texrender 21.0 and 21.3 us Rust, 12.1
//! and 12.5 us C (1.7x), about 9 ns more per converted frame, beside the
//! frame's texture upload and conversion draw.

use std::hint::black_box;
use std::os::raw::c_int;

use criterion::{Criterion, Throughput};
use obs_c_oracle::source_async_upload::{self as c, OracleUploadFormat};
use obs_source::ffi::source_async_upload::{self as rs, obs_source_async_upload_format};

const CALLS: usize = 1024;

/// The five compared values: width, height, format, full range, transfer.
type Fmt = (u32, u32, c_int, bool, u8);

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

fn pick<T: Copy>(r: u64, from: &[T]) -> T {
    from[usize::try_from(r % u64::try_from(from.len()).unwrap()).unwrap()]
}

#[derive(Clone, Copy)]
struct Frame {
    cur: Fmt,
    frame: Fmt,
    convert_type: c_int,
    texture_format: c_int,
    flags: u8,
    gpu_conversion: bool,
    has_texrender: bool,
    decoupled: bool,
    unbuffered: bool,
    peak: f32,
    white: f32,
    max_luminance: u16,
}

fn frames() -> Vec<Frame> {
    let mut rng = Lcg(2336);
    (0..CALLS)
        .map(|_| {
            let r = rng.next();
            let size = pick(r, &[(1920, 1080), (3840, 2160), (1280, 720), (1919, 1081)]);
            let format = pick(r >> 4, &[2, 1, 7, 8, 17, 18]); // NV12, I420, BGRA, BGRX, I010, P010
            let full_range = (r >> 8).is_multiple_of(2);
            let trc = pick(r >> 9, &[1u8, 2, 3]);
            let frame = (size.0, size.1, format, full_range, trc);
            // the textures change about one frame in eight
            let cur = if (r >> 12).is_multiple_of(8) {
                (size.1, size.0, format, !full_range, trc)
            } else {
                frame
            };
            Frame {
                cur,
                frame,
                convert_type: pick(r >> 15, &[0, 1, 2, 3, 15]),
                texture_format: pick(r >> 18, &[4, 5, 3, 9]), // BGRX, BGRA, RGBA, RGBA16F
                flags: u8::try_from((r >> 20) % 4).unwrap(),
                gpu_conversion: !(r >> 22).is_multiple_of(4),
                has_texrender: !(r >> 24).is_multiple_of(8),
                decoupled: (r >> 27).is_multiple_of(2),
                unbuffered: (r >> 28).is_multiple_of(2),
                peak: pick(r >> 29, &[400.0, 1000.0, 1001.0, 4000.0, 10000.0]),
                white: pick(r >> 32, &[80.0, 203.0, 300.0, 480.0]),
                max_luminance: pick(r >> 35, &[0u16, 1000, 4000, 10000]),
            }
        })
        .collect()
}

fn rs_fmt(f: Fmt) -> obs_source_async_upload_format {
    obs_source_async_upload_format {
        width: f.0,
        height: f.1,
        format: f.2,
        full_range: f.3,
        trc: f.4,
    }
}

fn c_fmt(f: Fmt) -> OracleUploadFormat {
    OracleUploadFormat {
        width: f.0,
        height: f.1,
        format: f.2,
        full_range: f.3,
        trc: f.4,
    }
}

struct Side {
    texture_changed: fn(Fmt, Fmt) -> u8,
    tries_gpu: fn(c_int) -> u8,
    size_uniforms: fn(u32, u32) -> [f32; 6],
    hlg_peak: fn(f32) -> f32,
    maximum_nits: fn(u8, f32) -> f32,
    over_sdr_white: fn(f32, f32) -> f32,
    hdr_lw: fn(u16) -> f32,
    sets_range: fn(bool) -> u8,
    linear_alpha: fn(u8) -> u8,
    uses_texrender: fn(bool, bool) -> u8,
    plain: fn(c_int) -> u8,
    bgr_swap: fn(c_int, c_int) -> c_int,
    reads_unbuffered: fn(bool) -> u8,
    reanchor: fn(bool, bool) -> u8,
}

const RUST: Side = Side {
    texture_changed: |a, b| {
        u8::from(rs::obs_source_async_upload_texture_changed(
            rs_fmt(a),
            rs_fmt(b),
        ))
    },
    tries_gpu: |t| u8::from(rs::obs_source_async_upload_tries_gpu(t)),
    size_uniforms: |x, y| {
        let s = rs::obs_source_async_upload_size_uniforms(x, y);
        [
            s.width,
            s.height,
            s.width_d2,
            s.height_d2,
            s.width_x2_i,
            s.height_x2_i,
        ]
    },
    hlg_peak: |p| rs::obs_source_async_upload_hlg_peak(p),
    maximum_nits: |t, p| rs::obs_source_async_upload_maximum_nits(t, p),
    over_sdr_white: |n, w| rs::obs_source_async_upload_over_sdr_white(n, w),
    hdr_lw: |l| rs::obs_source_async_upload_hdr_lw(l),
    sets_range: |f| u8::from(rs::obs_source_async_upload_sets_range(f)),
    linear_alpha: |f| u8::from(rs::obs_source_async_upload_linear_alpha(f)),
    uses_texrender: |g, h| u8::from(rs::obs_source_async_upload_uses_texrender(g, h)),
    plain: |t| u8::from(rs::obs_source_async_upload_plain(t)),
    bgr_swap: |t, f| rs::obs_source_async_upload_bgr_swap(t, f),
    reads_unbuffered: |d| u8::from(rs::obs_source_async_upload_reanchor_reads_unbuffered(d)),
    reanchor: |d, u| u8::from(rs::obs_source_async_upload_reanchor(d, u)),
};

// SAFETY (each): the oracle functions take and return plain values.
const C: Side = Side {
    texture_changed: |a, b| unsafe {
        c::oracle_obs_source_async_upload_texture_changed(c_fmt(a), c_fmt(b))
    },
    tries_gpu: |t| unsafe { c::oracle_obs_source_async_upload_tries_gpu(t) },
    size_uniforms: |x, y| {
        let s = unsafe { c::oracle_obs_source_async_upload_size_uniforms(x, y) };
        [
            s.width,
            s.height,
            s.width_d2,
            s.height_d2,
            s.width_x2_i,
            s.height_x2_i,
        ]
    },
    hlg_peak: |p| unsafe { c::oracle_obs_source_async_upload_hlg_peak(p) },
    maximum_nits: |t, p| unsafe { c::oracle_obs_source_async_upload_maximum_nits(t, p) },
    over_sdr_white: |n, w| unsafe { c::oracle_obs_source_async_upload_over_sdr_white(n, w) },
    hdr_lw: |l| unsafe { c::oracle_obs_source_async_upload_hdr_lw(l) },
    sets_range: |f| unsafe { c::oracle_obs_source_async_upload_sets_range(f) },
    linear_alpha: |f| unsafe { c::oracle_obs_source_async_upload_linear_alpha(f) },
    uses_texrender: |g, h| unsafe { c::oracle_obs_source_async_upload_uses_texrender(g, h) },
    plain: |t| unsafe { c::oracle_obs_source_async_upload_plain(t) },
    bgr_swap: |t, f| unsafe { c::oracle_obs_source_async_upload_bgr_swap(t, f) },
    reads_unbuffered: |d| unsafe { c::oracle_obs_source_async_upload_reanchor_reads_unbuffered(d) },
    reanchor: |d, u| unsafe { c::oracle_obs_source_async_upload_reanchor(d, u) },
};

fn fold(sum: u64, value: u64) -> u64 {
    sum.wrapping_mul(31).wrapping_add(value)
}

fn frame(side: &Side, frames: &[Frame]) -> u64 {
    let mut sum = 0u64;
    for f in frames {
        let f = black_box(*f);
        sum = fold(
            sum,
            u64::try_from((side.bgr_swap)(f.texture_format, f.frame.2)).unwrap_or(0),
        );
        let unbuffered = (side.reads_unbuffered)(f.decoupled) != 0 && f.unbuffered;
        sum = fold(sum, u64::from((side.reanchor)(f.decoupled, unbuffered)));
        let changed = (side.texture_changed)(f.cur, f.frame);
        sum = fold(sum, u64::from(changed));
        if changed != 0 {
            sum = fold(sum, u64::from((side.tries_gpu)(f.convert_type)));
        }
        sum = fold(sum, u64::from((side.linear_alpha)(f.flags)));
        if (side.uses_texrender)(f.gpu_conversion, f.has_texrender) != 0 {
            sum = fold(sum, 2);
        } else {
            sum = fold(sum, u64::from((side.plain)(f.convert_type)));
        }
    }
    sum
}

fn texrender(side: &Side, frames: &[Frame]) -> u64 {
    let mut sum = 0u64;
    for f in frames {
        let f = black_box(*f);
        for u in (side.size_uniforms)(f.frame.0, f.frame.1) {
            sum = fold(sum, u64::from(u.to_bits()));
        }
        let peak = (side.hlg_peak)(f.peak);
        let nits = (side.maximum_nits)(f.frame.4, peak);
        let over = (side.over_sdr_white)(nits, f.white);
        sum = fold(sum, u64::from(peak.to_bits()));
        sum = fold(sum, u64::from(over.to_bits()));
        sum = fold(sum, u64::from((side.hdr_lw)(f.max_luminance).to_bits()));
        sum = fold(sum, u64::from((side.sets_range)(f.frame.3)));
    }
    sum
}

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let frames = frames();
    for (name, run) in [
        ("frame", frame as fn(&Side, &[Frame]) -> u64),
        ("texrender", texrender),
    ] {
        assert_eq!(
            run(&RUST, &frames),
            run(&C, &frames),
            "benchmark fixture differs: {name}"
        );
        let mut group = criterion.benchmark_group(format!("source_async_upload/{name}"));
        group.throughput(Throughput::Elements(CALLS as u64));
        group.bench_function("rust", |b| b.iter(|| black_box(run(&RUST, &frames))));
        group.bench_function("c", |b| b.iter(|| black_box(run(&C, &frames))));
        group.finish();
    }
}
