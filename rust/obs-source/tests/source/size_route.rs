//! Tier 1: `obs_source::size_route`, the size query routing of
//! `obs_source_get_width()` and `obs_source_get_height()`.
//!
//! The `steps_*` cases port `test/cmocka/test_source_size_route_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the 128-row route table included.
//!
//! The `edge_*` cases are new: the route's invariants over every row,
//! proptests of the route and the entry against models written another way,
//! and the swap over any `i32` (what a 32-bit `long` holds, as on Windows)
//! and, where `long` is 64 bits, any `i64`.

use core::ffi::c_long;

use obs_source::size_route::{
    Entry, Node, Route, TYPE_FILTER, TYPE_TRANSITION, entry, route, swaps,
};
use proptest::prelude::*;

/// `enum obs_source_type` (`obs-source.h`).
const INPUT: i32 = 0;
const FILTER: i32 = TYPE_FILTER;
const TRANSITION: i32 = TYPE_TRANSITION;
const SCENE: i32 = 3;

const X: Route = Route::Transition;
const C: Route = Route::Callback;
const T: Route = Route::AskTarget;
const A: Route = Route::Async { swap: false };
const S: Route = Route::Async { swap: true };
const Z: Route = Route::Zero;

fn node_of(
    source_type: i32,
    has_data: bool,
    has_callback: bool,
    has_filter_parent: bool,
    enabled: bool,
    async_active: bool,
    async_rotation: c_long,
) -> Node {
    Node {
        source_type,
        has_data,
        has_callback,
        has_filter_parent,
        enabled,
        async_active,
        async_rotation,
    }
}

#[track_caller]
fn check_route(node: Node, expected: Route) {
    assert_eq!(route(node), expected, "route({node:?})");
}

#[track_caller]
fn check_entry(source_type: i32, has_filters: bool, expected: Entry) {
    assert_eq!(
        entry(source_type, has_filters),
        expected,
        "entry({source_type}, {has_filters})"
    );
}

/// An active async input with no callback: the route is the swap alone.
#[track_caller]
fn check_swap(rotation: c_long, swapped: bool) {
    check_route(
        node_of(INPUT, true, false, false, true, true, rotation),
        if swapped { S } else { A },
    );
}

/// type, data, callback, parent, enabled, async, route at rotation 0: every
/// combination, the type first, then a binary number with data first.
type Row = (i32, u8, u8, u8, u8, u8, Route);

