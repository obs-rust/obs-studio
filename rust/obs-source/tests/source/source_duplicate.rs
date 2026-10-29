//! Tier 1: `obs_source::source_duplicate`, the duplicate decisions of
//! `obs_source_duplicate()`, `get_new_filter_name()`,
//! `obs_scene_duplicate()`, `dup_child()` and `dup_group()`.
//!
//! The `steps_*` cases port `test/cmocka/test_source_duplicate_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations, the models of the callers included.
//!
//! The `edge_*` cases are new: the two intentional differences of the port
//! (a create that returned no source stops, and a repeated source reuses
//! only an earlier copy that was added to the new scene), the filter number
//! at the end of `int`, and proptests of every decision against models
//! written another way: any type, flags and duplicate type, the filter
//! names over random taken sets, and the item loop of
//! `obs_scene_duplicate()` over random scenes with repeated sources, groups
//! and items that were not added.

use obs_source::source_duplicate::{
    DO_NOT_DUPLICATE, DUP_COPY, DUP_PRIVATE_COPY, DUP_PRIVATE_REFS, DUP_REFS, ItemRoute, Reuse,
    Step, TYPE_FILTER, TYPE_SCENE, copies_filters, created_step, filter_number, flags_step,
    group_step, item_route, keeps_name, keeps_unique, private, resizes, reuse, scene_step,
    scene_type, type_step, unique,
};
use proptest::prelude::*;

/// `enum obs_source_type` (`obs-source.h`).
const INPUT: i32 = 0;
const FILTER: i32 = TYPE_FILTER;
const TRANSITION: i32 = 2;
const SCENE: i32 = TYPE_SCENE;

// ---------------------------------------------------------------------------
// obs_source_duplicate()

