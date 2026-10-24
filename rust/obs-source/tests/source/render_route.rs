//! Tier 1: `obs_source::render_route`, the source and filter render routing.
//!
//! The `steps_*` cases port `test/cmocka/test_source_render_route_steps.c`
//! 1:1, under the same case names, in the same order, with the same inputs
//! and expectations.
//!
//! The `edge_*` cases are new: every rule over every one of the 2^16 low
//! output flag patterns and the high bits, over types, direct render choices
//! and spaces from -300 to 300 and near the ends of `i32`; every step of the
//! draw against the old `if`/`else if` chains written out, with the values
//! each one reads; and proptests of each over full-range values.

use obs_source::render_route::{
    ALLOW_DIRECT_RENDERING, Draw, FLAG_ASYNC, FLAG_AUDIO, FLAG_CAP_DISABLED, FLAG_CUSTOM_DRAW,
    FLAG_SRGB, FLAG_VIDEO, FilterCheck, GS_CS_709_EXTENDED, GS_CS_709_SCRGB, GS_CS_SRGB,
    GS_CS_SRGB_16F, GS_RG16, NO_DIRECT_RENDERING, TYPE_FILTER, TYPE_INPUT, TYPE_SCENE,
    TYPE_TRANSITION, TechEnd, TechName, async_reads_flags, async_reads_rendering_filter,
    async_update, bypass, bypass_reads_space, draw, draw_next, draw_next_value, filter_check,
    filter_render, inactive, main_custom_draw, main_default_effect, main_reads_filters,
    main_srgb_aware, reads_enabled, size_skip, skip, tech_end, tech_end_linear_srgb, tech_name,
    texrender_stale, video_none, video_reads_flags,
};
use proptest::prelude::*;

/// Source types, as the C test's `TYPES`.
const TYPES: [i32; 9] = [
    TYPE_INPUT,
    TYPE_FILTER,
    TYPE_TRANSITION,
    TYPE_SCENE,
    -1,
    4,
    99,
    i32::MIN,
    i32::MAX,
];

/// Colour spaces, as the C test's `SPACES`.
const SPACES: [i32; 8] = [
    GS_CS_SRGB,
    GS_CS_SRGB_16F,
    GS_CS_709_EXTENDED,
    GS_CS_709_SCRGB,
    -1,
    4,
    i32::MIN,
    i32::MAX,
];

/// Direct render choices, as the C test's `ALLOW`.
const ALLOW: [i32; 6] = [
    NO_DIRECT_RENDERING,
    ALLOW_DIRECT_RENDERING,
    2,
    -1,
    i32::MIN,
    i32::MAX,
];

/// The flag bits the C test combines.
const FLAG_BITS: [u32; 6] = [
    FLAG_VIDEO,
    FLAG_ASYNC,
    FLAG_CUSTOM_DRAW,
    FLAG_SRGB,
    FLAG_AUDIO,
    FLAG_CAP_DISABLED,
];

/// Every combination of [`FLAG_BITS`], then `u32::MAX`, as the C test's
/// `flags_of()`.
fn flag_combos() -> Vec<u32> {
    let mut all: Vec<u32> = (0u32..64)
        .map(|combo| {
            FLAG_BITS
                .iter()
                .enumerate()
                .filter(|(bit, _)| combo & (1 << bit) != 0)
                .fold(0, |flags, (_, flag)| flags | flag)
        })
        .collect();
    all.push(u32::MAX);
    all
}

/// A walk through the draw: the route and the `Read*` steps, as the C
/// test's `walk_route()`.
fn walk(first: Draw, values: [bool; 4]) -> (Draw, Vec<Draw>) {
    let mut route = first;
    let mut reads = Vec::new();
    while route.is_read() {
        assert!(reads.len() < 8, "route does not end");
        reads.push(route);
        let value = match route {
            Draw::ReadRenderingFilter => values[0],
            Draw::ReadVideoRender | Draw::ReadTargetVideoRender => values[1],
            Draw::ReadFilterTarget => values[2],
            _ => values[3],
        };
        route = draw_next(route, value);
    }
    (route, reads)
}