#[rustfmt::skip]
const ROUTE_ROWS: [Row; 128] = [
    (INPUT, 0, 0, 0, 0, 0, Z),
    (INPUT, 0, 0, 0, 0, 1, A),
    (INPUT, 0, 0, 0, 1, 0, Z),
    (INPUT, 0, 0, 0, 1, 1, A),
    (INPUT, 0, 0, 1, 0, 0, T),
    (INPUT, 0, 0, 1, 0, 1, T),
    (INPUT, 0, 0, 1, 1, 0, T),
    (INPUT, 0, 0, 1, 1, 1, T),
    (INPUT, 0, 1, 0, 0, 0, Z),
    (INPUT, 0, 1, 0, 0, 1, A),
    (INPUT, 0, 1, 0, 1, 0, Z),
    (INPUT, 0, 1, 0, 1, 1, A),
    (INPUT, 0, 1, 1, 0, 0, T),
    (INPUT, 0, 1, 1, 0, 1, T),
    (INPUT, 0, 1, 1, 1, 0, T),
    (INPUT, 0, 1, 1, 1, 1, T),
    (INPUT, 1, 0, 0, 0, 0, Z),
    (INPUT, 1, 0, 0, 0, 1, A),
    (INPUT, 1, 0, 0, 1, 0, Z),
    (INPUT, 1, 0, 0, 1, 1, A),
    (INPUT, 1, 0, 1, 0, 0, T),
    (INPUT, 1, 0, 1, 0, 1, T),
    (INPUT, 1, 0, 1, 1, 0, T),
    (INPUT, 1, 0, 1, 1, 1, T),
    (INPUT, 1, 1, 0, 0, 0, C),
    (INPUT, 1, 1, 0, 0, 1, C),
    (INPUT, 1, 1, 0, 1, 0, C),
    (INPUT, 1, 1, 0, 1, 1, C),
    (INPUT, 1, 1, 1, 0, 0, T),
    (INPUT, 1, 1, 1, 0, 1, T),
    (INPUT, 1, 1, 1, 1, 0, C),
    (INPUT, 1, 1, 1, 1, 1, C),
    (FILTER, 0, 0, 0, 0, 0, Z),
    (FILTER, 0, 0, 0, 0, 1, A),
    (FILTER, 0, 0, 0, 1, 0, Z),
    (FILTER, 0, 0, 0, 1, 1, A),
    (FILTER, 0, 0, 1, 0, 0, T),
    (FILTER, 0, 0, 1, 0, 1, T),
    (FILTER, 0, 0, 1, 1, 0, T),
    (FILTER, 0, 0, 1, 1, 1, T),
    (FILTER, 0, 1, 0, 0, 0, Z),
    (FILTER, 0, 1, 0, 0, 1, A),
    (FILTER, 0, 1, 0, 1, 0, Z),
    (FILTER, 0, 1, 0, 1, 1, A),
    (FILTER, 0, 1, 1, 0, 0, T),
    (FILTER, 0, 1, 1, 0, 1, T),
    (FILTER, 0, 1, 1, 1, 0, T),
    (FILTER, 0, 1, 1, 1, 1, T),
    (FILTER, 1, 0, 0, 0, 0, Z),
    (FILTER, 1, 0, 0, 0, 1, A),
    (FILTER, 1, 0, 0, 1, 0, Z),
    (FILTER, 1, 0, 0, 1, 1, A),
    (FILTER, 1, 0, 1, 0, 0, T),
    (FILTER, 1, 0, 1, 0, 1, T),
    (FILTER, 1, 0, 1, 1, 0, T),
    (FILTER, 1, 0, 1, 1, 1, T),
    (FILTER, 1, 1, 0, 0, 0, C),
    (FILTER, 1, 1, 0, 0, 1, C),
    (FILTER, 1, 1, 0, 1, 0, C),
    (FILTER, 1, 1, 0, 1, 1, C),
    (FILTER, 1, 1, 1, 0, 0, T),
    (FILTER, 1, 1, 1, 0, 1, T),
    (FILTER, 1, 1, 1, 1, 0, C),
    (FILTER, 1, 1, 1, 1, 1, C),
    (TRANSITION, 0, 0, 0, 0, 0, Z),
    (TRANSITION, 0, 0, 0, 0, 1, Z),
    (TRANSITION, 0, 0, 0, 1, 0, X),
    (TRANSITION, 0, 0, 0, 1, 1, X),
    (TRANSITION, 0, 0, 1, 0, 0, Z),
    (TRANSITION, 0, 0, 1, 0, 1, Z),
    (TRANSITION, 0, 0, 1, 1, 0, X),
    (TRANSITION, 0, 0, 1, 1, 1, X),
    (TRANSITION, 0, 1, 0, 0, 0, Z),
    (TRANSITION, 0, 1, 0, 0, 1, Z),
    (TRANSITION, 0, 1, 0, 1, 0, X),
    (TRANSITION, 0, 1, 0, 1, 1, X),
    (TRANSITION, 0, 1, 1, 0, 0, Z),
    (TRANSITION, 0, 1, 1, 0, 1, Z),
    (TRANSITION, 0, 1, 1, 1, 0, X),
    (TRANSITION, 0, 1, 1, 1, 1, X),
    (TRANSITION, 1, 0, 0, 0, 0, Z),
    (TRANSITION, 1, 0, 0, 0, 1, Z),
    (TRANSITION, 1, 0, 0, 1, 0, X),
    (TRANSITION, 1, 0, 0, 1, 1, X),
    (TRANSITION, 1, 0, 1, 0, 0, Z),
    (TRANSITION, 1, 0, 1, 0, 1, Z),
    (TRANSITION, 1, 0, 1, 1, 0, X),
    (TRANSITION, 1, 0, 1, 1, 1, X),
    (TRANSITION, 1, 1, 0, 0, 0, Z),
    (TRANSITION, 1, 1, 0, 0, 1, Z),
    (TRANSITION, 1, 1, 0, 1, 0, X),
    (TRANSITION, 1, 1, 0, 1, 1, X),
    (TRANSITION, 1, 1, 1, 0, 0, Z),
    (TRANSITION, 1, 1, 1, 0, 1, Z),
    (TRANSITION, 1, 1, 1, 1, 0, X),
    (TRANSITION, 1, 1, 1, 1, 1, X),
    (SCENE, 0, 0, 0, 0, 0, Z),
    (SCENE, 0, 0, 0, 0, 1, A),
    (SCENE, 0, 0, 0, 1, 0, Z),
    (SCENE, 0, 0, 0, 1, 1, A),
    (SCENE, 0, 0, 1, 0, 0, T),
    (SCENE, 0, 0, 1, 0, 1, T),
    (SCENE, 0, 0, 1, 1, 0, T),
    (SCENE, 0, 0, 1, 1, 1, T),
    (SCENE, 0, 1, 0, 0, 0, Z),
    (SCENE, 0, 1, 0, 0, 1, A),
    (SCENE, 0, 1, 0, 1, 0, Z),
    (SCENE, 0, 1, 0, 1, 1, A),
    (SCENE, 0, 1, 1, 0, 0, T),
    (SCENE, 0, 1, 1, 0, 1, T),
    (SCENE, 0, 1, 1, 1, 0, T),
    (SCENE, 0, 1, 1, 1, 1, T),
    (SCENE, 1, 0, 0, 0, 0, Z),
    (SCENE, 1, 0, 0, 0, 1, A),
    (SCENE, 1, 0, 0, 1, 0, Z),
    (SCENE, 1, 0, 0, 1, 1, A),
    (SCENE, 1, 0, 1, 0, 0, T),
    (SCENE, 1, 0, 1, 0, 1, T),
    (SCENE, 1, 0, 1, 1, 0, T),
    (SCENE, 1, 0, 1, 1, 1, T),
    (SCENE, 1, 1, 0, 0, 0, C),
    (SCENE, 1, 1, 0, 0, 1, C),
    (SCENE, 1, 1, 0, 1, 0, C),
    (SCENE, 1, 1, 0, 1, 1, C),
    (SCENE, 1, 1, 1, 0, 0, T),
    (SCENE, 1, 1, 1, 0, 1, T),
    (SCENE, 1, 1, 1, 1, 0, C),
    (SCENE, 1, 1, 1, 1, 1, C),
];

