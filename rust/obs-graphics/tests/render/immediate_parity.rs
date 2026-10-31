//! Tier 3: `obs_graphics::immediate` and its C ABI against the original
//! `libobs/graphics/graphics-immediate.c.h` (the `oracle_*` build in
//! `obs-c-oracle`).
//!
//! Every call runs on the shim, the C and the core, and the results are
//! compared exactly:
//! - the layout of `struct gs_immediate_draw` and `struct gs_immediate_save`
//!   and the two constants;
//! - the push gate over counts around 0 and the immediate count, and every
//!   texture unit from -2 to 17 and at the ends of `int`, in both modes;
//! - the draw count, exhaustive over every combination of 0, 1, 2,
//!   `IMMEDIATE_COUNT - 1` and `IMMEDIATE_COUNT` for the vertex, normal,
//!   colour and texture 0 counts;
//! - the save over every pattern of empty and non-empty texture arrays
//!   (2^16), each with its arrays as long as the vertices, with one taken
//!   array shorter, and with no vertex (sampled with `parity::keep` below the
//!   full level; the patterns with at most two arrays filled, all sixteen, and
//!   alternating ones run on every level);
//! - proptests of any counts.
//!
//! Intentional differences, checked exactly where they apply:
//! 1. a texture unit outside 0-15 (the C took it and indexed past
//!    `texverts`) and units 1-15 in immediate mode (the C pushed to an
//!    array the immediate buffer never draws, and leaked it) are refused;
//! 2. the buffer's vertex count is the smallest of the vertex count and
//!    every non-empty array it takes, where the C took the vertex count and
//!    the device read past a shorter array;
//! 3. a save frees the texture arrays from the first empty one on, where
//!    the C freed none and leaked those after the first empty one.
//!
//! Everywhere else the shim, the C and the core agree.

use std::ffi::c_int;
use std::mem::{offset_of, size_of};

use obs_c_oracle::graphics_immediate::{self as oracle, OracleDraw, OracleSave, layout};
use obs_graphics::ffi::immediate::{self as shim, gs_immediate_draw, gs_immediate_save};
use obs_graphics::immediate::{self, IMMEDIATE_COUNT, TEX_ARRAYS};
use proptest::prelude::*;

fn c_unit_ok(immediate: bool, unit: c_int) -> bool {
    // SAFETY: plain values.
    unsafe { oracle::oracle_gs_immediate_unit_ok(immediate, unit) }
}

fn c_push_ok(immediate: bool, num: usize) -> bool {
    // SAFETY: plain values.
    unsafe { oracle::oracle_gs_immediate_push_ok(immediate, num) }
}

fn c_draw(verts: usize, norms: usize, colors: usize, tex0: usize) -> gs_immediate_draw {
    // SAFETY: plain values.
    let OracleDraw {
        num,
        norms_mismatch,
        colors_mismatch,
        tex_mismatch,
    } = unsafe { oracle::oracle_gs_immediate_draw(verts, norms, colors, tex0) };
    gs_immediate_draw {
        num,
        norms_mismatch,
        colors_mismatch,
        tex_mismatch,
    }
}

fn c_save(
    verts: usize,
    norms: usize,
    colors: usize,
    tex: &[usize; TEX_ARRAYS],
) -> gs_immediate_save {
    // SAFETY: tex holds GS_IMMEDIATE_TEX_ARRAYS counts.
    let OracleSave {
        num,
        num_tex,
        free_from,
    } = unsafe { oracle::oracle_gs_immediate_save(verts, norms, colors, tex.as_ptr()) };
    gs_immediate_save {
        num,
        num_tex,
        free_from,
    }
}

fn shim_save(
    verts: usize,
    norms: usize,
    colors: usize,
    tex: &[usize; TEX_ARRAYS],
) -> gs_immediate_save {
    // SAFETY: tex holds GS_IMMEDIATE_TEX_ARRAYS counts.
    unsafe { shim::gs_immediate_save(verts, norms, colors, tex.as_ptr()) }
}

fn c_layout(which: usize) -> usize {
    // SAFETY: plain value.
    unsafe { oracle::oracle_gs_immediate_layout(which) }
}

/// Intentional difference 1: the units the port refuses.
fn refused_unit(immediate: bool, unit: c_int) -> bool {
    !(0..16).contains(&unit) || (immediate && unit != 0)
}