/// `source_render_video()`'s old chain, with the values it reads.
fn old_draw(has_filters: bool, values: [bool; 4]) -> (Draw, Vec<Draw>) {
    let [rendering_filter, video_render, filter_target, deinterlacing] = values;
    let mut reads = Vec::new();
    if has_filters {
        reads.push(Draw::ReadRenderingFilter);
    }
    if has_filters && !rendering_filter {
        return (Draw::Filters, reads);
    }
    reads.push(Draw::ReadVideoRender);
    if video_render {
        return (Draw::Main, reads);
    }
    reads.push(Draw::ReadFilterTarget);
    if filter_target {
        return (Draw::Target, reads);
    }
    reads.push(Draw::ReadDeinterlacing);
    if deinterlacing {
        (Draw::Deinterlace, reads)
    } else {
        (Draw::Async, reads)
    }
}

/// `obs_source_skip_video_filter()`'s old chain, with the values it reads.
fn old_skip(same: bool, flags: u32, video_render: bool, deinterlacing: bool) -> (Draw, Vec<Draw>) {
    let custom_draw = flags & FLAG_CUSTOM_DRAW != 0;
    let async_ = flags & FLAG_ASYNC != 0;
    if !same {
        return (Draw::Target, Vec::new());
    }
    if !custom_draw && !async_ {
        return (Draw::Default, Vec::new());
    }
    let mut reads = vec![Draw::ReadTargetVideoRender];
    if video_render {
        return (Draw::Main, reads);
    }
    reads.push(Draw::ReadTargetDeinterlacing);
    if deinterlacing {
        (Draw::Deinterlace, reads)
    } else {
        (Draw::Async, reads)
    }
}

/// `can_bypass()` written out.
fn old_bypass_reads(same: bool, allow: i32, ff: u32, pf: u32) -> bool {
    same && allow == ALLOW_DIRECT_RENDERING
        && pf & FLAG_CUSTOM_DRAW == 0
        && pf & FLAG_ASYNC == 0
        && ff & FLAG_SRGB == pf & FLAG_SRGB
}

// --- enum values ---------------------------------------------------------

#[test]
fn steps_enum_values() {
    // The C acts on these numbers.
    assert_eq!(FilterCheck::Ready.value(), 0);
    assert_eq!(FilterCheck::NoTarget.value(), 1);
    assert_eq!(FilterCheck::NoParent.value(), 2);
    let draws = [
        (Draw::Default, 0),
        (Draw::Filters, 1),
        (Draw::Main, 2),
        (Draw::Target, 3),
        (Draw::Deinterlace, 4),
        (Draw::Async, 5),
        (Draw::ReadRenderingFilter, 6),
        (Draw::ReadVideoRender, 7),
        (Draw::ReadFilterTarget, 8),
        (Draw::ReadDeinterlacing, 9),
        (Draw::ReadTargetVideoRender, 10),
        (Draw::ReadTargetDeinterlacing, 11),
    ];
    for (d, v) in draws {
        assert_eq!(d.value(), v, "{d:?}");
        assert_eq!(d as u32, v, "{d:?} discriminant");
        assert_eq!(Draw::from_value(v), Some(d));
    }
    assert_eq!(
        Draw::ALL.map(Draw::value),
        core::array::from_fn(|i| u32::try_from(i).unwrap())
    );
    assert_eq!(TechEnd::Bypass.value(), 0);
    assert_eq!(TechEnd::Texture.value(), 1);
    assert_eq!(TYPE_INPUT, 0);
    assert_eq!(TYPE_FILTER, 1);
    assert_eq!(TYPE_TRANSITION, 2);
    assert_eq!(TYPE_SCENE, 3);
    assert_eq!(FLAG_VIDEO, 1);
    assert_eq!(FLAG_AUDIO, 2);
    assert_eq!(FLAG_ASYNC, 4);
    assert_eq!(FLAG_CUSTOM_DRAW, 8);
    assert_eq!(FLAG_CAP_DISABLED, 1 << 10);
    assert_eq!(FLAG_SRGB, 1 << 15);
    assert_eq!(NO_DIRECT_RENDERING, 0);
    assert_eq!(ALLOW_DIRECT_RENDERING, 1);
    assert_eq!(GS_RG16, 22);
}