const TYPES: [i32; 4] = [INPUT, FILTER, TRANSITION, SCENE];

fn row_node(row: &Row, source_type: i32, rotation: c_long) -> Node {
    let (_, data, callback, parent, enabled, async_active, _) = *row;
    node_of(
        source_type,
        data != 0,
        callback != 0,
        parent != 0,
        enabled != 0,
        async_active != 0,
        rotation,
    )
}

#[test]
fn steps_route_every_combination() {
    for (i, row) in ROUTE_ROWS.iter().enumerate() {
        let bits = (usize::from(row.1) << 4)
            | (usize::from(row.2) << 3)
            | (usize::from(row.3) << 2)
            | (usize::from(row.4) << 1)
            | usize::from(row.5);
        assert_eq!(row.0, TYPES[i / 32], "row {i} is out of order");
        assert_eq!(bits, i % 32, "row {i} is out of order");
        check_route(row_node(row, row.0, 0), row.6);
    }
}

#[test]
fn steps_route_every_combination_rotated() {
    for row in &ROUTE_ROWS {
        let expected = if row.6 == A { S } else { row.6 };
        check_route(row_node(row, row.0, 90), expected);
    }
}

#[test]
fn steps_route_other_types_as_inputs() {
    for source_type in [FILTER, SCENE, -1, 4, i32::MIN, i32::MAX] {
        for row in &ROUTE_ROWS[..32] {
            check_route(row_node(row, source_type, 0), row.6);
        }
    }
}

