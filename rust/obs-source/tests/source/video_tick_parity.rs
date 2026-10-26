//! Tier 3: the `obs_source_video_tick_*` shims and the safe core against
//! the original C (`obs-c-oracle`, `oracle/source_video_tick.c`: the
//! unmodified `libobs/obs-source-video-tick.c.h` with its functions
//! renamed).
//!
//! The seven functions take and return `int`, `uint32_t`, `long`, `bool`
//! and two enums by value, so a run compares each `bool` as the byte the C
//! returns (it must be 0 or 1, and the same on both sides) and each enum as
//! its integer. Each input is given to the oracle, to the shim (the
//! `extern "C"` function the hybrid links) and to the core. The counts are
//! the C's `long`: 64 bits here, 32 on Windows, where the special values
//! that do not fit are left out.
//!
//! Runs:
//! - the header's values and enum sizes, and `sizeof(long)` against
//!   [`c_long`];
//! - the transition tick for every type from -100,000 to 100,000, every
//!   65,521st `int` and the ends;
//! - the async and media steps for every low 16-bit flag word under 16
//!   high words, and every 65,521st `uint32_t`;
//! - the deferred update and both edges for every count from -100,000 to
//!   100,000 and 38 special counts, both flags;
//! - the callback rule and the route over every input;
//! - `obs_source_video_tick()`'s order of steps, copied onto a stand-in
//!   source and run once over the oracle's functions and once over the
//!   shims, over runs of ticks with every source type and flag set and
//!   counts that move between 0, the special values and any value, with a
//!   show callback that may add an activate reference: the traces and the
//!   stored flags must match;
//! - proptests of every function over full-range inputs.
//!
//! There is no intentional difference: the shims touch no memory.

use std::os::raw::{c_int, c_long, c_uint};

use obs_c_oracle::source_video_tick as oracle;
use obs_source::ffi::source_video_tick as shim;
use obs_source::video_tick::{self as core, AsyncRoute, Edge};
use proptest::prelude::*;

/// The C's `long` as `i64` (the same type on 64-bit Unix).
#[allow(
    clippy::useless_conversion,
    reason = "c_long is i64 on 64-bit Unix and i32 on Windows"
)]
fn widen(n: c_long) -> i64 {
    i64::from(n)
}

#[track_caller]
fn c_bool(byte: u8, what: &str) -> bool {
    assert!(byte <= 1, "{what}: the C returned the byte {byte}");
    byte == 1
}

fn edge_value(e: Edge) -> c_uint {
    match e {
        Edge::None => 0,
        Edge::On => 1,
        Edge::Off => 2,
    }
}

fn route_value(r: AsyncRoute) -> c_uint {
    match r {
        AsyncRoute::Closest => 0,
        AsyncRoute::Deinterlace => 1,
    }
}

#[test]
fn parity_values() {
    let want: [i64; 11] = [
        i64::from(core::TYPE_TRANSITION),
        i64::from(core::ASYNC),
        i64::from(core::CONTROLLABLE_MEDIA),
        i64::from(shim::OBS_SOURCE_VIDEO_TICK_EDGE_NONE),
        i64::from(shim::OBS_SOURCE_VIDEO_TICK_EDGE_ON),
        i64::from(shim::OBS_SOURCE_VIDEO_TICK_EDGE_OFF),
        i64::from(shim::OBS_SOURCE_VIDEO_TICK_ASYNC_CLOSEST),
        i64::from(shim::OBS_SOURCE_VIDEO_TICK_ASYNC_DEINTERLACE),
        i64::try_from(size_of::<shim::obs_source_video_tick_edge>()).unwrap(),
        i64::try_from(size_of::<shim::obs_source_video_tick_async_route>()).unwrap(),
        i64::try_from(size_of::<c_long>()).unwrap(),
    ];
    for (i, (name, want)) in oracle::VALUE_NAMES.iter().zip(want).enumerate() {
        // SAFETY: by-value index in, value out; the C touches no memory.
        let c = unsafe { oracle::oracle_source_video_tick_value(i) };
        assert_eq!(c, want, "{name}");
    }
    // SAFETY: as above.
    assert_eq!(
        unsafe { oracle::oracle_source_video_tick_value(oracle::VALUE_NAMES.len()) },
        i64::MIN
    );
    assert_eq!(edge_value(Edge::On), shim::OBS_SOURCE_VIDEO_TICK_EDGE_ON);
    assert_eq!(
        route_value(AsyncRoute::Deinterlace),
        shim::OBS_SOURCE_VIDEO_TICK_ASYNC_DEINTERLACE
    );
}

