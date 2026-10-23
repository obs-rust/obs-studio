//! Tier 1: `obs_source::color_space`, the colour space query routing of
//! `obs_source_get_color_space()`.
//!
//! The `steps_*` cases port `test/cmocka/test_source_color_space_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the 128-row route table included.
//!
//! The `edge_*` cases are new: the route's invariants over every row and
//! proptests over any type and any flags (only `OBS_SOURCE_TYPE_FILTER` is a
//! filter, only the VIDEO and ASYNC bits count), and the pick against a
//! model written another way (the frame's space when the list is empty or
//! holds it, the last entry otherwise) over lists of 0 to 8 values.

use obs_source::color_space::{ASYNC, Node, Route, TYPE_FILTER, VIDEO, pick, route};
use proptest::prelude::*;

/// `enum gs_color_space` (`graphics/graphics.h`).
const GS_CS_SRGB: i32 = 0;
const GS_CS_SRGB_16F: i32 = 1;
const GS_CS_709_EXTENDED: i32 = 2;
const GS_CS_709_SCRGB: i32 = 3;

/// `enum obs_source_type` and the output flags (`obs-source.h`).
const TYPE_INPUT: i32 = 0;
const TYPE_TRANSITION: i32 = 2;
const TYPE_SCENE: i32 = 3;
/// `OBS_SOURCE_AUDIO`.
const AUDIO: u32 = 1 << 1;
/// `OBS_SOURCE_ASYNC_VIDEO`.
const ASYNC_VIDEO: u32 = ASYNC | VIDEO;

const P: Route = Route::AskParent;
const T: Route = Route::AskTarget;
const A: Route = Route::AsyncPick;
const C: Route = Route::Callback;

fn node_of(
    source_type: i32,
    output_flags: u32,
    has_data: bool,
    enabled: bool,
    has_filter_parent: bool,
    has_filter_target: bool,
) -> Node {
    Node {
        source_type,
        output_flags,
        has_data,
        enabled,
        has_filter_parent,
        has_filter_target,
    }
}

#[track_caller]
fn check_route(node: Node, expected: Route) {
    assert_eq!(route(node), expected, "route({node:?})");
}

#[track_caller]
fn check_pick(video_space: i32, preferred: &[i32], expected: i32) {
    assert_eq!(
        pick(video_space, preferred),
        expected,
        "pick({video_space}, {preferred:?})"
    );
}

/// filter, video, async, data, enabled, parent, target, route: every
/// combination, counting up as a binary number with the filter bit first.
type Row = (u8, u8, u8, u8, u8, u8, u8, Route);