#[test]
fn steps_route_transition() {
    for bits in 0u8..16 {
        let (data, callback, parent, async_active) =
            (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
        check_route(
            node_of(TRANSITION, data, callback, parent, true, async_active, 90),
            X,
        );
        check_route(
            node_of(TRANSITION, data, callback, parent, false, async_active, 90),
            Z,
        );
    }
}

#[test]
fn steps_route_filter_chains() {
    check_route(node_of(FILTER, true, true, true, true, false, 0), C);
    check_route(node_of(FILTER, true, true, true, false, false, 0), T);
    check_route(node_of(FILTER, true, false, true, true, false, 0), T);
    check_route(node_of(FILTER, false, true, true, true, false, 0), T);
    check_route(node_of(FILTER, true, false, true, true, true, 90), T);
    check_route(node_of(INPUT, true, true, false, true, false, 0), C);
    check_route(node_of(INPUT, true, true, false, false, false, 0), C);
    check_route(node_of(INPUT, true, false, false, true, true, 0), A);
    check_route(node_of(INPUT, true, false, false, true, true, 90), S);
    check_route(node_of(INPUT, true, false, false, true, false, 90), Z);
    check_route(node_of(INPUT, true, true, false, true, true, 90), C);
    check_route(node_of(SCENE, true, true, false, true, false, 0), C);
}

/// Characterized, not endorsed.
#[test]
fn steps_route_detached_filter() {
    check_route(node_of(FILTER, true, true, false, false, false, 0), C);
    check_route(node_of(FILTER, true, false, false, true, false, 0), Z);
    check_route(node_of(FILTER, true, false, false, true, true, 90), S);
    check_route(node_of(INPUT, true, true, true, false, false, 0), T);
}

/// Characterized, not endorsed.
#[test]
fn steps_route_no_data() {
    check_route(node_of(INPUT, false, true, false, true, false, 0), Z);
    check_route(node_of(INPUT, false, true, false, true, true, 0), A);
    check_route(node_of(SCENE, false, true, false, true, false, 0), Z);
}

#[test]
fn steps_swap_rotations() {
    check_swap(0, false);
    check_swap(90, true);
    check_swap(-90, true);
    check_swap(180, false);
    check_swap(-180, false);
    check_swap(270, true);
    check_swap(-270, true);
    check_swap(45, true);
    check_swap(360, false);
    check_swap(1, true);
    check_swap(179, true);
    check_swap(181, true);
    check_swap(359, true);
    check_swap(-360, false);
    check_swap(540, false);
    check_swap(-1, true);
}

#[test]
fn steps_swap_long_extremes() {
    check_swap(c_long::MIN, true);
    check_swap(c_long::MAX, true);
    if c_long::BITS == 32 {
        check_swap(c_long::MIN + 128, false);
        check_swap(c_long::MAX - 127, false);
    } else {
        check_swap(c_long::MIN + 8, false);
        check_swap(c_long::MAX - 7, false);
        check_swap(c_long::from(i32::MIN), true);
        check_swap(c_long::from(i32::MAX), true);
        check_swap(c_long::from(i32::MIN) + 128, false);
    }
}

#[test]
fn steps_swap_only_when_async() {
    for rotation in [0, 90, 45, c_long::MIN, c_long::MAX] {
        check_route(node_of(INPUT, true, true, false, true, true, rotation), C);
        check_route(node_of(FILTER, true, false, true, true, true, rotation), T);
        check_route(
            node_of(TRANSITION, true, false, false, true, true, rotation),
            X,
        );
        check_route(node_of(INPUT, true, false, false, true, false, rotation), Z);
    }
}

#[test]
fn steps_entry_points() {
    check_entry(FILTER, false, Entry::Itself);
    check_entry(FILTER, true, Entry::Itself);
    check_entry(INPUT, true, Entry::FirstFilter);
    check_entry(INPUT, false, Entry::Itself);
    check_entry(TRANSITION, true, Entry::FirstFilter);
    check_entry(TRANSITION, false, Entry::Itself);
    check_entry(SCENE, true, Entry::FirstFilter);
    check_entry(SCENE, false, Entry::Itself);
    check_entry(-1, true, Entry::FirstFilter);
    check_entry(4, true, Entry::FirstFilter);
    check_entry(i32::MIN, true, Entry::FirstFilter);
    check_entry(i32::MAX, false, Entry::Itself);
}

/// Every row with every type and a spread of rotations: each route has
/// what it needs, and the async route's swap is [`swaps`] of the rotation.
#[test]
fn edge_route_invariants() {
    let rotations: [c_long; 8] = [0, 90, -90, 180, 45, 359, c_long::MIN, c_long::MAX];
    for row in &ROUTE_ROWS {
        for source_type in [INPUT, FILTER, TRANSITION, SCENE, -1] {
            for rotation in rotations {
                let node = row_node(row, source_type, rotation);
                match route(node) {
                    Route::Transition => {
                        assert_eq!(source_type, TRANSITION, "{node:?}");
                        assert!(node.enabled, "{node:?}");
                    }
                    Route::Callback => {
                        assert!(node.has_data && node.has_callback, "{node:?}");
                        assert!(!node.has_filter_parent || node.enabled, "{node:?}");
                        assert_ne!(source_type, TRANSITION, "{node:?}");
                    }
                    Route::AskTarget => {
                        assert!(node.has_filter_parent, "{node:?}");
                        assert_ne!(source_type, TRANSITION, "{node:?}");
                    }
                    Route::Async { swap } => {
                        assert!(node.async_active && !node.has_filter_parent, "{node:?}");
                        assert_eq!(swap, swaps(rotation), "{node:?}");
                    }
                    Route::Zero => {}
                }
            }
        }
    }
}

/// The swap is periodic in 180 wherever the sum fits.
#[test]
fn edge_swap_period() {
    for rotation in -1000..=1000 {
        assert_eq!(swaps(rotation), swaps(rotation + 180), "{rotation}");
        assert_eq!(swaps(rotation), rotation.rem_euclid(180) != 0, "{rotation}");
    }
}

/// The route written as one match over the fields.
fn route_model(node: Node) -> Route {
    let callback_answers = node.has_data && node.has_callback;
    match (
        node.source_type == TRANSITION,
        node.enabled,
        node.has_filter_parent,
        callback_answers,
        node.async_active,
    ) {
        (true, true, ..) => X,
        (true, false, ..) => Z,
        (false, _, false, true, _) | (false, true, true, true, _) => C,
        (false, _, true, ..) => T,
        (false, _, false, false, true) => Route::Async {
            swap: node.async_rotation.rem_euclid(180) != 0,
        },
        (false, _, false, false, false) => Z,
    }
}

fn any_node() -> impl Strategy<Value = Node> {
    (
        prop_oneof![any::<i32>(), -2i32..6],
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        prop_oneof![any::<c_long>(), (-720i32..=720).prop_map(c_long::from)],
    )
        .prop_map(|(t, d, c, p, e, a, r)| node_of(t, d, c, p, e, a, r))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn edge_route_model(node in any_node()) {
        prop_assert_eq!(route(node), route_model(node), "{:?}", node);
    }

    /// Only the transition type has a route of its own.
    #[test]
    fn edge_route_only_transition_type(node in any_node()) {
        let as_input = Node {
            source_type: if node.source_type == TRANSITION { TRANSITION } else { INPUT },
            ..node
        };
        prop_assert_eq!(route(node), route(as_input), "{:?}", node);
    }

    #[test]
    fn edge_entry_model(source_type in prop_oneof![any::<i32>(), -2i32..6], has_filters in any::<bool>()) {
        let expected = if source_type != FILTER && has_filters {
            Entry::FirstFilter
        } else {
            Entry::Itself
        };
        prop_assert_eq!(entry(source_type, has_filters), expected);
    }

    /// Any value a 32-bit `long` holds: the swap is `i32`'s `%`, the same
    /// on either width.
    #[test]
    fn edge_swap_any_i32(rotation in any::<i32>()) {
        prop_assert_eq!(swaps(c_long::from(rotation)), rotation % 180 != 0, "{}", rotation);
        prop_assert_eq!(swaps(c_long::from(rotation)), i64::from(rotation) % 180 != 0, "{}", rotation);
    }

    /// Any value a 64-bit `long` holds, where `long` is 64 bits (the
    /// conversion cannot fail there, and skips what a 32-bit one cannot
    /// hold).
    #[test]
    #[allow(irrefutable_let_patterns)]
    fn edge_swap_any_i64(rotation in any::<i64>()) {
        if let Ok(long) = c_long::try_from(rotation) {
            prop_assert_eq!(swaps(long), rotation % 180 != 0, "{}", rotation);
        }
    }
}
