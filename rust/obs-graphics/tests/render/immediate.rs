//! Tier 1: the safe core of `libobs/graphics/graphics-immediate.c.h`.
//!
//! `steps_*` mirror `test/cmocka/test_graphics_immediate_steps.c` one to
//! one (same names, values and expectations). `edge_*` cover what the C
//! test leaves out, the intentional differences (a texture unit outside
//! 0-15 or above 0 in immediate mode, a save with a shorter array, the
//! arrays a save leaves), and proptests of every decision against a model.

use obs_graphics::immediate::{
    self, Draw, IMMEDIATE_COUNT, Save, TEX_ARRAYS, draw, push_ok, unit_ok,
};
use proptest::prelude::*;

/// The cmocka `save()`: `filled` leading texture arrays of `verts`
/// coordinates.
fn save(verts: usize, norms: usize, colors: usize, filled: usize) -> Save {
    let mut tex = [0; TEX_ARRAYS];
    for t in tex.iter_mut().take(filled) {
        *t = verts;
    }
    immediate::save(verts, norms, colors, &tex)
}

fn check_draw(d: Draw, num: usize, norms: bool, colors: bool, tex: bool) {
    assert_eq!(
        d,
        Draw {
            num,
            norms_mismatch: norms,
            colors_mismatch: colors,
            tex_mismatch: tex,
        }
    );
}

#[test]
fn steps_an_empty_start_and_stop() {
    check_draw(draw(0, 0, 0, 0), 0, false, false, false);
    // Characterized, not endorsed: no other array is looked at.
    check_draw(draw(0, 3, 2, 1), 0, false, false, false);

    let s = save(0, 0, 0, 0);
    assert_eq!((s.num, s.num_tex), (0, 0));
    let mut tex = [0; TEX_ARRAYS];
    tex[0] = 1;
    tex[1] = 1;
    let s = immediate::save(0, 3, 2, &tex);
    assert_eq!((s.num, s.num_tex), (0, 0));
}

#[test]
fn steps_matching_counts_draw_every_vertex() {
    check_draw(draw(1, 0, 0, 0), 1, false, false, false);
    check_draw(draw(4, 4, 4, 4), 4, false, false, false);
    check_draw(draw(4, 0, 4, 0), 4, false, false, false);
    check_draw(
        draw(
            IMMEDIATE_COUNT,
            IMMEDIATE_COUNT,
            IMMEDIATE_COUNT,
            IMMEDIATE_COUNT,
        ),
        IMMEDIATE_COUNT,
        false,
        false,
        false,
    );

    let s = save(1, 1, 1, 1);
    assert_eq!((s.num, s.num_tex), (1, 1));
    let s = save(4, 4, 4, 1);
    assert_eq!((s.num, s.num_tex), (4, 1));
    let s = save(7, 0, 0, 0);
    assert_eq!((s.num, s.num_tex), (7, 0));
}

#[test]
fn steps_a_shorter_array_cuts_the_draw() {
    check_draw(draw(4, 3, 4, 4), 3, true, false, false);
    check_draw(draw(4, 4, 2, 4), 2, false, true, false);
    check_draw(draw(4, 4, 4, 1), 1, false, false, true);
    check_draw(draw(5, 4, 3, 2), 2, true, true, true);
    check_draw(draw(5, 2, 3, 4), 2, true, true, true);
    check_draw(draw(5, 0, 3, 0), 3, false, true, false);
}

#[test]
fn steps_a_longer_array_logs_without_a_cut() {
    // Characterized, not endorsed.
    check_draw(draw(4, 6, 4, 4), 4, true, false, false);
    check_draw(draw(4, 4, 5, 4), 4, false, true, false);
    check_draw(draw(4, 4, 4, 9), 4, false, false, true);
    check_draw(draw(4, 9, 2, 6), 2, true, true, true);
}