/// Checks one unit on the shim, the core and the C.
fn check_unit(immediate: bool, unit: c_int) {
    let port = shim::gs_immediate_unit_ok(immediate, unit);
    assert_eq!(port, immediate::unit_ok(immediate, unit));
    assert!(c_unit_ok(immediate, unit), "the C takes any unit");
    assert_eq!(
        port,
        !refused_unit(immediate, unit),
        "unit {unit} {immediate}"
    );
}

/// Checks one save on the shim, the core and the C, and where they differ.
fn check_save(verts: usize, norms: usize, colors: usize, tex: &[usize; TEX_ARRAYS]) {
    let port = shim_save(verts, norms, colors, tex);
    let core = immediate::save(verts, norms, colors, tex);
    assert_eq!(
        (port.num, port.num_tex, port.free_from),
        (core.num, core.num_tex, core.free_from)
    );
    let c = c_save(verts, norms, colors, tex);
    assert_eq!(port.num_tex, c.num_tex, "{verts} {norms} {colors} {tex:?}");
    if verts == 0 {
        assert_eq!(port, c);
        return;
    }

    // Difference 2: the shortest array the buffer takes.
    let taken = [norms, colors]
        .into_iter()
        .chain(tex.iter().copied().take(c.num_tex))
        .filter(|&n| n != 0);
    let shortest = taken.clone().fold(verts, usize::min);
    assert_eq!(c.num, verts);
    assert_eq!(port.num, shortest);
    assert_eq!(port.num == c.num, taken.into_iter().all(|n| n >= verts));

    // Difference 3: the arrays it leaves.
    assert_eq!(c.free_from, TEX_ARRAYS);
    assert_eq!(port.free_from, c.num_tex);
}

#[test]
fn layout_matches_c() {
    assert_eq!(c_layout(layout::DRAW_SIZE), size_of::<gs_immediate_draw>());
    assert_eq!(
        c_layout(layout::DRAW_NUM),
        offset_of!(gs_immediate_draw, num)
    );
    assert_eq!(
        c_layout(layout::DRAW_NORMS_MISMATCH),
        offset_of!(gs_immediate_draw, norms_mismatch)
    );
    assert_eq!(
        c_layout(layout::DRAW_COLORS_MISMATCH),
        offset_of!(gs_immediate_draw, colors_mismatch)
    );
    assert_eq!(
        c_layout(layout::DRAW_TEX_MISMATCH),
        offset_of!(gs_immediate_draw, tex_mismatch)
    );
    assert_eq!(c_layout(layout::SAVE_SIZE), size_of::<gs_immediate_save>());
    assert_eq!(
        c_layout(layout::SAVE_NUM),
        offset_of!(gs_immediate_save, num)
    );
    assert_eq!(
        c_layout(layout::SAVE_NUM_TEX),
        offset_of!(gs_immediate_save, num_tex)
    );
    assert_eq!(
        c_layout(layout::SAVE_FREE_FROM),
        offset_of!(gs_immediate_save, free_from)
    );
    assert_eq!(c_layout(layout::IMMEDIATE_COUNT), IMMEDIATE_COUNT);
    assert_eq!(c_layout(layout::TEX_ARRAYS), TEX_ARRAYS);
    assert_eq!(size_of::<OracleDraw>(), size_of::<gs_immediate_draw>());
    assert_eq!(size_of::<OracleSave>(), size_of::<gs_immediate_save>());
}

#[test]
fn push_gate_matches_c() {
    let counts = (0..4)
        .chain(IMMEDIATE_COUNT - 3..IMMEDIATE_COUNT + 3)
        .chain([100_000, usize::MAX - 1, usize::MAX]);
    for num in counts {
        for imm in [false, true] {
            let port = shim::gs_immediate_push_ok(imm, num);
            assert_eq!(port, immediate::push_ok(imm, num));
            assert_eq!(port, c_push_ok(imm, num), "{num} {imm}");
        }
    }
}

#[test]
fn texture_units_match_c_except_the_unbacked_ones() {
    for unit in (-2..=17).chain([c_int::MIN, c_int::MIN + 1, c_int::MAX - 1, c_int::MAX]) {
        check_unit(false, unit);
        check_unit(true, unit);
    }
}

