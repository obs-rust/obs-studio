//! Tier 1: `obs_source::video_tick`, the per-frame decisions of
//! `obs_source_video_tick()` and `async_tick()`.
//!
//! The `steps_*` cases port `test/cmocka/test_source_video_tick_steps.c`
//! 1:1: its 12 cases under the same names, in the same order, with the same
//! values and expectations, the "characterized, not endorsed" ones and the
//! copy of the tick's order of steps on a stand-in source included. The C
//! calls the hidden `obs_source_video_tick_*()` functions; here the safe
//! core is called directly. The C's `long` counts are `i64` here (the shim
//! widens `c_long`, 32 bits on Windows), so the ends of `long` are the ends
//! of `i64` and of `i32`.
//!
//! The `edge_*` cases are new: every type in a wide range and every flag
//! bit against the rule, every count against a model, every edge of 38
//! special counts, and proptests over full-range values and over runs of
//! ticks through a model of the caller.

use obs_source::video_tick::{
    ASYNC, AsyncRoute, CONTROLLABLE_MEDIA, Edge, TYPE_TRANSITION, async_route, async_tick,
    calls_tick, deferred_update, edge, media_actions, transition_tick,
};
use proptest::prelude::*;

// `enum obs_source_type` (obs-source.h).
const TYPE_INPUT: i32 = 0;
const TYPE_FILTER: i32 = 1;
const TYPE_SCENE: i32 = 3;
// Other output flags (obs-source.h).
const VIDEO: u32 = 1 << 0;
const AUDIO: u32 = 1 << 1;
const COMPOSITE: u32 = 1 << 6;
const ASYNC_VIDEO: u32 = ASYNC | VIDEO;

/// The source types, then values outside the enum.
const TYPES: [i32; 8] = [
    TYPE_INPUT,
    TYPE_FILTER,
    TYPE_TRANSITION,
    TYPE_SCENE,
    -1,
    4,
    i32::MIN,
    i32::MAX,
];

/// Loaded counts: 0, the usual ones, negative ones and the ends of `long`
/// (64-bit here; the 32-bit ends are in `SPECIAL_COUNTS`).
const COUNTS: [i64; 11] = [
    0,
    1,
    2,
    3,
    100,
    -1,
    -2,
    i64::MIN,
    i64::MIN + 1,
    i64::MAX,
    i64::MAX - 1,
];

#[test]
fn steps_enum_values() {
    assert_eq!(Edge::None as i32, 0);
    assert_eq!(Edge::On as i32, 1);
    assert_eq!(Edge::Off as i32, 2);
    assert_eq!(AsyncRoute::Closest as i32, 0);
    assert_eq!(AsyncRoute::Deinterlace as i32, 1);

    // the header's mirrors of obs-source.h
    assert_eq!(TYPE_TRANSITION, 2);
    assert_eq!(ASYNC, 1 << 2);
    assert_eq!(CONTROLLABLE_MEDIA, 1 << 13);
}

#[test]
fn steps_transition_every_type() {
    for t in TYPES {
        assert_eq!(transition_tick(t), t == TYPE_TRANSITION, "type {t}");
    }

    // every value near the transition type, so only 2 is one
    for t in -64..=64 {
        assert_eq!(transition_tick(t), t == 2, "type {t}");
    }
}

#[test]
fn steps_async_every_bit() {
    assert!(!async_tick(0));
    assert!(async_tick(u32::MAX));
    assert!(!async_tick(!ASYNC));
    assert!(async_tick(ASYNC_VIDEO));
    assert!(async_tick(ASYNC | CONTROLLABLE_MEDIA));
    assert!(!async_tick(VIDEO | AUDIO | CONTROLLABLE_MEDIA));

    for bit in 0..32 {
        let flag = 1u32 << bit;
        assert_eq!(async_tick(flag), bit == 2, "bit {bit}");
        assert_eq!(async_tick(!flag), bit != 2, "all but bit {bit}");
    }
}