#[rustfmt::skip]
const ROUTE_ROWS: [Row; 128] = [
    (0, 0, 0, 0, 0, 0, 0, C),
    (0, 0, 0, 0, 0, 0, 1, T),
    (0, 0, 0, 0, 0, 1, 0, P),
    (0, 0, 0, 0, 0, 1, 1, P),
    (0, 0, 0, 0, 1, 0, 0, C),
    (0, 0, 0, 0, 1, 0, 1, T),
    (0, 0, 0, 0, 1, 1, 0, P),
    (0, 0, 0, 0, 1, 1, 1, P),
    (0, 0, 0, 1, 0, 0, 0, C),
    (0, 0, 0, 1, 0, 0, 1, T),
    (0, 0, 0, 1, 0, 1, 0, P),
    (0, 0, 0, 1, 0, 1, 1, P),
    (0, 0, 0, 1, 1, 0, 0, C),
    (0, 0, 0, 1, 1, 0, 1, C),
    (0, 0, 0, 1, 1, 1, 0, P),
    (0, 0, 0, 1, 1, 1, 1, P),
    (0, 0, 1, 0, 0, 0, 0, A),
    (0, 0, 1, 0, 0, 0, 1, T),
    (0, 0, 1, 0, 0, 1, 0, P),
    (0, 0, 1, 0, 0, 1, 1, P),
    (0, 0, 1, 0, 1, 0, 0, A),
    (0, 0, 1, 0, 1, 0, 1, T),
    (0, 0, 1, 0, 1, 1, 0, P),
    (0, 0, 1, 0, 1, 1, 1, P),
    (0, 0, 1, 1, 0, 0, 0, A),
    (0, 0, 1, 1, 0, 0, 1, T),
    (0, 0, 1, 1, 0, 1, 0, P),
    (0, 0, 1, 1, 0, 1, 1, P),
    (0, 0, 1, 1, 1, 0, 0, A),
    (0, 0, 1, 1, 1, 0, 1, A),
    (0, 0, 1, 1, 1, 1, 0, P),
    (0, 0, 1, 1, 1, 1, 1, P),
    (0, 1, 0, 0, 0, 0, 0, C),
    (0, 1, 0, 0, 0, 0, 1, T),
    (0, 1, 0, 0, 0, 1, 0, C),
    (0, 1, 0, 0, 0, 1, 1, T),
    (0, 1, 0, 0, 1, 0, 0, C),
    (0, 1, 0, 0, 1, 0, 1, T),
    (0, 1, 0, 0, 1, 1, 0, C),
    (0, 1, 0, 0, 1, 1, 1, T),
    (0, 1, 0, 1, 0, 0, 0, C),
    (0, 1, 0, 1, 0, 0, 1, T),
    (0, 1, 0, 1, 0, 1, 0, C),
    (0, 1, 0, 1, 0, 1, 1, T),
    (0, 1, 0, 1, 1, 0, 0, C),
    (0, 1, 0, 1, 1, 0, 1, C),
    (0, 1, 0, 1, 1, 1, 0, C),
    (0, 1, 0, 1, 1, 1, 1, C),
    (0, 1, 1, 0, 0, 0, 0, A),
    (0, 1, 1, 0, 0, 0, 1, T),
    (0, 1, 1, 0, 0, 1, 0, A),
    (0, 1, 1, 0, 0, 1, 1, T),
    (0, 1, 1, 0, 1, 0, 0, A),
    (0, 1, 1, 0, 1, 0, 1, T),
    (0, 1, 1, 0, 1, 1, 0, A),
    (0, 1, 1, 0, 1, 1, 1, T),
    (0, 1, 1, 1, 0, 0, 0, A),
    (0, 1, 1, 1, 0, 0, 1, T),
    (0, 1, 1, 1, 0, 1, 0, A),
    (0, 1, 1, 1, 0, 1, 1, T),
    (0, 1, 1, 1, 1, 0, 0, A),
    (0, 1, 1, 1, 1, 0, 1, A),
    (0, 1, 1, 1, 1, 1, 0, A),
    (0, 1, 1, 1, 1, 1, 1, A),
    (1, 0, 0, 0, 0, 0, 0, C),
    (1, 0, 0, 0, 0, 0, 1, T),
    (1, 0, 0, 0, 0, 1, 0, C),
    (1, 0, 0, 0, 0, 1, 1, T),
    (1, 0, 0, 0, 1, 0, 0, C),
    (1, 0, 0, 0, 1, 0, 1, T),
    (1, 0, 0, 0, 1, 1, 0, C),
    (1, 0, 0, 0, 1, 1, 1, T),
    (1, 0, 0, 1, 0, 0, 0, C),
    (1, 0, 0, 1, 0, 0, 1, T),
    (1, 0, 0, 1, 0, 1, 0, C),
    (1, 0, 0, 1, 0, 1, 1, T),
    (1, 0, 0, 1, 1, 0, 0, C),
    (1, 0, 0, 1, 1, 0, 1, T),
    (1, 0, 0, 1, 1, 1, 0, C),
    (1, 0, 0, 1, 1, 1, 1, T),
    (1, 0, 1, 0, 0, 0, 0, A),
    (1, 0, 1, 0, 0, 0, 1, T),
    (1, 0, 1, 0, 0, 1, 0, A),
    (1, 0, 1, 0, 0, 1, 1, T),
    (1, 0, 1, 0, 1, 0, 0, A),
    (1, 0, 1, 0, 1, 0, 1, T),
    (1, 0, 1, 0, 1, 1, 0, A),
    (1, 0, 1, 0, 1, 1, 1, T),
    (1, 0, 1, 1, 0, 0, 0, A),
    (1, 0, 1, 1, 0, 0, 1, T),
    (1, 0, 1, 1, 0, 1, 0, A),
    (1, 0, 1, 1, 0, 1, 1, T),
    (1, 0, 1, 1, 1, 0, 0, A),
    (1, 0, 1, 1, 1, 0, 1, T),
    (1, 0, 1, 1, 1, 1, 0, A),
    (1, 0, 1, 1, 1, 1, 1, T),
    (1, 1, 0, 0, 0, 0, 0, C),
    (1, 1, 0, 0, 0, 0, 1, T),
    (1, 1, 0, 0, 0, 1, 0, C),
    (1, 1, 0, 0, 0, 1, 1, T),
    (1, 1, 0, 0, 1, 0, 0, C),
    (1, 1, 0, 0, 1, 0, 1, T),
    (1, 1, 0, 0, 1, 1, 0, C),
    (1, 1, 0, 0, 1, 1, 1, T),
    (1, 1, 0, 1, 0, 0, 0, C),
    (1, 1, 0, 1, 0, 0, 1, T),
    (1, 1, 0, 1, 0, 1, 0, C),
    (1, 1, 0, 1, 0, 1, 1, T),
    (1, 1, 0, 1, 1, 0, 0, C),
    (1, 1, 0, 1, 1, 0, 1, C),
    (1, 1, 0, 1, 1, 1, 0, C),
    (1, 1, 0, 1, 1, 1, 1, C),
    (1, 1, 1, 0, 0, 0, 0, A),
    (1, 1, 1, 0, 0, 0, 1, T),
    (1, 1, 1, 0, 0, 1, 0, A),
    (1, 1, 1, 0, 0, 1, 1, T),
    (1, 1, 1, 0, 1, 0, 0, A),
    (1, 1, 1, 0, 1, 0, 1, T),
    (1, 1, 1, 0, 1, 1, 0, A),
    (1, 1, 1, 0, 1, 1, 1, T),
    (1, 1, 1, 1, 0, 0, 0, A),
    (1, 1, 1, 1, 0, 0, 1, T),
    (1, 1, 1, 1, 0, 1, 0, A),
    (1, 1, 1, 1, 0, 1, 1, T),
    (1, 1, 1, 1, 1, 0, 0, A),
    (1, 1, 1, 1, 1, 0, 1, A),
    (1, 1, 1, 1, 1, 1, 0, A),
    (1, 1, 1, 1, 1, 1, 1, A),
];