#[test]
fn steps_the_immediate_cap() {
    assert!(push_ok(true, 0));
    assert!(push_ok(true, 1));
    assert!(push_ok(true, IMMEDIATE_COUNT - 1));
    assert!(!push_ok(true, IMMEDIATE_COUNT));

    assert!(push_ok(false, 0));
    assert!(push_ok(false, IMMEDIATE_COUNT - 1));
    assert!(push_ok(false, IMMEDIATE_COUNT));
    assert!(push_ok(false, IMMEDIATE_COUNT + 1));
    assert!(push_ok(false, 100_000));
    assert!(push_ok(false, usize::MAX));
}

#[test]
fn steps_the_backed_texture_units() {
    assert!(unit_ok(true, 0));
    for unit in 0..16 {
        assert!(unit_ok(false, unit));
    }
}

#[test]
fn steps_the_texture_array_count() {
    let s = save(3, 3, 3, 3);
    assert_eq!((s.num, s.num_tex), (3, 3));

    let s = save(3, 0, 0, TEX_ARRAYS);
    assert_eq!(s.num_tex, TEX_ARRAYS);

    // Characterized, not endorsed: array 1 is set and not drawn.
    let mut tex = [0; TEX_ARRAYS];
    tex[1] = 3;
    let s = immediate::save(3, 3, 3, &tex);
    assert_eq!((s.num, s.num_tex), (3, 0));

    tex[0] = 3;
    tex[3] = 3;
    assert_eq!(immediate::save(3, 0, 3, &tex).num_tex, 2);

    tex[2] = 1;
    assert_eq!(immediate::save(3, 0, 3, &tex).num_tex, 4);
}

#[test]
fn steps_the_save_frees_no_array_it_takes() {
    for filled in 0..=TEX_ARRAYS {
        let s = save(2, 2, 2, filled);
        assert_eq!(s.num_tex, filled);
        assert!(s.free_from >= s.num_tex);
        assert!(s.free_from <= TEX_ARRAYS);
    }
}

/* ---------------------------------------------------------------------- */
/* What the C test leaves out: the intentional differences */

#[test]
fn edge_a_unit_outside_the_arrays_is_refused() {
    for unit in [i32::MIN, -17, -2, -1, 16, 17, 1000, i32::MAX] {
        assert!(!unit_ok(false, unit), "unit {unit}");
        assert!(!unit_ok(true, unit), "immediate unit {unit}");
    }
}

#[test]
fn edge_immediate_mode_backs_only_unit_0() {
    assert!(unit_ok(true, 0));
    for unit in 1..16 {
        assert!(!unit_ok(true, unit), "unit {unit}");
    }
}

#[test]
fn edge_the_buffer_takes_the_shortest_array() {
    // A shorter normal, colour or texture array: the buffer stops there.
    assert_eq!(save(4, 3, 4, 0).num, 3);
    assert_eq!(save(4, 4, 2, 0).num, 2);
    assert_eq!(save(4, 0, 0, 0).num, 4);

    let mut tex = [0; TEX_ARRAYS];
    tex[0] = 4;
    tex[1] = 1;
    assert_eq!(immediate::save(4, 4, 4, &tex).num, 1);
    tex[1] = 4;
    tex[15] = 2;
    // Array 15 is after the first empty one: the buffer does not take it.
    let s = immediate::save(4, 4, 4, &tex);
    assert_eq!((s.num, s.num_tex), (4, 2));

    // Longer arrays cut nothing.
    assert_eq!(save(4, 9, 9, 1).num, 4);
    let mut long = [9; TEX_ARRAYS];
    long[0] = 4;
    assert_eq!(immediate::save(4, 0, 0, &long).num, 4);

    // Every array at the immediate count, and one short of it.
    let full = [IMMEDIATE_COUNT; TEX_ARRAYS];
    assert_eq!(
        immediate::save(IMMEDIATE_COUNT, IMMEDIATE_COUNT, IMMEDIATE_COUNT, &full).num,
        IMMEDIATE_COUNT
    );
    let mut short = full;
    short[15] = IMMEDIATE_COUNT - 1;
    assert_eq!(
        immediate::save(IMMEDIATE_COUNT, 0, 0, &short).num,
        IMMEDIATE_COUNT - 1
    );
}

