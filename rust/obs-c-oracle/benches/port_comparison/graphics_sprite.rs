//! obs-graphics: the sprite and texture upload geometry
//! (`libobs/graphics/graphics-sprite.c.h`), which `graphics.c` runs on every
//! `gs_draw_sprite_subregion()`, every rectangle texture drawn by
//! `gs_draw_quadf()`, every `gs_texture_set_image()` and every
//! `gs_texture_create()`. Each measurement runs 1024 calls over sizes,
//! regions and flips from a fixed generator (capture-like sizes, crops and
//! line sizes) and folds every output bit into a checksum both sides must
//! match:
//! - `subregion_sprite`: the subregion coordinates and the sprite built
//!   from them into a vertex buffer's arrays (`gs_draw_sprite_subregion`);
//! - `rect_sprite`: the texel coordinates and the 1x1 sprite
//!   (`gs_draw_quadf` of a rectangle texture);
//! - `upload_plan`: the copy plan, flipped and not, equal and padded line
//!   sizes;
//! - `mip_plan`: the mipmap rules over power-of-two and other sizes, flags
//!   and levels.
//!
//! Measured when it was added (x86_64 Linux, GCC, not pinned to a core, a
//! heavily loaded host, load average about 40; median per 1024 calls):
//! subregion_sprite 50.6 us Rust, 37.1 us C (1.36x); rect_sprite 42.5 us
//! Rust, 55.4 us C (0.77x); upload_plan 8.73 us Rust, 6.49 us C (1.35x);
//! mip_plan 5.99 us Rust, 13.4 us C (0.45x). Each is tens of nanoseconds
//! per call at most, once per draw, upload or texture creation; on this
//! host the spread between runs is as large as the difference.

use std::hint::black_box;

use criterion::{Criterion, Throughput};
use obs_c_oracle::graphics_math::OracleVec3;
use obs_c_oracle::graphics_sprite::{self as c, OracleSpriteUv};
use obs_c_oracle::vec2::OracleVec2;
use obs_graphics::ffi::sprite::{self as rs, gs_sprite_uv};
use obs_graphics::ffi::vec2::vec2;
use obs_graphics::ffi::vec3::vec3;

const CALLS: usize = 1024;

/// A small deterministic generator, so both sides see the same calls.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        u32::try_from(self.0 >> 33).unwrap()
    }

    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

const WIDTHS: [u32; 6] = [1280, 1920, 2560, 3840, 1366, 7680];
const HEIGHTS: [u32; 6] = [720, 1080, 1440, 2160, 768, 4320];

#[derive(Clone, Copy)]
struct Call {
    region: [u32; 6],
    flip: u32,
    linesize: u32,
    linesize_out: u32,
    height: usize,
    upload_flip: bool,
    mip_size: [u32; 2],
    flags: u32,
    levels: u32,
}

fn calls() -> Vec<Call> {
    let mut rng = Lcg(2082);
    (0..CALLS)
        .map(|_| {
            let k = usize::try_from(rng.below(6)).unwrap();
            let (w, h) = (WIDTHS[k], HEIGHTS[k]);
            let x = rng.below(w / 4);
            let y = rng.below(h / 4);
            let pad = [0, 0, 32, 64][usize::try_from(rng.below(4)).unwrap()];
            let pow = 1u32 << (rng.below(12) + 1);
            let (tw, th) = if rng.below(3) == 0 {
                (pow, pow)
            } else {
                (w, h)
            };
            Call {
                region: [
                    x,
                    y,
                    w - x - rng.below(w / 4),
                    h - y - rng.below(h / 4),
                    w,
                    h,
                ],
                flip: rng.below(4),
                linesize: w * 4,
                linesize_out: w * 4 + pad,
                height: usize::try_from(h).unwrap(),
                upload_flip: rng.below(5) == 0,
                flags: [0, 1, 4, 5, 2][usize::try_from(rng.below(5)).unwrap()]
                    | (rng.below(2) << 5),
                mip_size: [tw, th],
                levels: [1, 1, 0, 5][usize::try_from(rng.below(4)).unwrap()],
            }
        })
        .collect()
}

fn fold(sum: u64, v: u64) -> u64 {
    sum.wrapping_mul(31).wrapping_add(v)
}

fn fold_rust_build(mut sum: u64, points: &[vec3; 4], uvs: &[vec2; 4]) -> u64 {
    for p in points {
        for f in [p.x, p.y, p.z, p.w] {
            sum = fold(sum, u64::from(f.to_bits()));
        }
    }
    for v in uvs {
        sum = fold(sum, u64::from(v.x.to_bits()));
        sum = fold(sum, u64::from(v.y.to_bits()));
    }
    sum
}

fn fold_c_build(mut sum: u64, points: &[OracleVec3; 4], uvs: &[OracleVec2; 4]) -> u64 {
    for p in points {
        for f in [p.x, p.y, p.z, p.w] {
            sum = fold(sum, u64::from(f.to_bits()));
        }
    }
    for v in uvs {
        sum = fold(sum, u64::from(v.x.to_bits()));
        sum = fold(sum, u64::from(v.y.to_bits()));
    }
    sum
}

type Side = fn(&[Call]) -> u64;

const RUST_SUBREGION: Side = |calls| {
    let mut points = [vec3::default(); 4];
    let mut uvs = [vec2::default(); 4];
    let mut sum = 0;
    for call in calls {
        let [x, y, cx, cy, tw, th] = black_box(call.region);
        let uv = rs::gs_sprite_uv_subregion(x, y, cx, cy, tw, th, call.flip);
        #[allow(clippy::cast_precision_loss)]
        // SAFETY: four writable vec3 and vec2.
        unsafe {
            rs::gs_sprite_build(
                points.as_mut_ptr(),
                uvs.as_mut_ptr(),
                cx as f32,
                cy as f32,
                uv,
            );
        }
        sum = fold_rust_build(sum, &points, &uvs);
    }
    sum
};