fn row_node(row: &Row, source_type: i32, extra_flags: u32) -> Node {
    let (_, video, asynchronous, data, enabled, parent, target, _) = *row;
    let mut flags = extra_flags;
    if video != 0 {
        flags |= VIDEO;
    }
    if asynchronous != 0 {
        flags |= ASYNC;
    }
    node_of(
        source_type,
        flags,
        data != 0,
        enabled != 0,
        parent != 0,
        target != 0,
    )
}

fn row_type(row: &Row) -> i32 {
    if row.0 != 0 { TYPE_FILTER } else { TYPE_INPUT }
}

#[test]
fn steps_route_every_combination() {
    for (i, row) in ROUTE_ROWS.iter().enumerate() {
        let bits = (usize::from(row.0) << 6)
            | (usize::from(row.1) << 5)
            | (usize::from(row.2) << 4)
            | (usize::from(row.3) << 3)
            | (usize::from(row.4) << 2)
            | (usize::from(row.5) << 1)
            | usize::from(row.6);
        assert_eq!(bits, i, "row {i} is out of order");
        check_route(row_node(row, row_type(row), 0), row.7);
    }
}

#[test]
fn steps_route_other_types_as_inputs() {
    for source_type in [TYPE_TRANSITION, TYPE_SCENE, -1, 4, i32::MIN, i32::MAX] {
        for row in &ROUTE_ROWS[..64] {
            check_route(row_node(row, source_type, 0), row.7);
        }
    }
}

#[test]
fn steps_route_other_flags_ignored() {
    let others = !(VIDEO | ASYNC);
    for row in &ROUTE_ROWS {
        check_route(row_node(row, row_type(row), others), row.7);
        check_route(row_node(row, row_type(row), AUDIO), row.7);
    }
}

#[test]
fn steps_route_filter_chains() {
    check_route(
        node_of(TYPE_INPUT, ASYNC_VIDEO | AUDIO, true, true, false, false),
        A,
    );
    // #1493
    check_route(node_of(TYPE_FILTER, AUDIO, true, true, false, true), T);
    check_route(node_of(TYPE_FILTER, AUDIO, true, false, false, true), T);
    check_route(node_of(TYPE_FILTER, VIDEO, true, true, false, true), C);
    check_route(node_of(TYPE_FILTER, VIDEO, true, false, false, true), T);
    check_route(node_of(TYPE_FILTER, VIDEO, false, true, false, true), T);
    check_route(node_of(TYPE_FILTER, AUDIO, true, true, false, false), C);
    check_route(node_of(TYPE_INPUT, AUDIO, true, true, true, false), P);
    check_route(node_of(TYPE_INPUT, AUDIO, false, false, true, true), P);
    check_route(node_of(TYPE_INPUT, AUDIO, true, true, false, false), C);
    check_route(node_of(TYPE_INPUT, VIDEO, true, false, false, true), T);
    check_route(node_of(TYPE_INPUT, VIDEO, true, false, false, false), C);
    check_route(node_of(TYPE_SCENE, VIDEO, true, true, false, false), C);
}