#[test]
fn edge_the_save_frees_the_arrays_it_leaves() {
    // From the first empty array on; with all 16 taken, none.
    for filled in 0..=TEX_ARRAYS {
        let s = save(2, 2, 2, filled);
        assert_eq!(s.free_from, filled);
    }

    // Array 0 empty and array 1 filled: array 1 is freed.
    let mut tex = [0; TEX_ARRAYS];
    tex[1] = 3;
    assert_eq!(immediate::save(3, 3, 3, &tex).free_from, 0);

    // No vertex: no buffer, and graphics.c frees every array itself.
    let s = save(0, 1, 1, 3);
    assert_eq!(
        s,
        Save {
            num: 0,
            num_tex: 0,
            free_from: TEX_ARRAYS,
        }
    );
}

/* ---------------------------------------------------------------------- */
/* Properties */

/// A count: around 0, around the immediate count, or any.
fn count() -> impl Strategy<Value = usize> {
    prop_oneof![
        0_usize..4,
        (IMMEDIATE_COUNT - 2)..(IMMEDIATE_COUNT + 2),
        0_usize..2000,
        any::<usize>(),
    ]
}

fn tex_counts() -> impl Strategy<Value = [usize; TEX_ARRAYS]> {
    prop::array::uniform16(prop_oneof![2 => Just(0_usize), 3 => count()])
}

/// The C's draw count, without its early return.
fn model_draw(verts: usize, norms: usize, colors: usize, tex0: usize) -> Draw {
    if verts == 0 {
        return Draw::default();
    }
    let mut d = Draw {
        num: verts,
        ..Draw::default()
    };
    if norms != 0 && norms != verts {
        d.norms_mismatch = true;
        d.num = d.num.min(norms);
    }
    if colors != 0 && colors != verts {
        d.colors_mismatch = true;
        d.num = d.num.min(colors);
    }
    if tex0 != 0 && tex0 != verts {
        d.tex_mismatch = true;
        d.num = d.num.min(tex0);
    }
    d
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: obs_c_oracle::parity::cases(2048),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn edge_unit_ok_is_the_backed_arrays(immediate in any::<bool>(), unit in any::<i32>()) {
        let backed = if immediate { unit == 0 } else { (0..16).contains(&unit) };
        prop_assert_eq!(unit_ok(immediate, unit), backed);
    }

    #[test]
    fn edge_push_ok_caps_only_immediate_mode(immediate in any::<bool>(), num in count()) {
        prop_assert_eq!(push_ok(immediate, num), !(immediate && num == IMMEDIATE_COUNT));
    }

    #[test]
    fn edge_draw_matches_the_c(verts in count(), norms in count(), colors in count(), tex0 in count()) {
        prop_assert_eq!(draw(verts, norms, colors, tex0), model_draw(verts, norms, colors, tex0));
    }

    #[test]
    fn edge_save_takes_the_shortest_leading_arrays(
        verts in count(),
        norms in count(),
        colors in count(),
        tex in tex_counts(),
    ) {
        let s = immediate::save(verts, norms, colors, &tex);
        if verts == 0 {
            prop_assert_eq!(s, Save { num: 0, num_tex: 0, free_from: TEX_ARRAYS });
        } else {
            let num_tex = tex.iter().take_while(|&&t| t != 0).count();
            let mut num = verts;
            for c in [norms, colors].into_iter().chain(tex.iter().copied().take(num_tex)) {
                if c != 0 {
                    num = num.min(c);
                }
            }
            prop_assert_eq!(s, Save { num, num_tex, free_from: num_tex });
            prop_assert!(s.num >= 1 && s.num <= verts);
        }
    }
}