#[track_caller]
fn check_type(t: c_int) {
    // SAFETY: by-value integers in and out; the C touches no memory.
    let c = c_bool(
        unsafe { oracle::oracle_obs_source_video_tick_transition(t) },
        "transition",
    );
    assert_eq!(
        shim::obs_source_video_tick_transition(t),
        c,
        "shim type {t}"
    );
    assert_eq!(core::transition_tick(t), c, "core type {t}");
}

#[track_caller]
fn check_flags(f: u32) {
    // SAFETY: as above.
    let a = c_bool(
        unsafe { oracle::oracle_obs_source_video_tick_async(f) },
        "async",
    );
    // SAFETY: as above.
    let m = c_bool(
        unsafe { oracle::oracle_obs_source_video_tick_media(f) },
        "media",
    );
    assert_eq!(shim::obs_source_video_tick_async(f), a, "shim async {f:#x}");
    assert_eq!(core::async_tick(f), a, "core async {f:#x}");
    assert_eq!(shim::obs_source_video_tick_media(f), m, "shim media {f:#x}");
    assert_eq!(core::media_actions(f), m, "core media {f:#x}");
}

#[track_caller]
fn check_count(n: c_long) {
    // SAFETY: as above.
    let d = c_bool(
        unsafe { oracle::oracle_obs_source_video_tick_deferred_update(n) },
        "deferred_update",
    );
    assert_eq!(
        shim::obs_source_video_tick_deferred_update(n),
        d,
        "shim count {n}"
    );
    assert_eq!(core::deferred_update(widen(n)), d, "core count {n}");
    for was in [false, true] {
        // SAFETY: as above.
        let e = unsafe { oracle::oracle_obs_source_video_tick_edge(n, was) };
        assert!(e <= 2, "edge({n}, {was}): the C returned {e}");
        assert_eq!(
            shim::obs_source_video_tick_edge(n, was),
            e,
            "shim edge({n}, {was})"
        );
        assert_eq!(
            edge_value(core::edge(widen(n), was)),
            e,
            "core edge({n}, {was})"
        );
    }
}

#[test]
fn parity_transition() {
    for t in -100_000..=100_000 {
        check_type(t);
    }
    let mut t = i64::from(c_int::MIN);
    while t <= i64::from(c_int::MAX) {
        check_type(c_int::try_from(t).unwrap());
        t += 65_521;
    }
    for t in [c_int::MIN, c_int::MIN + 1, c_int::MAX - 1, c_int::MAX] {
        check_type(t);
    }
}

#[test]
fn parity_flags() {
    for low in 0..=u32::from(u16::MAX) {
        for high in 0..16u32 {
            // spread the four high bits over the high word
            let h = (high & 1) << 16 | (high & 2) << 20 | (high & 4) << 25 | (high & 8) << 28;
            check_flags(low | h);
        }
    }
    let mut f = 0u64;
    while f <= u64::from(u32::MAX) {
        check_flags(u32::try_from(f).unwrap());
        f += 65_521;
    }
    check_flags(u32::MAX);
}

/// 38 special counts, as in Tier 1, that fit the C's `long`.
fn special_counts() -> Vec<c_long> {
    let mut v: Vec<i64> = vec![0, 1, -1, 2, -2, 3, 100, -100];
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
    assert_eq!(v.len(), 38);
    v.into_iter()
        .filter_map(|x| c_long::try_from(x).ok())
        .collect()
}

#[test]
fn parity_counts() {
    for n in -100_000..=100_000 {
        check_count(n);
    }
    for n in special_counts() {
        check_count(n);
    }
}

#[test]
fn parity_bools() {
    for data in [false, true] {
        for cb in [false, true] {
            // SAFETY: by-value bools in, the byte out.
            let c = c_bool(
                unsafe { oracle::oracle_obs_source_video_tick_calls_tick(data, cb) },
                "calls_tick",
            );
            assert_eq!(shim::obs_source_video_tick_calls_tick(data, cb), c);
            assert_eq!(core::calls_tick(data, cb), c);
        }
        // SAFETY: as above.
        let r = unsafe { oracle::oracle_obs_source_video_tick_async_route(data) };
        assert!(r <= 1, "async_route({data}): the C returned {r}");
        assert_eq!(shim::obs_source_video_tick_async_route(data), r);
        assert_eq!(route_value(core::async_route(data)), r);
    }
}