const C_SUBREGION: Side = |calls| {
    let mut points = [OracleVec3::default(); 4];
    let mut uvs = [OracleVec2::default(); 4];
    let mut sum = 0;
    for call in calls {
        let [x, y, cx, cy, tw, th] = black_box(call.region);
        // SAFETY: plain values; four writable vec3 and vec2.
        #[allow(clippy::cast_precision_loss)]
        unsafe {
            let uv = c::oracle_gs_sprite_uv_subregion(x, y, cx, cy, tw, th, call.flip);
            c::oracle_gs_sprite_build(
                points.as_mut_ptr(),
                uvs.as_mut_ptr(),
                cx as f32,
                cy as f32,
                uv,
            );
        }
        sum = fold_c_build(sum, &points, &uvs);
    }
    sum
};

const RUST_RECT: Side = |calls| {
    let mut points = [vec3::default(); 4];
    let mut uvs = [vec2::default(); 4];
    let mut sum = 0;
    for call in calls {
        let [_, _, _, _, tw, th] = black_box(call.region);
        let uv: gs_sprite_uv = rs::gs_sprite_uv_rect(tw, th, call.flip);
        // SAFETY: four writable vec3 and vec2.
        unsafe { rs::gs_sprite_build(points.as_mut_ptr(), uvs.as_mut_ptr(), 1.0, 1.0, uv) };
        sum = fold_rust_build(sum, &points, &uvs);
    }
    sum
};

const C_RECT: Side = |calls| {
    let mut points = [OracleVec3::default(); 4];
    let mut uvs = [OracleVec2::default(); 4];
    let mut sum = 0;
    for call in calls {
        let [_, _, _, _, tw, th] = black_box(call.region);
        // SAFETY: plain values; four writable vec3 and vec2.
        unsafe {
            let uv: OracleSpriteUv = c::oracle_gs_sprite_uv_rect(tw, th, call.flip);
            c::oracle_gs_sprite_build(points.as_mut_ptr(), uvs.as_mut_ptr(), 1.0, 1.0, uv);
        }
        sum = fold_c_build(sum, &points, &uvs);
    }
    sum
};

fn usize_bits(v: usize) -> u64 {
    u64::try_from(v).unwrap()
}

const RUST_UPLOAD: Side = |calls| {
    let mut sum = 0;
    for call in calls {
        let p = rs::gs_texture_upload_plan(
            black_box(call.linesize),
            black_box(call.linesize_out),
            black_box(call.height),
            call.upload_flip,
        );
        sum = fold(sum, usize_bits(p.row_copy));
        sum = fold(sum, usize_bits(p.whole_size));
        sum = fold(sum, usize_bits(p.src_offset));
        sum = fold(sum, u64::from(p.mode.unsigned_abs()));
    }
    sum
};

const C_UPLOAD: Side = |calls| {
    let mut sum = 0;
    for call in calls {
        // SAFETY: plain values.
        let p = unsafe {
            c::oracle_gs_texture_upload_plan(
                black_box(call.linesize),
                black_box(call.linesize_out),
                black_box(call.height),
                call.upload_flip,
            )
        };
        sum = fold(sum, usize_bits(p.row_copy));
        sum = fold(sum, usize_bits(p.whole_size));
        sum = fold(sum, usize_bits(p.src_offset));
        sum = fold(sum, u64::from(p.mode.unsigned_abs()));
    }
    sum
};

const RUST_MIP: Side = |calls| {
    let mut sum = 0;
    for call in calls {
        let [tw, th] = black_box(call.mip_size);
        let p = rs::gs_texture_mip_plan(tw, th, black_box(call.flags), black_box(call.levels));
        sum = fold(sum, u64::from(p.flags));
        sum = fold(sum, u64::from(p.levels));
        sum = fold(sum, u64::from(p.warning.unsigned_abs()));
    }
    sum
};

const C_MIP: Side = |calls| {
    let mut sum = 0;
    for call in calls {
        let [tw, th] = black_box(call.mip_size);
        // SAFETY: plain values.
        let p = unsafe {
            c::oracle_gs_texture_mip_plan(tw, th, black_box(call.flags), black_box(call.levels))
        };
        sum = fold(sum, u64::from(p.flags));
        sum = fold(sum, u64::from(p.levels));
        sum = fold(sum, u64::from(p.warning.unsigned_abs()));
    }
    sum
};

pub(crate) fn benchmarks(criterion: &mut Criterion) {
    let calls = calls();
    for (name, rust, c) in [
        ("subregion_sprite", RUST_SUBREGION, C_SUBREGION),
        ("rect_sprite", RUST_RECT, C_RECT),
        ("upload_plan", RUST_UPLOAD, C_UPLOAD),
        ("mip_plan", RUST_MIP, C_MIP),
    ] {
        assert_eq!(rust(&calls), c(&calls), "benchmark fixture differs: {name}");
        let mut group = criterion.benchmark_group(format!("graphics_sprite/{name}"));
        group.throughput(Throughput::Elements(CALLS as u64));
        group.bench_function("rust", |b| b.iter(|| black_box(rust(&calls))));
        group.bench_function("c", |b| b.iter(|| black_box(c(&calls))));
        group.finish();
    }
}