// --- the target and parent check -----------------------------------------

#[test]
fn steps_filter_target_and_parent() {
    assert_eq!(filter_check(true, true), FilterCheck::Ready);
    assert_eq!(filter_check(false, true), FilterCheck::NoTarget);
    assert_eq!(filter_check(true, false), FilterCheck::NoParent);
    assert_eq!(filter_check(false, false), FilterCheck::NoTarget);
}

// --- source_render_video(): the checks before the draw -------------------

#[test]
fn steps_video_none_every_type_and_flags() {
    for t in TYPES {
        let filter = t == TYPE_FILTER;
        assert_eq!(video_reads_flags(t), !filter, "type {t}");
        for flags in flag_combos() {
            assert_eq!(
                video_none(t, flags),
                !filter && flags & FLAG_VIDEO == 0,
                "type {t} flags {flags:#x}"
            );
        }
    }
}

#[test]
fn steps_video_none_filter_and_audio() {
    assert!(!video_none(TYPE_FILTER, 0));
    assert!(!video_none(TYPE_FILTER, FLAG_AUDIO));
    assert!(video_none(TYPE_INPUT, FLAG_AUDIO));
    assert!(video_none(TYPE_SCENE, 0));
    assert!(!video_none(TYPE_TRANSITION, FLAG_VIDEO));
    assert!(!video_none(TYPE_INPUT, FLAG_ASYNC | FLAG_VIDEO));
    // Characterized, not endorsed: ASYNC without VIDEO is no video.
    assert!(video_none(TYPE_INPUT, FLAG_ASYNC));
}

#[test]
fn steps_async_update_every_type_flags_and_filter() {
    for t in TYPES {
        let input = t == TYPE_INPUT;
        assert_eq!(async_reads_flags(t), input, "type {t}");
        for flags in flag_combos() {
            let async_ = input && flags & FLAG_ASYNC != 0;
            assert_eq!(async_reads_rendering_filter(t, flags), async_);
            for rf in [false, true] {
                assert_eq!(
                    async_update(t, flags, rf),
                    async_ && !rf,
                    "{t} {flags:#x} {rf}"
                );
            }
        }
    }
}

#[test]
fn steps_async_update_cases() {
    let av = FLAG_ASYNC | FLAG_VIDEO;
    assert!(async_update(TYPE_INPUT, av, false));
    assert!(!async_update(TYPE_INPUT, av, true));
    assert!(!async_update(TYPE_INPUT, FLAG_VIDEO, false));
    assert!(!async_update(TYPE_FILTER, av, false));
    assert!(!async_update(TYPE_SCENE, av, false));
    assert!(!async_update(TYPE_TRANSITION, av, false));
    // Characterized, not endorsed: ASYNC without VIDEO still updates.
    assert!(async_update(TYPE_INPUT, FLAG_ASYNC, false));
}

#[test]
fn steps_inactive_every_input() {
    assert!(!reads_enabled(false));
    assert!(reads_enabled(true));
    assert!(inactive(false, false));
    assert!(inactive(false, true));
    assert!(inactive(true, false));
    assert!(!inactive(true, true));
}

// --- the draw, one read at a time ----------------------------------------

#[test]
fn steps_draw_every_source() {
    for m in 0u32..32 {
        let has_filters = m & 1 != 0;
        let values = [m & 2 != 0, m & 4 != 0, m & 8 != 0, m & 16 != 0];
        assert_eq!(
            walk(draw(has_filters), values),
            old_draw(has_filters, values),
            "{m:#b}"
        );
    }
}

