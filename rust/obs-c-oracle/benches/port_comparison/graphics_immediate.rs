//! obs-graphics: the immediate-mode decisions
//! (`libobs/graphics/graphics-immediate.c.h`). Every vertex, normal, colour
//! and texture coordinate a plugin pushes in `gs_render_start` mode asks the
//! push gate, each stop asks whether its arrays match, and each save asks for
//! its vertex count. Each side runs a frame of 100 immediate-mode quads (four
//! vertices, four colours and four texture coordinates on unit 0 each, then a
//! stop) and 20 saved buffers of 64 vertices with two texture arrays, through
//! the decisions as `graphics.c` asks them, and folds the answers into a
//! checksum both must match. The counts all match, where the port keeps the
//! C's answers; the save's free start is left out, as the port frees the
//! arrays the C leaked. Nothing is allocated in the loop.

use super::pair;

use criterion::Criterion;
use obs_c_oracle::graphics_immediate::{
    oracle_gs_immediate_cut, oracle_gs_immediate_mismatch, oracle_gs_immediate_push_ok,
    oracle_gs_immediate_save_cut, oracle_gs_immediate_unit_ok,
};
use obs_graphics::ffi::immediate::{
    gs_immediate_cut, gs_immediate_mismatch, gs_immediate_push_ok, gs_immediate_save_cut,
    gs_immediate_unit_ok,
};
use std::hint::black_box;

const QUADS: usize = 100;
const SAVES: usize = 20;
const SAVED: usize = 64;

fn fold(acc: u64, value: usize) -> u64 {
    acc.wrapping_mul(31).wrapping_add(value as u64)
}

/// The frame, from the five functions of one side.
macro_rules! frame {
    ($unit_ok:expr, $push_ok:expr, $mismatch:expr, $cut:expr, $save_cut:expr) => {{
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
            let mut num = verts;
            for count in [0, colors, tex0] {
                if $mismatch(count, verts) {
                    num = $cut(num, count);
                }
            }
            acc = fold(acc, num);
        }
        for _ in 0..SAVES {
            let mut verts = 0;
            for _ in 0..SAVED {
                if $push_ok(black_box(false), verts) {
                    verts += 1;
                }
            }
            let mut num = $save_cut($save_cut(verts, verts), 0);
            for _ in 0..2 {
                num = $save_cut(num, verts);
            }
            acc = fold(acc, num);
        }
        acc
    }};
}

fn rust_frame() -> u64 {
    frame!(
        gs_immediate_unit_ok,
        gs_immediate_push_ok,
        gs_immediate_mismatch,
        gs_immediate_cut,
        gs_immediate_save_cut
    )
}

fn c_frame() -> u64 {
    // SAFETY: plain values; every input is defined in the C.
    unsafe {
        frame!(
            oracle_gs_immediate_unit_ok,
            oracle_gs_immediate_push_ok,
            oracle_gs_immediate_mismatch,
            oracle_gs_immediate_cut,
            oracle_gs_immediate_save_cut
        )
    }
}

pub fn benchmarks(criterion: &mut Criterion) {
    pair(criterion, "graphics_immediate/frame/100_quads", rust_frame, c_frame);
}
