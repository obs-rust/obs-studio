//! Tier 3: `obs_graphics::immediate` and its C ABI against the original
//! `libobs/graphics/graphics-immediate.c.h` (the `oracle_*` build in
//! `obs-c-oracle`).
//!
//! Every call runs on the shim, the C and the core, and the results are
//! compared exactly:
//! - the header's two constants, and the size and alignment of `size_t`,
//!   `bool` and `int`, the only types that cross, against `usize`, `bool`
//!   and `c_int`;
//! - the push gate over counts around 0, around the immediate count and at
//!   the top of `size_t`, and every texture unit from -2 to 17 and at the ends
//!   of `int`, in both modes;
//! - the stop's mismatch and cut, exhaustive over 0, 1, 2,
//!   `IMMEDIATE_COUNT - 1`, `IMMEDIATE_COUNT` and the top of `size_t` for the
//!   counts, and whole stops as `gs_render_stop()` asks them over every
//!   combination of those for the vertex, normal, colour and texture 0
//!   counts;
//! - the save's cut over the same counts, its free start for 0 to 16
//!   texture arrays, and whole saves as `gs_render_save()` asks them over
//!   every pattern of empty and non-empty texture arrays (2^16): arrays as
//!   long as the vertices, longer, the last taken one shorter, one shorter
//!   after the first empty one; sampled by `parity::keep` below the full
//!   level, with the patterns of at most two arrays, all 16 and the two
//!   alternating ones on every level;
//! - proptests of any counts and units.
//!
//! Intentional differences, checked exactly where they apply:
//! 1. a texture unit outside 0-15 (the C took it and indexed past
//!    `texverts`) and units 1-15 in immediate mode (the C pushed to an
//!    array the immediate buffer never draws, and leaked it) are refused;
//! 2. the save cuts the buffer's vertex count to a shorter non-empty array it
//!    takes, where the C kept the vertex count and the device read past the
//!    shorter array;
//! 3. a save frees the texture arrays from the first one it does not take,
//!    where the C freed none and leaked those after the first empty one.
//!
//! Everywhere else the shim, the C and the core agree.

use std::ffi::c_int;
use std::mem::{align_of, size_of};

use obs_c_oracle::graphics_immediate::{self as oracle, layout};
use obs_graphics::ffi::immediate as shim;
use obs_graphics::immediate::{self, IMMEDIATE_COUNT, TEX_ARRAYS};
use proptest::prelude::*;

/// The counts where every compare turns.
const COUNTS: [usize; 6] = [0, 1, 2, IMMEDIATE_COUNT - 1, IMMEDIATE_COUNT, usize::MAX];

fn c_layout(which: usize) -> usize {
    // SAFETY: plain value.
    unsafe { oracle::oracle_gs_immediate_layout(which) }
}

fn c_unit_ok(immediate: bool, unit: c_int) -> bool {
    // SAFETY: plain values.
    unsafe { oracle::oracle_gs_immediate_unit_ok(immediate, unit) }
}

fn c_push_ok(immediate: bool, num: usize) -> bool {
    // SAFETY: plain values.
    unsafe { oracle::oracle_gs_immediate_push_ok(immediate, num) }
}

fn c_mismatch(count: usize, verts: usize) -> bool {
    // SAFETY: plain values.
    unsafe { oracle::oracle_gs_immediate_mismatch(count, verts) }
}

fn c_cut(num: usize, count: usize) -> usize {
    // SAFETY: plain values.
    unsafe { oracle::oracle_gs_immediate_cut(num, count) }
}

fn c_save_cut(num: usize, count: usize) -> usize {
    // SAFETY: plain values.
    unsafe { oracle::oracle_gs_immediate_save_cut(num, count) }
}

fn c_save_free_from(num_tex: usize) -> usize {
    // SAFETY: plain value.
    unsafe { oracle::oracle_gs_immediate_save_free_from(num_tex) }
}

/// One side's six decisions.
struct Side {
    mismatch: fn(usize, usize) -> bool,
    cut: fn(usize, usize) -> usize,
    save_cut: fn(usize, usize) -> usize,
    save_free_from: fn(usize) -> usize,
}

const C: Side = Side {
    mismatch: c_mismatch,
    cut: c_cut,
    save_cut: c_save_cut,
    save_free_from: c_save_free_from,
};

const SHIM: Side = Side {
    mismatch: |count, verts| shim::gs_immediate_mismatch(count, verts),
    cut: |num, count| shim::gs_immediate_cut(num, count),
    save_cut: |num, count| shim::gs_immediate_save_cut(num, count),
    save_free_from: |num_tex| shim::gs_immediate_save_free_from(num_tex),
};

/// `gs_render_stop()`'s draw count and which arrays log, as it asks.
fn stop(side: &Side, verts: usize, counts: [usize; 3]) -> (usize, [bool; 3]) {
    let mut num = verts;
    let mut logs = [false; 3];
    if num == 0 {
        return (0, logs);
    }
    for (log, count) in logs.iter_mut().zip(counts) {
        if (side.mismatch)(count, verts) {
            *log = true;
            num = (side.cut)(num, count);
        }
    }
    (num, logs)
}

