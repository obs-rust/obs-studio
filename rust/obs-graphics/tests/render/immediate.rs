//! Tier 1: the safe core of `libobs/graphics/graphics-immediate.c.h`.
//!
//! `steps_*` mirror `test/cmocka/test_graphics_immediate_steps.c` one to
//! one (same names, values and expectations), the port's cases of its
//! `GS_IMMEDIATE_TEST_RUST` build included. `edge_*` cover the rest of the
//! intentional differences (every unit, every cut and free start) and
//! proptests of every decision against a model.

use obs_graphics::immediate::{
    IMMEDIATE_COUNT, TEX_ARRAYS, cut, mismatch, push_ok, save_cut, save_free_from, unit_ok,
};
use proptest::prelude::*;

/// The cmocka `stop_count()`: `gs_render_stop()`'s draw count, with the
/// number of error logs.
fn stop_count(verts: usize, norms: usize, colors: usize, tex0: usize) -> (usize, i32) {
    let mut num = verts;
    let mut logs = 0;
    if num == 0 {
        return (0, 0);
    }
    for count in [norms, colors, tex0] {
        if mismatch(count, verts) {
            logs += 1;
            num = cut(num, count);
        }
    }
    (num, logs)
}

/// The cmocka `save_count()`: `gs_render_save()`'s vertex count for a buffer
/// with `num_tex` texture arrays of `tex` vertices each.
fn save_count(verts: usize, norms: usize, colors: usize, num_tex: usize, tex: usize) -> usize {
    let mut num = save_cut(save_cut(verts, norms), colors);
    for _ in 0..num_tex {
        num = save_cut(num, tex);
    }
    num
}

#[test]
fn steps_matching_counts_draw_every_vertex() {
    assert_eq!(stop_count(1, 0, 0, 0), (1, 0));
    assert_eq!(stop_count(4, 4, 4, 4), (4, 0));
    assert_eq!(stop_count(4, 0, 4, 0), (4, 0));
    assert_eq!(
        stop_count(
            IMMEDIATE_COUNT,
            IMMEDIATE_COUNT,
            IMMEDIATE_COUNT,
            IMMEDIATE_COUNT
        ),
        (IMMEDIATE_COUNT, 0)
    );
}

#[test]
fn steps_a_shorter_array_cuts_the_draw() {
    assert!(mismatch(3, 4));
    assert_eq!(cut(4, 3), 3);
    assert_eq!(cut(3, 3), 3);
    assert_eq!(cut(0, 3), 0);

    assert_eq!(stop_count(4, 3, 4, 4), (3, 1));
    assert_eq!(stop_count(4, 4, 2, 4), (2, 1));
    assert_eq!(stop_count(4, 4, 4, 1), (1, 1));
    assert_eq!(stop_count(5, 4, 3, 2), (2, 3));
    assert_eq!(stop_count(5, 2, 3, 4), (2, 3));
}

#[test]
fn steps_an_empty_array_never_differs() {
    assert!(!mismatch(0, 0));
    assert!(!mismatch(0, 1));
    assert!(!mismatch(0, IMMEDIATE_COUNT));
    assert!(!mismatch(4, 4));
    assert_eq!(stop_count(5, 0, 3, 0), (3, 1));
}

#[test]
fn steps_a_longer_array_logs_without_a_cut() {
    // Characterized, not endorsed.
    assert!(mismatch(6, 4));
    assert_eq!(cut(4, 6), 4);
    assert_eq!(stop_count(4, 6, 4, 4), (4, 1));
    assert_eq!(stop_count(4, 9, 2, 6), (2, 3));
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
fn steps_a_full_save_keeps_its_count() {
    assert_eq!(save_cut(4, 0), 4);
    assert_eq!(save_cut(4, 4), 4);
    assert_eq!(save_cut(4, 9), 4);
    assert_eq!(save_count(1, 1, 1, 1, 1), 1);
    assert_eq!(save_count(4, 4, 4, 1, 4), 4);
    assert_eq!(save_count(7, 0, 0, 0, 0), 7);
    assert_eq!(save_count(3, 9, 0, TEX_ARRAYS, 3), 3);
    assert_eq!(
        save_count(
            IMMEDIATE_COUNT,
            IMMEDIATE_COUNT,
            IMMEDIATE_COUNT,
            TEX_ARRAYS,
            IMMEDIATE_COUNT
        ),
        IMMEDIATE_COUNT
    );
}

#[test]
fn steps_a_save_taking_every_array_frees_none() {
    assert_eq!(save_free_from(TEX_ARRAYS), TEX_ARRAYS);
    for num_tex in 0..=TEX_ARRAYS {
        let from = save_free_from(num_tex);
        assert!(from >= num_tex);
        assert!(from <= TEX_ARRAYS);
    }
}

#[test]
fn steps_the_port_refuses_unbacked_units() {
    assert!(!unit_ok(false, -1));
    assert!(!unit_ok(false, 16));
    assert!(!unit_ok(true, -1));
    assert!(!unit_ok(true, 16));
    for unit in 1..16 {
        assert!(!unit_ok(true, unit), "unit {unit}");
    }
}

#[test]
fn steps_the_port_cuts_a_save_to_a_shorter_array() {
    assert_eq!(save_cut(4, 3), 3);
    assert_eq!(save_count(4, 3, 4, 0, 0), 3);
    assert_eq!(save_count(4, 4, 2, 0, 0), 2);
    assert_eq!(save_count(4, 4, 4, 2, 1), 1);
}

#[test]
fn steps_the_port_frees_the_arrays_a_save_leaves() {
    for num_tex in 0..=TEX_ARRAYS {
        assert_eq!(save_free_from(num_tex), num_tex);
    }
}

/* ---------------------------------------------------------------------- */
/* The rest of the intentional differences */

#[test]
fn edge_a_unit_outside_the_arrays_is_refused() {
    for unit in [i32::MIN, -17, -2, -1, 16, 17, 1000, i32::MAX] {
        assert!(!unit_ok(false, unit), "unit {unit}");
        assert!(!unit_ok(true, unit), "immediate unit {unit}");
    }
}

#[test]
fn edge_the_save_cut_keeps_longer_and_empty_arrays() {
    for num in [0, 1, 4, IMMEDIATE_COUNT, usize::MAX] {
        assert_eq!(save_cut(num, 0), num);
        assert_eq!(save_cut(num, num), num);
        assert_eq!(save_cut(num, num.saturating_add(1)), num);
        assert_eq!(save_cut(num, usize::MAX), num);
    }
    assert_eq!(save_cut(usize::MAX, 1), 1);
    assert_eq!(
        save_cut(IMMEDIATE_COUNT, IMMEDIATE_COUNT - 1),
        IMMEDIATE_COUNT - 1
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
    fn edge_the_stop_matches_the_c(verts in count(), norms in count(), colors in count(), tex0 in count()) {
        prop_assert_eq!(mismatch(norms, verts), norms != 0 && norms != verts);
        prop_assert_eq!(cut(verts, norms), if verts < norms { verts } else { norms });
        let (num, _) = stop_count(verts, norms, colors, tex0);
        prop_assert!(num <= verts);
    }

    #[test]
    fn edge_the_save_cut_is_the_shortest_taken_array(num in count(), count in count()) {
        let cut = save_cut(num, count);
        prop_assert_eq!(cut, if count == 0 { num } else { num.min(count) });
    }

    #[test]
    fn edge_the_save_frees_from_its_texture_arrays(num_tex in 0..=TEX_ARRAYS) {
        prop_assert_eq!(save_free_from(num_tex), num_tex);
    }
}