#[test]
fn draw_matches_c_exhaustively() {
    let counts = [0, 1, 2, IMMEDIATE_COUNT - 1, IMMEDIATE_COUNT];
    for verts in counts {
        for norms in counts {
            for colors in counts {
                for tex0 in counts {
                    let port = shim::gs_immediate_draw(verts, norms, colors, tex0);
                    let core = immediate::draw(verts, norms, colors, tex0);
                    assert_eq!(
                        (
                            port.num,
                            port.norms_mismatch,
                            port.colors_mismatch,
                            port.tex_mismatch
                        ),
                        (
                            core.num,
                            core.norms_mismatch,
                            core.colors_mismatch,
                            core.tex_mismatch
                        )
                    );
                    assert_eq!(
                        port,
                        c_draw(verts, norms, colors, tex0),
                        "{verts} {norms} {colors} {tex0}"
                    );
                }
            }
        }
    }
}

#[test]
fn save_matches_c_over_every_array_pattern() {
    let mut differs = [0_usize; 2];
    for mask in 0_u32..(1 << TEX_ARRAYS) {
        let always = mask.count_ones() <= 2 || mask == 0xffff || mask == 0x5555 || mask == 0xaaaa;
        if !always && !obs_c_oracle::parity::keep(u64::from(mask)) {
            continue;
        }
        let mut tex = [0; TEX_ARRAYS];
        for (i, t) in tex.iter_mut().enumerate() {
            if mask & (1 << i) != 0 {
                *t = 3;
            }
        }
        // As long as the vertices, with and without normals and colours.
        check_save(3, 3, 3, &tex);
        check_save(3, 0, 0, &tex);
        // Longer arrays.
        check_save(2, 3, 0, &tex);
        // No vertex.
        check_save(0, 3, 3, &tex);
        // One taken array shorter: the last leading one.
        let leading = tex.iter().take_while(|&&t| t != 0).count();
        if let Some(last) = leading.checked_sub(1) {
            let mut short = tex;
            short[last] = 1;
            check_save(3, 3, 3, &short);
            differs[0] += 1;
        }
        // A shorter array the buffer does not take: no cut.
        if let Some(after) = (leading + 1..TEX_ARRAYS).find(|&i| tex[i] != 0) {
            let mut short = tex;
            short[after] = 1;
            check_save(3, 3, 3, &short);
            assert_eq!(shim_save(3, 3, 3, &short).num, 3);
            differs[1] += 1;
        }
    }
    assert!(differs.iter().all(|&n| n > 0));

    // Shorter normals or colours, and every array at the immediate count.
    let full = [IMMEDIATE_COUNT; TEX_ARRAYS];
    check_save(IMMEDIATE_COUNT, IMMEDIATE_COUNT - 1, 0, &full);
    check_save(IMMEDIATE_COUNT, 0, 1, &full);
    check_save(IMMEDIATE_COUNT, IMMEDIATE_COUNT, IMMEDIATE_COUNT, &full);
}

#[test]
fn a_null_texture_pointer_reads_nothing() {
    // SAFETY: a null tex is allowed by the shim.
    let s = unsafe { shim::gs_immediate_save(3, 3, 3, std::ptr::null()) };
    assert_eq!((s.num, s.num_tex, s.free_from), (3, 0, 0));
}

fn count() -> impl Strategy<Value = usize> {
    prop_oneof![
        0_usize..4,
        (IMMEDIATE_COUNT - 2)..(IMMEDIATE_COUNT + 2),
        0_usize..2000,
        any::<usize>(),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(2048),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn any_counts_match_c_except_the_differences(
        verts in count(),
        norms in count(),
        colors in count(),
        tex in prop::array::uniform16(prop_oneof![2 => Just(0_usize), 3 => count()]),
        imm in any::<bool>(),
        unit in any::<c_int>(),
    ) {
        check_save(verts, norms, colors, &tex);
        check_unit(imm, unit);
        prop_assert_eq!(shim::gs_immediate_push_ok(imm, verts), c_push_ok(imm, verts));
        prop_assert_eq!(
            shim::gs_immediate_draw(verts, norms, colors, tex[0]),
            c_draw(verts, norms, colors, tex[0])
        );
    }
}