/// `gs_render_save()`'s buffer, as it asks: the vertex count, the texture
/// arrays it takes and the first one freed. `None` with no vertex.
fn save(
    side: &Side,
    verts: usize,
    norms: usize,
    colors: usize,
    tex: &[usize; TEX_ARRAYS],
) -> Option<(usize, usize, usize)> {
    if verts == 0 {
        return None;
    }
    let num_tex = tex.iter().take_while(|&&t| t != 0).count();
    let mut num = (side.save_cut)((side.save_cut)(verts, norms), colors);
    for &t in tex.iter().take(num_tex) {
        num = (side.save_cut)(num, t);
    }
    Some((num, num_tex, (side.save_free_from)(num_tex)))
}

/// Intentional difference 1: the units the port refuses.
fn refused_unit(immediate: bool, unit: c_int) -> bool {
    !(0..16).contains(&unit) || (immediate && unit != 0)
}

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

/// Checks one whole save on the shim and the C, and where they differ.
fn check_save(verts: usize, norms: usize, colors: usize, tex: &[usize; TEX_ARRAYS]) {
    let port = save(&SHIM, verts, norms, colors, tex);
    let c = save(&C, verts, norms, colors, tex);
    let (Some((num, num_tex, free)), Some((c_num, c_num_tex, c_free))) = (port, c) else {
        assert_eq!((port, c), (None, None));
        return;
    };
    assert_eq!(num_tex, c_num_tex);

    // Difference 2: the shortest array the buffer takes.
    let taken = [norms, colors]
        .into_iter()
        .chain(tex.iter().copied().take(num_tex))
        .filter(|&n| n != 0);
    assert_eq!(c_num, verts);
    assert_eq!(num, taken.clone().fold(verts, usize::min));
    assert_eq!(num == c_num, taken.into_iter().all(|n| n >= verts));

    // Difference 3: the arrays it leaves.
    assert_eq!(c_free, TEX_ARRAYS);
    assert_eq!(free, num_tex);
}

#[test]
fn layout_matches_c() {
    assert_eq!(c_layout(layout::IMMEDIATE_COUNT), IMMEDIATE_COUNT);
    assert_eq!(c_layout(layout::TEX_ARRAYS), TEX_ARRAYS);
    assert_eq!(c_layout(layout::SIZE_T_SIZE), size_of::<usize>());
    assert_eq!(c_layout(layout::SIZE_T_ALIGN), align_of::<usize>());
    assert_eq!(c_layout(layout::BOOL_SIZE), size_of::<bool>());
    assert_eq!(c_layout(layout::BOOL_ALIGN), align_of::<bool>());
    assert_eq!(c_layout(layout::INT_SIZE), size_of::<c_int>());
    assert_eq!(c_layout(layout::INT_ALIGN), align_of::<c_int>());
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
fn stop_matches_c_exhaustively() {
    for a in COUNTS {
        for b in COUNTS {
            assert_eq!(shim::gs_immediate_mismatch(a, b), c_mismatch(a, b));
            assert_eq!(shim::gs_immediate_mismatch(a, b), immediate::mismatch(a, b));
            assert_eq!(shim::gs_immediate_cut(a, b), c_cut(a, b));
            assert_eq!(shim::gs_immediate_cut(a, b), immediate::cut(a, b));
        }
    }
    for verts in COUNTS {
        for norms in COUNTS {
            for colors in COUNTS {
                for tex0 in COUNTS {
                    let counts = [norms, colors, tex0];
                    assert_eq!(stop(&SHIM, verts, counts), stop(&C, verts, counts));
                }
            }
        }
    }
}

#[test]
fn save_decisions_match_c_except_the_differences() {
    for num in COUNTS {
        for count in COUNTS {
            let port = shim::gs_immediate_save_cut(num, count);
            assert_eq!(port, immediate::save_cut(num, count));
            assert_eq!(c_save_cut(num, count), num);
            assert_eq!(port, if count == 0 { num } else { num.min(count) });
        }
    }
    for num_tex in 0..=TEX_ARRAYS {
        let port = shim::gs_immediate_save_free_from(num_tex);
        assert_eq!(port, immediate::save_free_from(num_tex));
        assert_eq!(c_save_free_from(num_tex), TEX_ARRAYS);
        assert_eq!(port, num_tex);
    }
}

#[test]
fn whole_saves_match_c_over_every_array_pattern() {
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
        check_save(3, 3, 3, &tex);
        check_save(3, 0, 0, &tex);
        check_save(2, 3, 0, &tex);
        check_save(0, 3, 3, &tex);
        let leading = tex.iter().take_while(|&&t| t != 0).count();
        if let Some(last) = leading.checked_sub(1) {
            let mut short = tex;
            short[last] = 1;
            check_save(3, 3, 3, &short);
            differs[0] += 1;
        }
        if let Some(after) = (leading + 1..TEX_ARRAYS).find(|&i| tex[i] != 0) {
            let mut short = tex;
            short[after] = 1;
            check_save(3, 3, 3, &short);
            assert_eq!(save(&SHIM, 3, 3, 3, &short).map(|s| s.0), Some(3));
            differs[1] += 1;
        }
    }
    assert!(differs.iter().all(|&n| n > 0));

    let full = [IMMEDIATE_COUNT; TEX_ARRAYS];
    check_save(IMMEDIATE_COUNT, IMMEDIATE_COUNT - 1, 0, &full);
    check_save(IMMEDIATE_COUNT, 0, 1, &full);
    check_save(IMMEDIATE_COUNT, IMMEDIATE_COUNT, IMMEDIATE_COUNT, &full);
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
        let counts = [norms, colors, tex[0]];
        prop_assert_eq!(stop(&SHIM, verts, counts), stop(&C, verts, counts));
    }
}