#[test]
fn steps_media_every_bit() {
    assert!(!media_actions(0));
    assert!(media_actions(u32::MAX));
    assert!(!media_actions(!CONTROLLABLE_MEDIA));
    assert!(media_actions(CONTROLLABLE_MEDIA | ASYNC_VIDEO));
    assert!(!media_actions(ASYNC_VIDEO | AUDIO));

    for bit in 0..32 {
        let flag = 1u32 << bit;
        assert_eq!(media_actions(flag), bit == 13, "bit {bit}");
        assert_eq!(media_actions(!flag), bit != 13, "all but bit {bit}");
    }
}

/// Each type with each combination of the two flags and of other bits: the
/// three steps are independent. Characterized, not endorsed: the type is
/// not checked against the flags.
#[test]
fn steps_steps_every_type_and_flags() {
    const OTHER: [u32; 4] = [0, VIDEO, AUDIO | COMPOSITE, !(ASYNC | CONTROLLABLE_MEDIA)];

    for (t, &ty) in TYPES.iter().enumerate() {
        for combo in 0..4u32 {
            for other in OTHER {
                let mut flags = other;
                if combo & 1 != 0 {
                    flags |= ASYNC;
                }
                if combo & 2 != 0 {
                    flags |= CONTROLLABLE_MEDIA;
                }

                assert_eq!(transition_tick(ty), t == 2);
                assert_eq!(async_tick(flags), combo & 1 != 0);
                assert_eq!(media_actions(flags), combo & 2 != 0);
            }
        }
    }
}

/// Only a count above 0 is due. Characterized, not endorsed: a negative
/// count is not due, and neither is 0.
#[test]
fn steps_deferred_update_counts() {
    for count in COUNTS {
        assert_eq!(deferred_update(count), count > 0, "count {count}");
    }

    for count in -100..=100 {
        assert_eq!(deferred_update(count), count > 0, "count {count}");
    }
}

/// Both edges in both directions and no edge.
#[test]
fn steps_edge_both_directions() {
    assert_eq!(edge(1, false), Edge::On);
    assert_eq!(edge(0, true), Edge::Off);
    assert_eq!(edge(0, false), Edge::None);
    assert_eq!(edge(1, true), Edge::None);

    // any count but 0 is showing or active
    assert_eq!(edge(2, true), Edge::None);
    assert_eq!(edge(2, false), Edge::On);
}

/// Characterized, not endorsed: a negative count (one release too many) is
/// not 0, so it shows or activates the source and keeps it so.
#[test]
fn steps_edge_every_count() {
    for refs in COUNTS {
        let now = refs != 0;
        assert_eq!(edge(refs, now), Edge::None, "refs {refs}");
        assert_eq!(
            edge(refs, !now),
            if now { Edge::On } else { Edge::Off },
            "refs {refs}"
        );
    }

    assert_eq!(edge(-1, false), Edge::On);
    assert_eq!(edge(-1, true), Edge::None);
    assert_eq!(edge(i64::MIN, false), Edge::On);
    assert_eq!(edge(i64::MAX, true), Edge::None);
}

/// The caller's store on an edge.
fn store(flag: &mut bool, e: Edge) {
    if e != Edge::None {
        *flag = e == Edge::On;
    }
}

/// A run of ticks from a hidden start: only the moves between 0 and not 0
/// are edges.
#[test]
fn steps_edge_sequence() {
    let refs = [0, 1, 2, 1, 0, 0, 3, 0, -1, 0];
    let mut trace = String::new();
    let mut flag = false;

    for r in refs {
        let e = edge(r, flag);
        trace.push_str(match e {
            Edge::On => "on ",
            Edge::Off => "off ",
            Edge::None => "- ",
        });
        store(&mut flag, e);
    }

    assert_eq!(trace, "- on - - off - on off on off ");
}

/// The callback is called only with data and a callback.
#[test]
fn steps_calls_tick_truth_table() {
    assert!(!calls_tick(false, false));
    assert!(!calls_tick(false, true));
    assert!(!calls_tick(true, false));
    assert!(calls_tick(true, true));
}

#[test]
fn steps_async_route_both() {
    assert_eq!(async_route(false), AsyncRoute::Closest);
    assert_eq!(async_route(true), AsyncRoute::Deinterlace);
}