/// The seven functions, from one side.
struct Side {
    transition: fn(c_int) -> bool,
    async_tick: fn(u32) -> bool,
    media: fn(u32) -> bool,
    deferred: fn(c_long) -> bool,
    edge: fn(c_long, bool) -> c_uint,
    calls_tick: fn(bool, bool) -> bool,
    route: fn(bool) -> c_uint,
}

fn o_transition(t: c_int) -> bool {
    // SAFETY: by-value values in and out; the C touches no memory.
    c_bool(
        unsafe { oracle::oracle_obs_source_video_tick_transition(t) },
        "transition",
    )
}

fn o_async(f: u32) -> bool {
    // SAFETY: as above.
    c_bool(
        unsafe { oracle::oracle_obs_source_video_tick_async(f) },
        "async",
    )
}

fn o_media(f: u32) -> bool {
    // SAFETY: as above.
    c_bool(
        unsafe { oracle::oracle_obs_source_video_tick_media(f) },
        "media",
    )
}

fn o_deferred(n: c_long) -> bool {
    // SAFETY: as above.
    c_bool(
        unsafe { oracle::oracle_obs_source_video_tick_deferred_update(n) },
        "deferred_update",
    )
}

fn o_edge(n: c_long, was: bool) -> c_uint {
    // SAFETY: as above.
    unsafe { oracle::oracle_obs_source_video_tick_edge(n, was) }
}

fn o_calls_tick(d: bool, c: bool) -> bool {
    // SAFETY: as above.
    c_bool(
        unsafe { oracle::oracle_obs_source_video_tick_calls_tick(d, c) },
        "calls_tick",
    )
}

fn o_route(d: bool) -> c_uint {
    // SAFETY: as above.
    unsafe { oracle::oracle_obs_source_video_tick_async_route(d) }
}

const ORACLE: Side = Side {
    transition: o_transition,
    async_tick: o_async,
    media: o_media,
    deferred: o_deferred,
    edge: o_edge,
    calls_tick: o_calls_tick,
    route: o_route,
};

/// The shims through Rust-ABI pointers.
const SHIM: Side = Side {
    transition: |t| shim::obs_source_video_tick_transition(t),
    async_tick: |f| shim::obs_source_video_tick_async(f),
    media: |f| shim::obs_source_video_tick_media(f),
    deferred: |n| shim::obs_source_video_tick_deferred_update(n),
    edge: |n, was| shim::obs_source_video_tick_edge(n, was),
    calls_tick: |d, c| shim::obs_source_video_tick_calls_tick(d, c),
    route: |d| shim::obs_source_video_tick_async_route(d),
};

/// A stand-in source and the inputs of one tick.
#[derive(Clone, Debug)]
struct Tick {
    ty: c_int,
    flags: u32,
    defer: c_long,
    show_refs: c_long,
    activate_refs: c_long,
    filters: usize,
    has_data: bool,
    has_callback: bool,
    deinterlacing: bool,
    show_activates: bool,
}

/// `obs_source_video_tick()`'s and `async_tick()`'s order of steps over one
/// side's functions; the trace, then the stored flags.
fn run(side: &Side, ticks: &[Tick], showing: &mut bool, active: &mut bool) -> String {
    let mut t = String::new();
    for k in ticks {
        if (side.transition)(k.ty) {
            t.push_str("transition ");
        }
        if (side.async_tick)(k.flags) {
            t.push_str("lock ");
            t.push_str(if (side.route)(k.deinterlacing) == 1 {
                "deinterlace "
            } else {
                "closest "
            });
            if (side.route)(k.deinterlacing) == 1 {
                t.push_str("filter(prev) ");
            }
            t.push_str("filter(cur) unlock ");
        }
        if (side.media)(k.flags) {
            t.push_str("media ");
        }
        if (side.deferred)(k.defer) {
            t.push_str("deferred ");
        }
        let mut activate_refs = k.activate_refs;
        let e = (side.edge)(k.show_refs, *showing);
        if e != 0 {
            let now = e == 1;
            t.push_str(if now { "show " } else { "hide " });
            if now && k.show_activates {
                activate_refs = activate_refs.wrapping_add(1);
            }
            for i in (0..k.filters).rev() {
                t.push_str(&format!("{}(f{i}) ", if now { "show" } else { "hide" }));
            }
            *showing = now;
        }
        let e = (side.edge)(activate_refs, *active);
        if e != 0 {
            let now = e == 1;
            t.push_str(if now { "activate " } else { "deactivate " });
            for i in (0..k.filters).rev() {
                t.push_str(&format!(
                    "{}(f{i}) ",
                    if now { "activate" } else { "deactivate" }
                ));
            }
            *active = now;
        }
        if (side.calls_tick)(k.has_data, k.has_callback) {
            t.push_str("video_tick ");
        }
        t.push_str("| ");
    }
    t
}