#[test]
fn steps_draw_next_every_step() {
    assert_eq!(draw(true), Draw::ReadRenderingFilter);
    assert_eq!(draw(false), Draw::ReadVideoRender);
    let steps = [
        (
            Draw::ReadRenderingFilter,
            Draw::Filters,
            Draw::ReadVideoRender,
        ),
        (Draw::ReadVideoRender, Draw::ReadFilterTarget, Draw::Main),
        (
            Draw::ReadFilterTarget,
            Draw::ReadDeinterlacing,
            Draw::Target,
        ),
        (Draw::ReadDeinterlacing, Draw::Async, Draw::Deinterlace),
        (
            Draw::ReadTargetVideoRender,
            Draw::ReadTargetDeinterlacing,
            Draw::Main,
        ),
        (
            Draw::ReadTargetDeinterlacing,
            Draw::Async,
            Draw::Deinterlace,
        ),
    ];
    for (step, if_false, if_true) in steps {
        assert_eq!(draw_next(step, false), if_false, "{step:?}");
        assert_eq!(draw_next(step, true), if_true, "{step:?}");
    }
    for step in [
        Draw::Default,
        Draw::Filters,
        Draw::Main,
        Draw::Target,
        Draw::Deinterlace,
        Draw::Async,
    ] {
        for v in [false, true] {
            assert_eq!(draw_next(step, v), step);
        }
    }
    // -1, 12, 99, INT_MIN and INT_MAX as the C's enum value.
    for outside in [u32::MAX, 12, 99, 0x8000_0000, 0x7fff_ffff] {
        for v in [false, true] {
            assert_eq!(draw_next_value(outside, v), outside);
        }
    }
}

#[test]
fn steps_skip_every_target_and_flags() {
    for same in [false, true] {
        for flags in flag_combos() {
            for m in 0u32..4 {
                let (vr, de) = (m & 1 != 0, m & 2 != 0);
                assert_eq!(
                    walk(skip(same, flags), [false, vr, false, de]),
                    old_skip(same, flags, vr, de),
                    "{same} {flags:#x} {m}"
                );
            }
        }
    }
}

#[test]
fn steps_skip_cases() {
    assert_eq!(skip(false, FLAG_VIDEO), Draw::Target);
    assert_eq!(skip(false, FLAG_CUSTOM_DRAW), Draw::Target);
    assert_eq!(skip(true, FLAG_VIDEO), Draw::Default);
    assert_eq!(
        skip(true, FLAG_ASYNC | FLAG_VIDEO),
        Draw::ReadTargetVideoRender
    );
    assert_eq!(
        skip(true, FLAG_VIDEO | FLAG_CUSTOM_DRAW),
        Draw::ReadTargetVideoRender
    );
    // The SRGB flag plays no part.
    assert_eq!(skip(true, FLAG_VIDEO | FLAG_SRGB), Draw::Default);
}

// --- obs_source_main_render() --------------------------------------------

#[test]
fn steps_main_render_every_input() {
    assert!(main_reads_filters(false));
    assert!(!main_reads_filters(true));
    for flags in flag_combos() {
        let custom_draw = flags & FLAG_CUSTOM_DRAW != 0;
        assert_eq!(main_custom_draw(flags), custom_draw);
        assert_eq!(main_srgb_aware(flags), flags & FLAG_SRGB != 0);
        for parent in [false, true] {
            for filters in [false, true] {
                assert_eq!(
                    main_default_effect(flags, parent, filters),
                    !parent && !filters && !custom_draw
                );
            }
        }
    }
}

// --- obs_source_process_filter_begin_with_color_space() -------------------