/// The stand-in source of the C test.
#[derive(Default)]
struct Standin {
    ty: i32,
    flags: u32,
    defer: i64,
    show_refs: i64,
    activate_refs: i64,
    showing: bool,
    active: bool,
    filters: usize,
    has_data: bool,
    has_callback: bool,
    deinterlacing: bool,
    /// A show callback that adds an activate reference, as a scene's child
    /// can.
    show_activates: bool,
    trace: String,
}

impl Standin {
    fn put(&mut self, what: &str) {
        self.trace.push_str(what);
        self.trace.push(' ');
    }

    fn put_filter(&mut self, what: &str, i: usize) {
        self.put(&format!("{what}(f{i})"));
    }

    /// The copy of `async_tick()`'s order of steps.
    fn async_tick_copy(&mut self) {
        self.put("lock");
        if async_route(self.deinterlacing) == AsyncRoute::Deinterlace {
            self.put("deinterlace");
        } else {
            self.put("closest");
        }
        if async_route(self.deinterlacing) == AsyncRoute::Deinterlace {
            self.put("filter(prev)");
        }
        self.put("filter(cur)");
        self.put("unlock");
    }

    /// The copy of `obs_source_video_tick()`'s order of steps.
    fn tick_copy(&mut self) {
        self.trace.clear();

        if transition_tick(self.ty) {
            self.put("transition");
        }
        if async_tick(self.flags) {
            self.async_tick_copy();
        }
        if media_actions(self.flags) {
            self.put("media");
        }
        if deferred_update(self.defer) {
            self.put("deferred");
        }

        let e = edge(self.show_refs, self.showing);
        if e != Edge::None {
            let now = e == Edge::On;
            self.put(if now { "show" } else { "hide" });
            if now && self.show_activates {
                self.activate_refs += 1;
            }
            for i in (0..self.filters).rev() {
                self.put_filter(if now { "show" } else { "hide" }, i);
            }
            self.showing = now;
        }

        // loaded after the show edge
        let e = edge(self.activate_refs, self.active);
        if e != Edge::None {
            let now = e == Edge::On;
            self.put(if now { "activate" } else { "deactivate" });
            for i in (0..self.filters).rev() {
                self.put_filter(if now { "activate" } else { "deactivate" }, i);
            }
            self.active = now;
        }

        if calls_tick(self.has_data, self.has_callback) {
            self.put("video_tick");
        }
    }
}

#[test]
fn steps_tick_order() {
    let mut s = Standin {
        ty: TYPE_INPUT,
        show_refs: 1,
        activate_refs: 1,
        has_data: true,
        has_callback: true,
        ..Standin::default()
    };
    s.tick_copy();
    assert_eq!(s.trace, "show activate video_tick ");
    assert!(s.showing);
    assert!(s.active);
    s.tick_copy();
    assert_eq!(s.trace, "video_tick ");

    let mut all = Standin {
        ty: TYPE_TRANSITION,
        flags: ASYNC_VIDEO | CONTROLLABLE_MEDIA,
        defer: 1,
        show_refs: 2,
        activate_refs: 1,
        filters: 3,
        has_data: true,
        has_callback: true,
        deinterlacing: true,
        ..Standin::default()
    };
    all.tick_copy();
    assert_eq!(
        all.trace,
        "transition lock deinterlace filter(prev) filter(cur) unlock media deferred show \
         show(f2) show(f1) show(f0) activate activate(f2) activate(f1) activate(f0) video_tick "
    );
    all.show_refs = 0;
    all.activate_refs = 0;
    all.defer = 0;
    all.deinterlacing = false;
    all.has_callback = false;
    all.tick_copy();
    assert_eq!(
        all.trace,
        "transition lock closest filter(cur) unlock media hide hide(f2) hide(f1) hide(f0) \
         deactivate deactivate(f2) deactivate(f1) deactivate(f0) "
    );
    assert!(!all.showing);
    assert!(!all.active);

    // the activate refs are loaded after the show edge
    let mut child = Standin {
        ty: TYPE_SCENE,
        show_refs: 1,
        show_activates: true,
        ..Standin::default()
    };
    child.tick_copy();
    assert_eq!(child.trace, "show activate ");

    // no data: no callback, the edges still run
    let mut no_data = Standin {
        ty: TYPE_FILTER,
        show_refs: 1,
        has_callback: true,
        ..Standin::default()
    };
    no_data.tick_copy();
    assert_eq!(no_data.trace, "show ");
}