#[derive(Debug, Clone, Copy)]
struct SourceModel {
    source_type: i32,
    /// `obs_scene_from_source()` finds a scene.
    scene: bool,
    /// `obs_group_from_source()` finds a group.
    group: bool,
    output_flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Null,
    Ref,
    SceneCopy,
    ScenePrivateCopy,
    CreateWithFilters,
    CreateNoFilters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Asks {
    scene_lookups: u32,
    group_lookups: u32,
    flag_reads: u32,
    creates: u32,
}

/// A model of `obs_source_duplicate()`: each lookup and read only where the
/// step asks for it; `created` is whether the create returns a source.
fn duplicate_route(s: SourceModel, create_private: bool, created: bool) -> (Outcome, Asks) {
    let mut asks = Asks::default();

    if type_step(s.source_type) == Step::AskScene {
        asks.scene_lookups += 1;
        let mut step = scene_step(s.scene, create_private);
        if step == Step::Ref {
            return (Outcome::Ref, asks);
        }
        if step == Step::AskGroup {
            asks.group_lookups += 1;
            step = group_step(s.group);
        }
        if step != Step::SceneCopy {
            return (Outcome::Null, asks);
        }
        let outcome = if scene_type(create_private) == DUP_PRIVATE_COPY {
            Outcome::ScenePrivateCopy
        } else {
            Outcome::SceneCopy
        };
        return (outcome, asks);
    }

    asks.flag_reads += 1;
    if flags_step(s.output_flags) == Step::Ref {
        return (Outcome::Ref, asks);
    }

    asks.creates += 1;
    if created_step(created) != Step::CopyState {
        return (Outcome::Null, asks);
    }

    let outcome = if copies_filters(s.source_type) {
        Outcome::CreateWithFilters
    } else {
        Outcome::CreateNoFilters
    };
    (outcome, asks)
}

#[test]
fn steps_type_step() {
    assert_eq!(type_step(SCENE), Step::AskScene);
    assert_eq!(type_step(INPUT), Step::AskFlags);
    assert_eq!(type_step(FILTER), Step::AskFlags);
    assert_eq!(type_step(TRANSITION), Step::AskFlags);
    // any other value is not a scene
    assert_eq!(type_step(-1), Step::AskFlags);
    assert_eq!(type_step(4), Step::AskFlags);
    assert_eq!(type_step(i32::MAX), Step::AskFlags);
}

#[test]
fn steps_scene_steps() {
    // a public duplicate of a scene shares it; a private one copies it
    assert_eq!(scene_step(true, false), Step::Ref);
    assert_eq!(scene_step(true, true), Step::SceneCopy);
    // no scene: look for a group, whichever kind is asked
    assert_eq!(scene_step(false, false), Step::AskGroup);
    assert_eq!(scene_step(false, true), Step::AskGroup);

    assert_eq!(group_step(true), Step::SceneCopy);
    assert_eq!(group_step(false), Step::None);

    assert_eq!(scene_type(false), DUP_COPY);
    assert_eq!(scene_type(true), DUP_PRIVATE_COPY);
}

#[test]
fn steps_flags_and_filters() {
    assert_eq!(flags_step(0), Step::Create);
    assert_eq!(flags_step(DO_NOT_DUPLICATE), Step::Ref);
    // only that bit counts
    assert_eq!(flags_step(!DO_NOT_DUPLICATE), Step::Create);
    assert_eq!(flags_step(u32::MAX), Step::Ref);
    assert_eq!(flags_step(0x3 | DO_NOT_DUPLICATE), Step::Ref);

    // a source was created: copy its state
    assert_eq!(created_step(true), Step::CopyState);

    assert!(copies_filters(INPUT));
    assert!(copies_filters(TRANSITION));
    assert!(copies_filters(SCENE));
    assert!(!copies_filters(FILTER));
    assert!(copies_filters(-1));
}

const fn src(source_type: i32, scene: bool, group: bool, output_flags: u32) -> SourceModel {
    SourceModel {
        source_type,
        scene,
        group,
        output_flags,
    }
}
const fn asks(scene_lookups: u32, group_lookups: u32, flag_reads: u32, creates: u32) -> Asks {
    Asks {
        scene_lookups,
        group_lookups,
        flag_reads,
        creates,
    }
}
const ND: u32 = DO_NOT_DUPLICATE;

const ROUTE_CASES: &[(&str, SourceModel, bool, Outcome, Asks)] = &[
    (
        "public scene",
        src(SCENE, true, false, 0),
        false,
        Outcome::Ref,
        asks(1, 0, 0, 0),
    ),
    (
        "private scene",
        src(SCENE, true, false, 0),
        true,
        Outcome::ScenePrivateCopy,
        asks(1, 0, 0, 0),
    ),
    (
        "public group",
        src(SCENE, false, true, 0),
        false,
        Outcome::SceneCopy,
        asks(1, 1, 0, 0),
    ),
    (
        "private group",
        src(SCENE, false, true, 0),
        true,
        Outcome::ScenePrivateCopy,
        asks(1, 1, 0, 0),
    ),
    // typed as a scene, but neither a scene nor a group
    (
        "unknown scene",
        src(SCENE, false, false, 0),
        false,
        Outcome::Null,
        asks(1, 1, 0, 0),
    ),
    (
        "unknown scene private",
        src(SCENE, false, false, 0),
        true,
        Outcome::Null,
        asks(1, 1, 0, 0),
    ),
    // a scene's flags are never read
    (
        "scene no dup",
        src(SCENE, true, false, ND),
        true,
        Outcome::ScenePrivateCopy,
        asks(1, 0, 0, 0),
    ),
    (
        "input",
        src(INPUT, false, false, 0),
        false,
        Outcome::CreateWithFilters,
        asks(0, 0, 1, 1),
    ),
    (
        "input private",
        src(INPUT, false, false, 0),
        true,
        Outcome::CreateWithFilters,
        asks(0, 0, 1, 1),
    ),
    (
        "input no dup",
        src(INPUT, false, false, ND),
        false,
        Outcome::Ref,
        asks(0, 0, 1, 0),
    ),
    (
        "input no dup private",
        src(INPUT, false, false, ND),
        true,
        Outcome::Ref,
        asks(0, 0, 1, 0),
    ),
    (
        "filter",
        src(FILTER, false, false, 0),
        true,
        Outcome::CreateNoFilters,
        asks(0, 0, 1, 1),
    ),
    (
        "filter no dup",
        src(FILTER, false, false, ND),
        true,
        Outcome::Ref,
        asks(0, 0, 1, 0),
    ),
    (
        "transition",
        src(TRANSITION, false, false, 0),
        false,
        Outcome::CreateWithFilters,
        asks(0, 0, 1, 1),
    ),
];

#[test]
fn steps_source_routes() {
    for &(what, source, create_private, outcome, want_asks) in ROUTE_CASES {
        assert_eq!(
            duplicate_route(source, create_private, true),
            (outcome, want_asks),
            "{what}"
        );
    }
}

// ---------------------------------------------------------------------------
// get_new_filter_name()

/// A model of `get_new_filter_name()`: the name, then "<name> N" with the
/// number the core gives, until no filter has it.
fn new_filter_name(name: &str, taken: &[&str]) -> String {
    let mut inc = 0;
    let mut out = name.to_owned();
    while taken.contains(&out.as_str()) {
        inc += 1;
        out = format!("{name} {}", filter_number(inc));
    }
    out
}

#[test]
fn steps_filter_numbers() {
    assert_eq!(filter_number(1), 2);
    assert_eq!(filter_number(2), 3);
    assert_eq!(filter_number(3), 4);
    assert_eq!(filter_number(99), 100);
    assert_eq!(filter_number(i32::MAX - 1), i32::MAX);
}

#[test]
fn steps_filter_names() {
    assert_eq!(new_filter_name("Color", &[]), "Color");
    assert_eq!(new_filter_name("Color", &["Color"]), "Color 2");
    assert_eq!(
        new_filter_name("Color", &["Color 3", "Color", "Color 2"]),
        "Color 4"
    );
    // a gap is taken first
    assert_eq!(new_filter_name("Color", &["Color", "Color 3"]), "Color 2");
    // the numbers follow the original name, not the last try
    assert_eq!(new_filter_name("Color 2", &["Color 2"]), "Color 2 2");
}

// ---------------------------------------------------------------------------
// obs_scene_duplicate()

#[test]
fn steps_scene_flags() {
    // type, unique, private, unique for a scene, unique for a group
    let cases: &[(i32, bool, bool, bool, bool)] = &[
        (DUP_REFS, false, false, false, false),
        (DUP_COPY, true, false, true, false),
        (DUP_PRIVATE_REFS, false, true, false, false),
        (DUP_PRIVATE_COPY, true, true, true, true),
        // outside the enum: neither, and a group never copies
        (-1, false, false, false, false),
        (4, false, false, false, false),
    ];

    for &(dup_type, want_unique, want_private, scene_unique, group_unique) in cases {
        let u = unique(dup_type);
        assert_eq!(u, want_unique, "unique({dup_type})");
        assert_eq!(private(dup_type), want_private, "private({dup_type})");
        assert_eq!(
            keeps_unique(u, false, dup_type),
            scene_unique,
            "scene {dup_type}"
        );
        assert_eq!(
            keeps_unique(u, true, dup_type),
            group_unique,
            "group {dup_type}"
        );
    }

    // the override only clears the flag
    assert!(!keeps_unique(false, false, DUP_PRIVATE_COPY));
    assert!(!keeps_unique(false, true, DUP_PRIVATE_COPY));
    assert!(keeps_unique(true, false, DUP_REFS));
    assert!(!keeps_unique(true, true, DUP_REFS));
}

#[test]
fn steps_item_routes() {
    assert_eq!(item_route(true, true), ItemRoute::Group);
    assert_eq!(item_route(true, false), ItemRoute::Group);
    assert_eq!(item_route(false, true), ItemRoute::Child);
    assert_eq!(item_route(false, false), ItemRoute::Ref);
}

#[test]
fn steps_names_and_resize() {
    assert!(keeps_name(true));
    assert!(!keeps_name(false));
    assert!(resizes(true));
    assert!(!resizes(false));
}

#[test]
fn steps_reuse_step() {
    // the same source, its copy in the new scene: reuse it
    let same = reuse(1, 1, true);
    assert!(same == Reuse::AtIndex || same == Reuse::Copy);
    // another source: go on
    assert_eq!(reuse(1, 2, true), Reuse::None);
    assert_eq!(reuse(2, 1, false), Reuse::None);
    assert_eq!(reuse(0, 1, true), Reuse::None);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Plan {
    Group,
    New,
    /// The old item whose copy is shared.
    Reuse(usize),
    Ref,
}

/// A model of `obs_scene_duplicate()`'s item loop. `added[i]` is whether
/// item `i`'s source was added to the new scene; it is read only for the
/// items before the one being copied.
fn scene_copies(
    dup_type: i32,
    scene_is_group: bool,
    sources: &[u32],
    is_group: &[bool],
    added: &[bool],
) -> Vec<Plan> {
    let u = keeps_unique(unique(dup_type), scene_is_group, dup_type);
    let mut out = Vec::new();
    for (idx, (&source, &group)) in sources.iter().zip(is_group).enumerate() {
        out.push(match item_route(group, u) {
            ItemRoute::Group => Plan::Group,
            ItemRoute::Child => {
                let mut copy = Plan::New;
                for i in 0..idx {
                    if reuse(sources[i], source, added[i]) != Reuse::None {
                        copy = Plan::Reuse(i);
                        break;
                    }
                }
                copy
            }
            ItemRoute::Ref => Plan::Ref,
        });
    }
    out
}

#[test]
fn steps_scene_one_source_three_times() {
    let all = [true; 3];
    // one copy, two references to it
    assert_eq!(
        scene_copies(DUP_COPY, false, &[1, 1, 1], &[false; 3], &all),
        [Plan::New, Plan::Reuse(0), Plan::Reuse(0)]
    );
    // references only: each item refers to the old source
    assert_eq!(
        scene_copies(DUP_REFS, false, &[1, 1, 1], &[false; 3], &all),
        [Plan::Ref; 3]
    );
}

#[test]
fn steps_scene_mixed_sources() {
    let sources = [1, 2, 1, 3, 2, 4, 4];
    let groups = [false, false, false, false, false, true, true];
    assert_eq!(
        scene_copies(DUP_PRIVATE_COPY, false, &sources, &groups, &[true; 7]),
        [
            Plan::New,
            Plan::New,
            Plan::Reuse(0),
            Plan::New,
            Plan::Reuse(1),
            // a group used twice is copied twice
            Plan::Group,
            Plan::Group,
        ]
    );
}

#[test]
fn steps_scene_and_group_types() {
    let cases: &[(i32, bool, Plan)] = &[
        (DUP_REFS, false, Plan::Ref),
        (DUP_COPY, false, Plan::New),
        (DUP_PRIVATE_REFS, false, Plan::Ref),
        (DUP_PRIVATE_COPY, false, Plan::New),
        // a public copy of a group keeps its sources
        (DUP_REFS, true, Plan::Ref),
        (DUP_COPY, true, Plan::Ref),
        (DUP_PRIVATE_REFS, true, Plan::Ref),
        (DUP_PRIVATE_COPY, true, Plan::New),
    ];

    for &(dup_type, scene_is_group, child) in cases {
        assert_eq!(
            scene_copies(
                dup_type,
                scene_is_group,
                &[1, 2],
                &[false, true],
                &[true; 2]
            ),
            // a group inside is always copied, with the same type
            [child, Plan::Group],
            "type {dup_type}, group {scene_is_group}"
        );
    }
}

// ---------------------------------------------------------------------------
// Intentional differences and edges

/// The create returned no source: stop and return NULL, instead of writing
/// the copied state through it (the C has no check).
#[test]
fn edge_failed_create_stops() {
    assert_eq!(created_step(false), Step::None);
    for source_type in [INPUT, FILTER, TRANSITION] {
        let s = SourceModel {
            source_type,
            scene: false,
            group: false,
            output_flags: 0,
        };
        for create_private in [false, true] {
            let (outcome, asks) = duplicate_route(s, create_private, false);
            assert_eq!(outcome, Outcome::Null);
            assert_eq!(asks.creates, 1);
        }
    }
}

/// The reuse step never answers the C's index into the new scene.
#[test]
fn edge_reuse_never_at_index() {
    for earlier in 0..3_u32 {
        for source in 0..3_u32 {
            for copied in [false, true] {
                assert_ne!(reuse(earlier, source, copied), Reuse::AtIndex);
            }
        }
    }
    assert_eq!(reuse(1, 1, true), Reuse::Copy);
}

/// A repeated source whose first earlier item was not added reuses the next
/// earlier item that was, or is copied afresh. The C took the new scene's
/// item at the old index, another item's source once an earlier item was
/// not added.
#[test]
fn edge_reuse_skips_items_not_added() {
    let sources = [1, 1, 2, 1];
    let groups = [false; 4];
    // item 0 was not added: item 1 is copied afresh, item 3 reuses item 1
    assert_eq!(
        scene_copies(
            DUP_COPY,
            false,
            &sources,
            &groups,
            &[false, true, true, true]
        ),
        [Plan::New, Plan::New, Plan::New, Plan::Reuse(1)]
    );
    // neither 0 nor 1 was added: item 3 is copied afresh
    assert_eq!(
        scene_copies(
            DUP_COPY,
            false,
            &sources,
            &groups,
            &[false, false, true, true]
        ),
        [Plan::New, Plan::New, Plan::New, Plan::New]
    );
    // an item that was not added before another source changes nothing
    assert_eq!(
        scene_copies(
            DUP_COPY,
            false,
            &[2, 1, 1],
            &[false; 3],
            &[false, true, true]
        ),
        [Plan::New, Plan::New, Plan::Reuse(1)]
    );
}

/// The C's `int` would overflow past `INT_MAX`; the number stops there.
#[test]
fn edge_filter_number_saturates() {
    assert_eq!(filter_number(i32::MAX), i32::MAX);
    assert_eq!(filter_number(0), 1);
    assert_eq!(filter_number(-1), 0);
    assert_eq!(filter_number(i32::MIN), i32::MIN + 1);
}

fn model_unique(dup_type: i32) -> bool {
    matches!(dup_type, 1 | 3)
}

fn model_private(dup_type: i32) -> bool {
    matches!(dup_type, 2 | 3)
}

proptest! {
    #[test]
    fn edge_source_steps_match_model(
        source_type in prop_oneof![Just(SCENE), Just(INPUT), Just(FILTER), any::<i32>()],
        scene in any::<bool>(),
        group in any::<bool>(),
        output_flags in any::<u32>(),
        create_private in any::<bool>(),
        created in any::<bool>(),
    ) {
        let s = SourceModel { source_type, scene, group, output_flags };
        let (outcome, asks) = duplicate_route(s, create_private, created);
        let is_scene = source_type == 3;
        let want = if is_scene {
            if scene && !create_private {
                Outcome::Ref
            } else if !scene && !group {
                Outcome::Null
            } else if create_private {
                Outcome::ScenePrivateCopy
            } else {
                Outcome::SceneCopy
            }
        } else if output_flags & 0x80 != 0 {
            Outcome::Ref
        } else if !created {
            Outcome::Null
        } else if source_type == 1 {
            Outcome::CreateNoFilters
        } else {
            Outcome::CreateWithFilters
        };
        prop_assert_eq!(outcome, want);
        prop_assert_eq!(asks.scene_lookups, u32::from(is_scene));
        prop_assert_eq!(asks.group_lookups, u32::from(is_scene && !scene));
        prop_assert_eq!(asks.flag_reads, u32::from(!is_scene));
        prop_assert_eq!(asks.creates, u32::from(!is_scene && output_flags & 0x80 == 0));
    }

    #[test]
    fn edge_scene_flags_match_model(
        dup_type in prop_oneof![0..4_i32, any::<i32>()],
        u in any::<bool>(),
        is_group in any::<bool>(),
    ) {
        prop_assert_eq!(unique(dup_type), model_unique(dup_type));
        prop_assert_eq!(private(dup_type), model_private(dup_type));
        prop_assert_eq!(keeps_unique(u, is_group, dup_type), u && (!is_group || dup_type == 3));
        prop_assert_eq!(keeps_name(is_group), is_group);
        prop_assert_eq!(resizes(is_group), is_group);
    }

    #[test]
    fn edge_filter_number_matches_model(taken in any::<i32>()) {
        prop_assert_eq!(i64::from(filter_number(taken)), (i64::from(taken) + 1).min(i64::from(i32::MAX)));
    }

    #[test]
    fn edge_filter_names_are_new(
        name in "[A-C]{1,2}",
        taken in proptest::collection::vec("[A-C]{1,2}( [1-9])?", 0..12),
    ) {
        let taken_refs: Vec<&str> = taken.iter().map(String::as_str).collect();
        let got = new_filter_name(&name, &taken_refs);
        prop_assert!(!taken_refs.contains(&got.as_str()));
        // the name itself, or the lowest free "<name> N" from 2 up
        if !taken_refs.contains(&name.as_str()) {
            prop_assert_eq!(got, name);
        } else {
            let n = (2..).find(|n| !taken_refs.contains(&format!("{name} {n}").as_str()));
            prop_assert_eq!(Some(got), n.map(|n| format!("{name} {n}")));
        }
    }

    /// The item loop over random scenes of up to 12 items with repeated
    /// sources, groups and items that were not added, against a model
    /// written another way: a copied item reuses the first earlier item
    /// with the same source that was added, else it is copied afresh.
    #[test]
    fn edge_scene_copies_match_model(
        dup_type in 0..4_i32,
        scene_is_group in any::<bool>(),
        items in proptest::collection::vec((0..5_u32, any::<bool>(), any::<bool>()), 0..=12),
    ) {
        let sources: Vec<u32> = items.iter().map(|i| i.0).collect();
        // sources 0 and 1 are groups
        let groups: Vec<bool> = sources.iter().map(|&s| s < 2).collect();
        let added: Vec<bool> = items.iter().map(|i| i.1 || i.2).collect();
        let got = scene_copies(dup_type, scene_is_group, &sources, &groups, &added);

        let copies = dup_type == 1 || dup_type == 3;
        let copies = copies && (!scene_is_group || dup_type == 3);
        for (idx, copy) in got.iter().enumerate() {
            let want = if groups[idx] {
                Plan::Group
            } else if !copies {
                Plan::Ref
            } else {
                sources[..idx]
                    .iter()
                    .zip(&added)
                    .position(|(&s, &a)| s == sources[idx] && a)
                    .map_or(Plan::New, Plan::Reuse)
            };
            prop_assert_eq!(*copy, want, "item {}", idx);
        }
    }

    #[test]
    fn edge_item_route_matches_model(is_group in any::<bool>(), u in any::<bool>()) {
        let want = match (is_group, u) {
            (true, _) => ItemRoute::Group,
            (false, true) => ItemRoute::Child,
            (false, false) => ItemRoute::Ref,
        };
        prop_assert_eq!(item_route(is_group, u), want);
    }
}