#[test]
fn steps_bypass_every_input() {
    let combos = flag_combos();
    for same in [false, true] {
        for allow in ALLOW {
            for &ff in &combos {
                for &pf in &combos {
                    let reads = old_bypass_reads(same, allow, ff, pf);
                    assert_eq!(bypass_reads_space(same, allow, ff, pf), reads);
                    for s in SPACES {
                        for cs in SPACES {
                            assert_eq!(bypass(same, allow, ff, pf, s, cs), reads && s == cs);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn steps_bypass_cases() {
    let direct = ALLOW_DIRECT_RENDERING;
    assert!(bypass(true, direct, 0, FLAG_VIDEO, GS_CS_SRGB, GS_CS_SRGB));
    assert!(bypass(
        true,
        direct,
        FLAG_SRGB,
        FLAG_VIDEO | FLAG_SRGB,
        GS_CS_709_EXTENDED,
        GS_CS_709_EXTENDED
    ));
    assert!(!bypass(
        true, direct, FLAG_SRGB, FLAG_VIDEO, GS_CS_SRGB, GS_CS_SRGB
    ));
    assert!(!bypass(
        true,
        direct,
        0,
        FLAG_VIDEO | FLAG_SRGB,
        GS_CS_SRGB,
        GS_CS_SRGB
    ));
    assert!(!bypass(
        true,
        direct,
        0,
        FLAG_VIDEO,
        GS_CS_SRGB,
        GS_CS_709_SCRGB
    ));
    assert!(!bypass(
        true,
        direct,
        0,
        FLAG_VIDEO,
        GS_CS_SRGB_16F,
        GS_CS_SRGB
    ));
    assert!(!bypass(
        false, direct, 0, FLAG_VIDEO, GS_CS_SRGB, GS_CS_SRGB
    ));
    assert!(!bypass(
        true,
        NO_DIRECT_RENDERING,
        0,
        FLAG_VIDEO,
        GS_CS_SRGB,
        GS_CS_SRGB
    ));
    assert!(!bypass(
        true,
        direct,
        0,
        FLAG_ASYNC | FLAG_VIDEO,
        GS_CS_SRGB,
        GS_CS_SRGB
    ));
    assert!(!bypass(
        true,
        direct,
        0,
        FLAG_VIDEO | FLAG_CUSTOM_DRAW,
        GS_CS_SRGB,
        GS_CS_SRGB
    ));
    // Characterized, not endorsed: a direct render choice outside the enum
    // is not direct; spaces outside the enum bypass when they are equal.
    assert!(!bypass(true, 2, 0, FLAG_VIDEO, GS_CS_SRGB, GS_CS_SRGB));
    assert!(bypass(true, direct, 0, FLAG_VIDEO, 99, 99));
    // The current space is not looked at when the rest does not hold.
    assert!(!bypass(false, direct, 0, FLAG_VIDEO, GS_CS_SRGB, i32::MIN));
}

#[test]
fn steps_size_skip_every_size() {
    let sizes = [0, 1, 2, 1080, 1920, -1, i32::MIN, i32::MAX];
    for x in sizes {
        for y in sizes {
            assert_eq!(size_skip(x, y), x == 0 || y == 0, "{x} {y}");
        }
    }
}

#[test]
fn steps_texrender_stale_every_format() {
    for a in -2..=GS_RG16 + 2 {
        for b in -2..=GS_RG16 + 2 {
            assert_eq!(texrender_stale(a, b), a != b);
        }
    }
    assert!(!texrender_stale(i32::MIN, i32::MIN));
    assert!(texrender_stale(i32::MIN, i32::MAX));
}

#[test]
fn steps_filter_render_every_input() {
    for same in [false, true] {
        for flags in flag_combos() {
            let plain = flags & FLAG_CUSTOM_DRAW == 0 && flags & FLAG_ASYNC == 0;
            let expected = if same && plain {
                Draw::Default
            } else {
                Draw::Target
            };
            assert_eq!(filter_render(same, flags), expected, "{same} {flags:#x}");
        }
    }
}

// --- obs_source_process_filter_tech_end() --------------------------------

#[test]
fn steps_tech_end_every_input() {
    for flags in flag_combos() {
        assert_eq!(tech_end_linear_srgb(flags), flags & FLAG_SRGB != 0);
    }
    assert_eq!(tech_end(true), TechEnd::Bypass);
    assert_eq!(tech_end(false), TechEnd::Texture);
    assert_eq!(tech_name(false), TechName::Draw);
    assert_eq!(tech_name(true), TechName::Given);
}

// --- edge cases ----------------------------------------------------------

/// Values from -300 to 300 and near the ends of `i32`.
fn edge_ints() -> Vec<i32> {
    let mut all: Vec<i32> = (-300..=300).collect();
    all.extend((0..4).map(|i| i32::MIN + i));
    all.extend((0..4).map(|i| i32::MAX - i));
    all
}

/// Every low 16-bit flag pattern, each also with the high bits set.
fn edge_flags() -> impl Iterator<Item = u32> {
    (0u32..=0xffff).flat_map(|low| [low, low | 0xffff_0000])
}

#[test]
fn edge_checks_every_type_and_flag_pattern() {
    for t in edge_ints() {
        assert_eq!(video_reads_flags(t), t != 1);
        assert_eq!(async_reads_flags(t), t == 0);
    }
    for flags in edge_flags() {
        for t in [
            TYPE_INPUT,
            TYPE_FILTER,
            TYPE_TRANSITION,
            TYPE_SCENE,
            -1,
            i32::MAX,
        ] {
            assert_eq!(video_none(t, flags), t != 1 && flags & 1 == 0);
            let async_ = t == 0 && flags & 4 != 0;
            assert_eq!(async_reads_rendering_filter(t, flags), async_);
            assert_eq!(async_update(t, flags, false), async_);
            assert!(!async_update(t, flags, true));
        }
        let custom = flags & 8 != 0;
        assert_eq!(main_custom_draw(flags), custom);
        assert_eq!(main_srgb_aware(flags), flags & 0x8000 != 0);
        assert_eq!(main_default_effect(flags, false, false), !custom);
        assert!(!main_default_effect(flags, true, false));
        assert!(!main_default_effect(flags, false, true));
        assert_eq!(tech_end_linear_srgb(flags), flags & 0x8000 != 0);
        let plain = flags & 0xc == 0;
        assert_eq!(filter_render(true, flags) == Draw::Default, plain);
        assert_eq!(filter_render(false, flags), Draw::Target);
        let s = skip(true, flags);
        assert_eq!(
            s,
            if plain {
                Draw::Default
            } else {
                Draw::ReadTargetVideoRender
            }
        );
        assert_eq!(skip(false, flags), Draw::Target);
    }
}

#[test]
fn edge_bypass_every_flag_pattern_and_space() {
    // The filter's flags matter only through SRGB.
    for pf in edge_flags() {
        for ff in [0, FLAG_SRGB, u32::MAX, u32::MAX ^ FLAG_SRGB] {
            let reads = old_bypass_reads(true, 1, ff, pf);
            assert_eq!(
                bypass_reads_space(true, 1, ff, pf),
                reads,
                "{ff:#x} {pf:#x}"
            );
            assert!(!bypass_reads_space(false, 1, ff, pf));
            assert!(!bypass_reads_space(true, 0, ff, pf));
            assert_eq!(bypass(true, 1, ff, pf, 0, 0), reads);
            assert!(!bypass(true, 1, ff, pf, 0, 1));
        }
    }
    let ints = edge_ints();
    for &allow in &ints {
        assert_eq!(bypass_reads_space(true, allow, 0, 0), allow == 1);
    }
    for &s in &ints {
        for &cs in &[-300, -1, 0, 1, 2, 3, 4, 300, i32::MIN, i32::MAX] {
            assert_eq!(bypass(true, 1, 0, FLAG_VIDEO, s, cs), s == cs);
            assert!(!bypass(true, 0, 0, FLAG_VIDEO, s, cs));
        }
    }
}

#[test]
fn edge_sizes_and_formats() {
    let ints = edge_ints();
    for &x in &ints {
        for &y in &[0, 1, -1, i32::MIN, i32::MAX] {
            assert_eq!(size_skip(x, y), x == 0 || y == 0);
            assert_eq!(size_skip(y, x), x == 0 || y == 0);
        }
        for &f in &[0, 3, 9, 22, -1, i32::MIN, i32::MAX] {
            assert_eq!(texrender_stale(x, f), x != f);
        }
    }
}

#[test]
fn edge_every_step_and_value() {
    // The whole C value range a step can come back as: the enum and its
    // neighbours, and the ends of u32 (an int of -1 or INT_MIN).
    let mut values: Vec<u32> = (0..=40).collect();
    values.extend([u32::MAX, u32::MAX - 1, 0x8000_0000, 0x7fff_ffff]);
    for v in values {
        for b in [false, true] {
            let next = draw_next_value(v, b);
            match Draw::from_value(v) {
                Some(step) => assert_eq!(next, draw_next(step, b).value()),
                None => assert_eq!(next, v),
            }
        }
    }
    // Every step moves forward to a draw in at most four reads.
    for step in Draw::ALL {
        for m in 0u32..16 {
            let (route, reads) = walk(step, [m & 1 != 0, m & 2 != 0, m & 4 != 0, m & 8 != 0]);
            assert!(!route.is_read());
            assert!(reads.len() <= 4, "{step:?} {m}");
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn prop_checks(t in any::<i32>(), flags in any::<u32>(), rf in any::<bool>(),
                   data in any::<bool>(), enabled in any::<bool>()) {
        prop_assert_eq!(video_none(t, flags), t != TYPE_FILTER && flags & FLAG_VIDEO == 0);
        prop_assert_eq!(
            async_update(t, flags, rf),
            t == TYPE_INPUT && flags & FLAG_ASYNC != 0 && !rf
        );
        prop_assert_eq!(inactive(data, enabled), !data || !enabled);
        prop_assert_eq!(reads_enabled(data), data);
    }

    #[test]
    fn prop_draw(has_filters in any::<bool>(), values in any::<[bool; 4]>()) {
        prop_assert_eq!(walk(draw(has_filters), values), old_draw(has_filters, values));
    }

    #[test]
    fn prop_skip(same in any::<bool>(), flags in any::<u32>(), vr in any::<bool>(),
                 de in any::<bool>()) {
        prop_assert_eq!(walk(skip(same, flags), [false, vr, false, de]),
                        old_skip(same, flags, vr, de));
    }

    #[test]
    fn prop_bypass(same in any::<bool>(), allow in any::<i32>(), ff in any::<u32>(),
                   pf in any::<u32>(), s in -4i32..8, cs in -4i32..8) {
        let reads = old_bypass_reads(same, allow, ff, pf);
        prop_assert_eq!(bypass_reads_space(same, allow, ff, pf), reads);
        prop_assert_eq!(bypass(same, allow, ff, pf, s, cs), reads && s == cs);
    }

    #[test]
    fn prop_main(flags in any::<u32>(), parent in any::<bool>(), filters in any::<bool>()) {
        prop_assert_eq!(
            main_default_effect(flags, parent, filters),
            !parent && !filters && flags & FLAG_CUSTOM_DRAW == 0
        );
        prop_assert_eq!(main_reads_filters(parent), !parent);
    }

    #[test]
    fn prop_sizes(cx in any::<i32>(), cy in any::<i32>(), a in any::<i32>(), b in any::<i32>()) {
        prop_assert_eq!(size_skip(cx, cy), cx == 0 || cy == 0);
        prop_assert_eq!(texrender_stale(a, b), a != b);
    }
}