/// 38 special counts: around 0, around the ends of `int`, `unsigned int`
/// and `long`, and around 2^32.
fn special_counts() -> Vec<i64> {
    let mut v = vec![0, 1, -1, 2, -2, 3, 100, -100];
    for base in [
        i64::from(i32::MIN),
        i64::from(i32::MAX),
        i64::from(u32::MAX),
        1i64 << 32,
        i64::MIN,
        i64::MAX,
    ] {
        for d in -3i64..=3 {
            if let Some(x) = base.checked_add(d) {
                v.push(x);
            }
        }
    }
    v.sort_unstable();
    v.dedup();
    v
}

#[test]
fn edge_special_counts() {
    let counts = special_counts();
    assert_eq!(counts.len(), 38);
    for refs in counts {
        for was in [false, true] {
            let want = match (refs != 0, was) {
                (true, false) => Edge::On,
                (false, true) => Edge::Off,
                _ => Edge::None,
            };
            assert_eq!(edge(refs, was), want, "refs {refs} was {was}");
        }
        assert_eq!(deferred_update(refs), refs > 0, "count {refs}");
    }
}

#[test]
fn edge_every_type_in_range() {
    for t in -70_000..=70_000 {
        assert_eq!(transition_tick(t), t == TYPE_TRANSITION, "type {t}");
    }
    for t in [i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX] {
        assert!(!transition_tick(t));
    }
}

/// Every flag value with the two bits masked in each way: only its own bit
/// decides each step.
#[test]
fn edge_flags_every_low_word() {
    for low in 0..=u32::from(u16::MAX) {
        for high in [0u32, 0xffff_0000, 0x5555_0000, 0xaaaa_0000] {
            let flags = low | high;
            assert_eq!(async_tick(flags), flags & ASYNC != 0, "flags {flags:#x}");
            assert_eq!(
                media_actions(flags),
                flags & CONTROLLABLE_MEDIA != 0,
                "flags {flags:#x}"
            );
        }
    }
}

/// The tick's step sequence for one source state, from the rules written
/// another way.
fn model_steps(ty: i32, flags: u32) -> (bool, bool, bool) {
    (ty == 2, (flags >> 2) & 1 == 1, (flags >> 13) & 1 == 1)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn edge_proptest_steps(ty in any::<i32>(), flags in any::<u32>()) {
        let (t, a, m) = model_steps(ty, flags);
        prop_assert_eq!(transition_tick(ty), t);
        prop_assert_eq!(async_tick(flags), a);
        prop_assert_eq!(media_actions(flags), m);
    }

    #[test]
    fn edge_proptest_counts(refs in any::<i64>(), was in any::<bool>(), data in any::<bool>(), cb in any::<bool>(), di in any::<bool>()) {
        let e = edge(refs, was);
        let now = refs != 0;
        prop_assert_eq!(e == Edge::None, now == was);
        if e != Edge::None {
            prop_assert_eq!(e == Edge::On, now);
        }
        prop_assert_eq!(deferred_update(refs), refs > 0);
        prop_assert_eq!(calls_tick(data, cb), data && cb);
        prop_assert_eq!(async_route(di) == AsyncRoute::Deinterlace, di);
    }

    /// Runs of ticks through a model of the caller: after every tick the
    /// stored flag is `refs != 0`, and the edges alternate on and off.
    #[test]
    fn edge_proptest_tick_runs(refs in prop::collection::vec(prop_oneof![Just(0i64), Just(1), Just(-1), any::<i64>()], 1..64), start in any::<bool>()) {
        let mut flag = start;
        let mut last: Option<Edge> = None;
        for r in refs {
            let e = edge(r, flag);
            store(&mut flag, e);
            prop_assert_eq!(flag, r != 0);
            if e != Edge::None {
                if let Some(prev) = last {
                    prop_assert_ne!(prev, e);
                }
                last = Some(e);
            }
        }
    }
}
