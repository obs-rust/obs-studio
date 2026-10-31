//! obs-graphics: the immediate-mode decisions
//! (`libobs/graphics/graphics-immediate.c.h`). Every vertex, normal, colour
//! and texture coordinate a plugin pushes in `gs_render_start` mode asks the
//! push gate, and each stop and save asks for its draw count or buffer.
//! Each side runs a frame of 100 immediate-mode quads (four vertices, four
//! colours and four texture coordinates on unit 0 each, then a stop) and 20
//! saved buffers of 64 vertices with two texture arrays, through the
//! decisions as `graphics.c` asks them, and folds the answers into a
//! checksum both must match. The counts all match, where the port keeps the
//! C's answers. Nothing is allocated in the loop.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::graphics_immediate::{
    oracle_gs_immediate_draw, oracle_gs_immediate_push_ok, oracle_gs_immediate_save,
    oracle_gs_immediate_unit_ok,
};
use obs_graphics::ffi::immediate::{
    gs_immediate_draw, gs_immediate_push_ok, gs_immediate_save, gs_immediate_unit_ok,
};
use std::hint::black_box;

const QUADS: usize = 100;
const SAVES: usize = 20;
const SAVED: usize = 64;

fn fold(acc: u64, value: usize) -> u64 {
    acc.wrapping_mul(31).wrapping_add(value as u64)
}

/// The frame, from the four functions of one side. The save's free start is
/// left out: the port frees the arrays the C leaked.
macro_rules! frame {
    ($unit_ok:expr, $push_ok:expr, $draw:expr, $save:expr) => {{
        let mut acc = 0;
        for _ in 0..QUADS {
            let (mut verts, mut colors, mut tex0) = (0, 0, 0);
            for _ in 0..4 {
                let immediate = black_box(true);
                if $push_ok(immediate, verts) {
                    verts += 1;
                }
                if $push_ok(immediate, colors) {
                    colors += 1;
                }
                if $unit_ok(immediate, black_box(0)) && $push_ok(immediate, tex0) {
                    tex0 += 1;
                }
            }
            let draw = $draw(verts, 0, colors, tex0);
            acc = fold(acc, draw.num);
            acc = fold(acc, usize::from(draw.colors_mismatch));
        }
        for _ in 0..SAVES {
            let mut verts = 0;
            for _ in 0..SAVED {
                if $push_ok(black_box(false), verts) {
                    verts += 1;
                }
            }
            let mut tex = [0_usize; 16];
            tex[0] = verts;
            tex[1] = verts;
            let save = $save(verts, verts, 0, tex.as_ptr());
            acc = fold(fold(acc, save.num), save.num_tex);
        }
        acc
    }};
}

fn rust_frame() -> u64 {
    // SAFETY: tex holds 16 counts.
    unsafe {
        frame!(
            gs_immediate_unit_ok,
            gs_immediate_push_ok,
            gs_immediate_draw,
            gs_immediate_save
        )
    }
}

fn c_frame() -> u64 {
    // SAFETY: plain values; tex holds 16 counts.
    unsafe {
        frame!(
            oracle_gs_immediate_unit_ok,
            oracle_gs_immediate_push_ok,
            oracle_gs_immediate_draw,
            oracle_gs_immediate_save
        )
    }
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "graphics_immediate/frame/100_quads", rust_frame, c_frame);
}