fn count() -> impl Strategy<Value = c_long> {
    prop_oneof![
        Just(0 as c_long),
        Just(1 as c_long),
        Just(-1 as c_long),
        Just(2 as c_long),
        Just(c_long::MIN),
        Just(c_long::MAX),
        any::<c_long>(),
    ]
}

fn tick() -> impl Strategy<Value = Tick> {
    (
        prop_oneof![Just(0), Just(1), Just(2), Just(3), any::<c_int>()],
        prop_oneof![
            Just(0u32),
            Just(core::ASYNC),
            Just(core::CONTROLLABLE_MEDIA),
            Just(core::ASYNC | core::CONTROLLABLE_MEDIA),
            any::<u32>(),
        ],
        count(),
        count(),
        count(),
        0usize..4,
        any::<(bool, bool, bool, bool)>(),
    )
        .prop_map(
            |(
                ty,
                flags,
                defer,
                show_refs,
                activate_refs,
                filters,
                (has_data, has_callback, deinterlacing, show_activates),
            )| Tick {
                ty,
                flags,
                defer,
                show_refs,
                activate_refs,
                filters,
                has_data,
                has_callback,
                deinterlacing,
                show_activates,
            },
        )
}

/// Every source type with every combination of the two flags, the four
/// count pairs around 0, both starting flags and both show callbacks.
#[test]
fn parity_tick_every_state() {
    let types = [0, 1, 2, 3, -1, 4, c_int::MIN, c_int::MAX];
    let flags = [
        0,
        core::ASYNC,
        core::CONTROLLABLE_MEDIA,
        core::ASYNC | core::CONTROLLABLE_MEDIA,
    ];
    let counts: [c_long; 4] = [0, 1, -1, c_long::MAX];
    let mut runs = 0u32;
    for ty in types {
        for f in flags {
            for defer in counts {
                for show_refs in counts {
                    for activate_refs in counts {
                        for bits in 0..64u32 {
                            let k = Tick {
                                ty,
                                flags: f,
                                defer,
                                show_refs,
                                activate_refs,
                                filters: usize::try_from(bits % 3).unwrap(),
                                has_data: bits & 1 != 0,
                                has_callback: bits & 2 != 0,
                                deinterlacing: bits & 4 != 0,
                                show_activates: bits & 8 != 0,
                            };
                            let (s0, a0) = (bits & 16 != 0, bits & 32 != 0);
                            let (mut so, mut ao, mut ss, mut as_) = (s0, a0, s0, a0);
                            let c = run(&ORACLE, std::slice::from_ref(&k), &mut so, &mut ao);
                            let r = run(&SHIM, std::slice::from_ref(&k), &mut ss, &mut as_);
                            assert_eq!(r, c, "{k:?} showing {s0} active {a0}");
                            assert_eq!((ss, as_), (so, ao), "{k:?}");
                            runs += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(runs, 8 * 4 * 4 * 4 * 4 * 64);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn parity_proptest_values(t in any::<c_int>(), f in any::<u32>(), n in any::<c_long>(), b in any::<(bool, bool)>()) {
        check_type(t);
        check_flags(f);
        check_count(n);
        // SAFETY: by-value bools in, the byte out.
        let c = c_bool(unsafe { oracle::oracle_obs_source_video_tick_calls_tick(b.0, b.1) }, "calls_tick");
        prop_assert_eq!(shim::obs_source_video_tick_calls_tick(b.0, b.1), c);
        // SAFETY: as above.
        let r = unsafe { oracle::oracle_obs_source_video_tick_async_route(b.0) };
        prop_assert_eq!(shim::obs_source_video_tick_async_route(b.0), r);
    }

    #[test]
    fn parity_proptest_tick_runs(ticks in prop::collection::vec(tick(), 1..32), start in any::<(bool, bool)>()) {
        let (mut so, mut ao, mut ss, mut as_) = (start.0, start.1, start.0, start.1);
        let c = run(&ORACLE, &ticks, &mut so, &mut ao);
        let r = run(&SHIM, &ticks, &mut ss, &mut as_);
        prop_assert_eq!(r, c);
        prop_assert_eq!((ss, as_), (so, ao));
    }
}