/// Characterized, not endorsed.
#[test]
fn steps_route_filter_flagged_async() {
    check_route(
        node_of(TYPE_FILTER, ASYNC_VIDEO, true, true, false, true),
        A,
    );
    check_route(
        node_of(TYPE_FILTER, ASYNC_VIDEO, true, true, false, false),
        A,
    );
    check_route(node_of(TYPE_FILTER, ASYNC, true, true, false, true), T);
    check_route(node_of(TYPE_FILTER, ASYNC, true, true, false, false), A);
    check_route(
        node_of(TYPE_FILTER, ASYNC_VIDEO, true, false, false, true),
        T,
    );
    check_route(
        node_of(TYPE_FILTER, ASYNC_VIDEO, true, false, false, false),
        A,
    );
    check_route(node_of(TYPE_FILTER, ASYNC, true, true, true, false), A);
}

/// Characterized, not endorsed.
#[test]
fn steps_route_no_data_nowhere_to_go() {
    check_route(node_of(TYPE_INPUT, VIDEO, false, true, false, false), C);
    check_route(node_of(TYPE_FILTER, VIDEO, false, false, false, false), C);
    check_route(
        node_of(TYPE_INPUT, ASYNC_VIDEO, false, false, false, false),
        A,
    );
}

#[test]
fn steps_pick_empty_list() {
    for space in [
        GS_CS_SRGB,
        GS_CS_SRGB_16F,
        GS_CS_709_EXTENDED,
        GS_CS_709_SCRGB,
    ] {
        check_pick(space, &[], space);
    }
    // A count of 0 reads nothing, whatever the pointer.
    check_pick(GS_CS_709_EXTENDED, &[GS_CS_SRGB][..0], GS_CS_709_EXTENDED);
}

#[test]
fn steps_pick_match_first() {
    check_pick(
        GS_CS_709_EXTENDED,
        &[GS_CS_709_EXTENDED, GS_CS_SRGB, GS_CS_SRGB_16F],
        GS_CS_709_EXTENDED,
    );
    check_pick(GS_CS_SRGB, &[GS_CS_SRGB, GS_CS_709_EXTENDED], GS_CS_SRGB);
    check_pick(GS_CS_SRGB_16F, &[GS_CS_SRGB_16F], GS_CS_SRGB_16F);
}

#[test]
fn steps_pick_match_last() {
    // The preferred spaces of the HDR Tone Mapping filter (#1493).
    let tonemap = [GS_CS_SRGB, GS_CS_SRGB_16F, GS_CS_709_EXTENDED];
    check_pick(GS_CS_709_EXTENDED, &tonemap, GS_CS_709_EXTENDED);
    check_pick(GS_CS_SRGB_16F, &tonemap, GS_CS_SRGB_16F);
    check_pick(
        GS_CS_709_SCRGB,
        &[GS_CS_SRGB, GS_CS_709_SCRGB],
        GS_CS_709_SCRGB,
    );
}

#[test]
fn steps_pick_match_in_the_middle() {
    check_pick(
        GS_CS_709_EXTENDED,
        &[GS_CS_SRGB, GS_CS_709_EXTENDED, GS_CS_709_SCRGB],
        GS_CS_709_EXTENDED,
    );
    let twice = [
        GS_CS_SRGB_16F,
        GS_CS_709_EXTENDED,
        GS_CS_SRGB,
        GS_CS_709_EXTENDED,
    ];
    check_pick(GS_CS_709_EXTENDED, &twice, GS_CS_709_EXTENDED);
    check_pick(GS_CS_SRGB, &twice, GS_CS_SRGB);
}

#[test]
fn steps_pick_no_match() {
    check_pick(
        GS_CS_709_EXTENDED,
        &[GS_CS_SRGB, GS_CS_SRGB_16F],
        GS_CS_SRGB_16F,
    );
    check_pick(GS_CS_709_EXTENDED, &[GS_CS_SRGB], GS_CS_SRGB);
    check_pick(
        GS_CS_SRGB,
        &[GS_CS_709_SCRGB, GS_CS_709_EXTENDED, GS_CS_SRGB_16F],
        GS_CS_SRGB_16F,
    );
    check_pick(
        GS_CS_SRGB,
        &[GS_CS_709_SCRGB, GS_CS_709_SCRGB],
        GS_CS_709_SCRGB,
    );
}

/// Characterized, not endorsed.
#[test]
fn steps_pick_out_of_range_passed_through() {
    check_pick(GS_CS_709_EXTENDED, &[GS_CS_SRGB_16F, 99], 99);
    check_pick(GS_CS_SRGB, &[-1], -1);
    let extremes = [i32::MIN, GS_CS_SRGB_16F, i32::MAX];
    check_pick(GS_CS_709_EXTENDED, &extremes, i32::MAX);
    check_pick(GS_CS_SRGB_16F, &extremes, GS_CS_SRGB_16F);
    let past_end = [GS_CS_SRGB, GS_CS_709_SCRGB + 1, GS_CS_709_EXTENDED];
    check_pick(GS_CS_709_SCRGB + 1, &past_end, GS_CS_709_SCRGB + 1);
    check_pick(-7, &[], -7);
}

/// Every row: a filter never asks its parent, nothing asks a target it does
/// not have or a parent it does not have, and a source that has video, data
/// and is enabled never asks anyone.
#[test]
fn edge_route_invariants() {
    for row in &ROUTE_ROWS {
        for source_type in [TYPE_INPUT, TYPE_FILTER, TYPE_TRANSITION, TYPE_SCENE] {
            let node = row_node(row, source_type, 0);
            let got = route(node);
            if got == P {
                assert!(node.has_filter_parent, "{node:?}");
                assert_ne!(source_type, TYPE_FILTER, "{node:?}");
            }
            if got == T {
                assert!(node.has_filter_target, "{node:?}");
            }
            if node.output_flags & VIDEO != 0 && node.has_data && node.enabled {
                assert!(matches!(got, A | C), "{node:?}");
            }
            if got == A {
                assert_ne!(node.output_flags & ASYNC, 0, "{node:?}");
            }
        }
    }
}

/// #1493: a filter without VIDEO and with a target asks the target, with
/// any other flags, enabled or not, with data or not.
#[test]
fn edge_route_filter_without_video_asks_target() {
    for row in ROUTE_ROWS
        .iter()
        .filter(|row| row.0 != 0 && row.1 == 0 && row.6 != 0)
    {
        for extra in [0, AUDIO, !(VIDEO | ASYNC)] {
            check_route(row_node(row, TYPE_FILTER, extra), T);
        }
    }
}

fn normalized(node: Node) -> Node {
    Node {
        source_type: if node.source_type == TYPE_FILTER {
            TYPE_FILTER
        } else {
            TYPE_INPUT
        },
        output_flags: node.output_flags & (VIDEO | ASYNC),
        ..node
    }
}

fn any_node() -> impl Strategy<Value = Node> {
    (
        prop_oneof![any::<i32>(), -2i32..6],
        any::<u32>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
        any::<bool>(),
    )
        .prop_map(|(t, f, d, e, p, g)| node_of(t, f, d, e, p, g))
}

/// The model of the pick: the frame's space when the list is empty or holds
/// it, the last entry otherwise.
fn pick_model(video_space: i32, preferred: &[i32]) -> i32 {
    match preferred.last() {
        Some(&last) if !preferred.contains(&video_space) => last,
        _ => video_space,
    }
}

fn any_space() -> impl Strategy<Value = i32> {
    prop_oneof![-1i32..5, any::<i32>()]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    /// Any type but OBS_SOURCE_TYPE_FILTER routes as an input, and only the
    /// VIDEO and ASYNC bits count.
    #[test]
    fn edge_route_only_filter_type_and_two_flags(node in any_node()) {
        prop_assert_eq!(route(node), route(normalized(node)), "{:?}", node);
    }

    #[test]
    fn edge_pick_model(
        video_space in any_space(),
        preferred in proptest::collection::vec(any_space(), 0..=8),
    ) {
        prop_assert_eq!(
            pick(video_space, &preferred),
            pick_model(video_space, &preferred),
            "{} {:?}", video_space, &preferred
        );
    }

    /// A list that holds the frame's space somewhere always gives it back.
    #[test]
    fn edge_pick_holds_the_frame_space(
        video_space in any_space(),
        mut preferred in proptest::collection::vec(any_space(), 0..8),
        at in any::<proptest::sample::Index>(),
    ) {
        let i = at.index(preferred.len() + 1);
        preferred.insert(i, video_space);
        prop_assert_eq!(pick(video_space, &preferred), video_space);
    }
}
