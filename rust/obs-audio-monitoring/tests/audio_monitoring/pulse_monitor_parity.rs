//! Tier 3 parity: the `pulse_monitor` core and its shims against the
//! original `pulseaudio-monitor-steps.c.h` (oracle/pulse_monitor_steps.c).
//!
//! The unchanged C test `test_pulse_monitor_steps` compiles the C into
//! itself; `test_pulse_monitor_steps_rust` runs it against the shims, and
//! this file compares all three sides (oracle, shims, core) exactly: the
//! constants, every decision over its edge values and exhaustive or strided
//! ranges, the callers (the write loop, the open routes and the reset routes
//! of `pulseaudio-output.c`) on the oracle and on the shims, and proptests.
//!
//! Intentional difference, excluded here: a null id given to
//! `pulseaudio_monitor_is_default()`, or to `pulseaudio_monitor_fallback()`
//! with an invalid format, is not `"default"` and does not fall back in
//! Rust; the C dereferences it. `pulseaudio-output.c` asks both only after
//! `pulseaudio_monitor_has_device()`. Nothing else is excluded.

// `long` and `uint_fast32_t` are as wide as `i64` and `u64` on some hosts
// only, so their widening is a no-op here and needed elsewhere.
#![allow(clippy::useless_conversion)]

use std::ffi::{CString, c_char, c_int, c_long, c_uint};
use std::ptr;

use obs_audio_monitoring::ffi::pulse_format::uint_fast32_t;
use obs_audio_monitoring::ffi::pulse_monitor as shim;
use obs_audio_monitoring::pulse_format::SAMPLE_INVALID;
use obs_audio_monitoring::pulse_monitor::{
    self as core, CONTEXT_AUTHORIZING, CONTEXT_CONNECTING, CONTEXT_FAILED, CONTEXT_READY,
    CONTEXT_SETTING_NAME, CONTEXT_TERMINATED, CONTEXT_UNCONNECTED, DO_NOT_SELF_MONITOR, SinkResult,
};
use obs_c_oracle::pulse_monitor_steps::{
    VALUE_NAMES, oracle_pulse_monitor_playback_bytes, oracle_pulse_monitor_value,
    oracle_pulseaudio_monitor_context_good, oracle_pulseaudio_monitor_event_queued,
    oracle_pulseaudio_monitor_fallback, oracle_pulseaudio_monitor_format_invalid,
    oracle_pulseaudio_monitor_grow, oracle_pulseaudio_monitor_grown_tlength,
    oracle_pulseaudio_monitor_has_device, oracle_pulseaudio_monitor_is_default,
    oracle_pulseaudio_monitor_last, oracle_pulseaudio_monitor_needs_volume,
    oracle_pulseaudio_monitor_open_flags, oracle_pulseaudio_monitor_playback_idle,
    oracle_pulseaudio_monitor_self_monitor_check, oracle_pulseaudio_monitor_sink_named,
    oracle_pulseaudio_monitor_sink_result, oracle_pulseaudio_monitor_tlength_usec,
    oracle_pulseaudio_monitor_uncork, oracle_pulseaudio_monitor_write_more,
    oracle_pulseaudio_monitor_write_short,
};
use proptest::prelude::*;

const U32_MAX: usize = 0xffff_ffff;

/// One side of the comparison: the oracle or the shims, through the C ABI.
#[derive(Clone, Copy)]
struct Side {
    grow: unsafe extern "C" fn(usize, u32) -> bool,
    grown_tlength: unsafe extern "C" fn(usize) -> u32,
    uncork: unsafe extern "C" fn(usize, u32) -> bool,
    write_more: unsafe extern "C" fn(usize) -> bool,
    write_short: unsafe extern "C" fn(usize, usize) -> bool,
    is_default: unsafe extern "C" fn(*const c_char) -> bool,
    fallback: unsafe extern "C" fn(c_int, *const c_char) -> bool,
    has_device: unsafe extern "C" fn(*const c_char) -> bool,
    self_monitor_check: unsafe extern "C" fn(u32) -> bool,
    sink_named: unsafe extern "C" fn(*const c_char) -> bool,
    format_invalid: unsafe extern "C" fn(c_int) -> bool,
    open_flags: unsafe extern "C" fn(bool, *const c_char) -> c_uint,
}

const ORACLE: Side = Side {
    grow: oracle_pulseaudio_monitor_grow,
    grown_tlength: oracle_pulseaudio_monitor_grown_tlength,
    uncork: oracle_pulseaudio_monitor_uncork,
    write_more: oracle_pulseaudio_monitor_write_more,
    write_short: oracle_pulseaudio_monitor_write_short,
    is_default: oracle_pulseaudio_monitor_is_default,
    fallback: oracle_pulseaudio_monitor_fallback,
    has_device: oracle_pulseaudio_monitor_has_device,
    self_monitor_check: oracle_pulseaudio_monitor_self_monitor_check,
    sink_named: oracle_pulseaudio_monitor_sink_named,
    format_invalid: oracle_pulseaudio_monitor_format_invalid,
    open_flags: oracle_pulseaudio_monitor_open_flags,
};

const SHIMS: Side = Side {
    grow: shim::pulseaudio_monitor_grow,
    grown_tlength: shim::pulseaudio_monitor_grown_tlength,
    uncork: shim::pulseaudio_monitor_uncork,
    write_more: shim::pulseaudio_monitor_write_more,
    write_short: shim::pulseaudio_monitor_write_short,
    is_default: shim::pulseaudio_monitor_is_default,
    fallback: shim::pulseaudio_monitor_fallback,
    has_device: shim::pulseaudio_monitor_has_device,
    self_monitor_check: shim::pulseaudio_monitor_self_monitor_check,
    sink_named: shim::pulseaudio_monitor_sink_named,
    format_invalid: shim::pulseaudio_monitor_format_invalid,
    open_flags: shim::pulseaudio_monitor_open_flags,
};

fn edge_sizes() -> Vec<usize> {
    let mut v = vec![
        0,
        1,
        2,
        4799,
        4800,
        4801,
        9599,
        9600,
        9601,
        19_199,
        19_200,
        19_201,
        0x7fff_ffff,
        0x8000_0000,
        0xffff_fffe,
        U32_MAX,
        usize::MAX / 2,
        usize::MAX - 1,
        usize::MAX,
    ];
    if usize::MAX > U32_MAX {
        v.extend([
            U32_MAX + 1,
            U32_MAX + 2,
            U32_MAX + 6,
            2 * U32_MAX,
            2 * U32_MAX + 1,
            2 * U32_MAX + 2,
        ]);
    }
    v
}

const EDGE_U32: [u32; 14] = [
    0,
    1,
    2,
    4799,
    4800,
    4801,
    9600,
    0x7fff_ffff,
    0x8000_0000,
    0x8000_0001,
    0xc000_0000,
    0xffff_fffe,
    0xffff_ffff,
    0x1234_5678,
];

fn sizes_around_tlengths() -> Vec<usize> {
    let mut v = edge_sizes();
    for t in EDGE_U32 {
        let t = t as usize;
        for d in [t.wrapping_sub(1), t, t + 1] {
            v.push(d);
        }
        let twice = t.wrapping_mul(2);
        for d in [twice.wrapping_sub(1), twice, twice.wrapping_add(1)] {
            v.push(d);
        }
        let wrapped = (t as u32).wrapping_mul(2) as usize;
        for d in [wrapped.wrapping_sub(1), wrapped, wrapped.wrapping_add(1)] {
            v.push(d);
        }
    }
    v
}

fn ids() -> Vec<CString> {
    [
        &b"default"[..],
        b"",
        b"Default",
        b"defaults",
        b"defaul",
        b"default ",
        b" default",
        b"default.monitor",
        b"sink.a",
        b"alsa_output.usb-headphones.analog-stereo",
        b"\xff\xfe",
    ]
    .iter()
    .map(|s| CString::new(*s).expect("no NUL"))
    .collect()
}

#[test]
fn constants_match_the_oracle() {
    let rust: [i64; 14] = [
        i64::from(DO_NOT_SELF_MONITOR),
        SinkResult::Info as i64,
        SinkResult::End as i64,
        SinkResult::Error as i64,
        i64::from(CONTEXT_UNCONNECTED),
        i64::from(CONTEXT_CONNECTING),
        i64::from(CONTEXT_AUTHORIZING),
        i64::from(CONTEXT_SETTING_NAME),
        i64::from(CONTEXT_READY),
        i64::from(CONTEXT_FAILED),
        i64::from(CONTEXT_TERMINATED),
        i64::from(SAMPLE_INVALID),
        size_of::<uint_fast32_t>() as i64,
        size_of::<c_long>() as i64,
    ];
    for (i, (name, value)) in VALUE_NAMES.iter().zip(rust).enumerate() {
        // SAFETY: `i` is below the table length; out of range returns 0.
        let c = unsafe { oracle_pulse_monitor_value(i) };
        assert_eq!(value, c, "{name}: Rust {value}, C {c}");
    }
    // SAFETY: a plain oracle getter.
    let tl = unsafe { oracle_pulseaudio_monitor_tlength_usec() };
    assert_eq!(tl, shim::pulseaudio_monitor_tlength_usec());
    assert_eq!(tl, core::tlength_usec());
    assert_eq!(tl, core::TLENGTH_USEC);
}

#[test]
fn write_rules_over_every_pair() {
    let sizes = sizes_around_tlengths();
    let mut calls = 0u64;
    for &q in &sizes {
        // SAFETY: plain values.
        unsafe {
            let c = oracle_pulseaudio_monitor_grown_tlength(q);
            assert_eq!(
                shim::pulseaudio_monitor_grown_tlength(q),
                c,
                "grown_tlength({q})"
            );
            assert_eq!(core::grown_tlength(q), c, "grown_tlength({q})");
            let c = oracle_pulseaudio_monitor_write_more(q);
            assert_eq!(shim::pulseaudio_monitor_write_more(q), c, "write_more({q})");
            assert_eq!(core::write_more(q), c);
            let c = oracle_pulseaudio_monitor_last(q);
            assert_eq!(shim::pulseaudio_monitor_last(q), c, "last({q})");
            assert_eq!(core::last(q), c);
            for t in EDGE_U32 {
                let c = oracle_pulseaudio_monitor_grow(q, t);
                assert_eq!(shim::pulseaudio_monitor_grow(q, t), c, "grow({q}, {t})");
                assert_eq!(core::grow(q, t), c, "grow({q}, {t})");
                let c = oracle_pulseaudio_monitor_uncork(q, t);
                assert_eq!(shim::pulseaudio_monitor_uncork(q, t), c, "uncork({q}, {t})");
                assert_eq!(core::uncork(q, t), c, "uncork({q}, {t})");
                calls += 2;
            }
            for &a in &sizes {
                let c = oracle_pulseaudio_monitor_write_short(a, q);
                assert_eq!(
                    shim::pulseaudio_monitor_write_short(a, q),
                    c,
                    "write_short({a}, {q})"
                );
                assert_eq!(core::write_short(a, q), c, "write_short({a}, {q})");
                calls += 1;
            }
        }
    }
    assert!(calls > 10_000, "{calls}");
}

#[test]
fn playback_steps() {
    let refs: [c_long; 8] = [c_long::MIN, c_long::MIN + 1, -2, -1, 0, 1, 2, c_long::MAX];
    for r in refs {
        // SAFETY: a plain value.
        let c = unsafe { oracle_pulseaudio_monitor_playback_idle(r) };
        assert_eq!(
            shim::pulseaudio_monitor_playback_idle(r),
            c,
            "playback_idle({r})"
        );
        assert_eq!(core::playback_idle(i64::from(r)), c);
    }
    let wide = size_of::<uint_fast32_t>() == 8;
    let bpf = [
        0u64,
        1,
        2,
        3,
        4,
        8,
        16,
        24,
        32,
        255,
        0x7fff_ffff,
        0xffff_ffff,
        0x1_0000_0000,
        u64::MAX,
    ];
    for b in bpf {
        let bf = b as uint_fast32_t;
        for f in EDGE_U32 {
            // SAFETY: plain values.
            let c = unsafe { oracle_pulse_monitor_playback_bytes(b, f) };
            assert_eq!(
                shim::pulseaudio_monitor_playback_bytes(bf, f),
                c,
                "playback_bytes({b}, {f})"
            );
            assert_eq!(
                core::playback_bytes(u64::from(bf), f, wide),
                c,
                "playback_bytes({b}, {f})"
            );
        }
    }
}

#[test]
fn volume_near_one_and_a_stride_through_every_float() {
    let check = |bits: u32| {
        let v = f32::from_bits(bits);
        // SAFETY: a plain value.
        let c = unsafe { oracle_pulseaudio_monitor_needs_volume(v) };
        assert_eq!(
            shim::pulseaudio_monitor_needs_volume(v),
            c,
            "needs_volume({v:e}, {bits:#x})"
        );
        assert_eq!(core::needs_volume(v), c, "needs_volume({v:e}, {bits:#x})");
    };
    // Every float within 2^16 ulps of 1 and of the two EPSILON boundaries.
    for centre in [0x3f80_0000u32, 0x3f80_0346, 0x3f7f_f973] {
        for bits in centre - 65_536..=centre + 65_536 {
            check(bits);
        }
    }
    // Every 4099th bit pattern (NaNs, infinities, subnormals, negatives).
    let mut bits = 0u32;
    loop {
        check(bits);
        match bits.checked_add(4099) {
            Some(b) => bits = b,
            None => break,
        }
    }
    for bits in [
        0x7f80_0000u32,
        0xff80_0000,
        0x7fc0_0000,
        0xffc0_0000,
        0x7f80_0001,
        0x8000_0000,
        0x0000_0001,
    ] {
        check(bits);
    }
}

#[test]
fn sink_results_over_eol() {
    let check = |eol: i32| {
        // SAFETY: a plain value.
        let c = unsafe { oracle_pulseaudio_monitor_sink_result(eol) };
        assert_eq!(
            shim::pulseaudio_monitor_sink_result(eol),
            c,
            "sink_result({eol})"
        );
        assert_eq!(core::sink_result(eol) as c_uint, c, "sink_result({eol})");
    };
    for eol in -1000..=1000 {
        check(eol);
    }
    let mut e = i32::MIN;
    loop {
        check(e);
        match e.checked_add(65_537) {
            Some(n) => e = n,
            None => break,
        }
    }
    check(i32::MAX);
}

#[test]
fn flags_events_and_states() {
    let mut words: Vec<u32> = (0..=0xffffu32).collect();
    for hi in [0x0001_0000u32, 0x8000_0000, 0xffff_0000] {
        words.extend((0..=0xffu32).map(|w| w | hi));
    }
    let mut w = 0u32;
    loop {
        words.push(w);
        match w.checked_add(65_521) {
            Some(n) => w = n,
            None => break,
        }
    }
    words.push(u32::MAX);
    for &w in &words {
        // SAFETY: plain values.
        unsafe {
            let c = oracle_pulseaudio_monitor_self_monitor_check(w);
            assert_eq!(
                shim::pulseaudio_monitor_self_monitor_check(w),
                c,
                "self_monitor_check({w:#x})"
            );
            assert_eq!(core::self_monitor_check(w), c);
            let c = oracle_pulseaudio_monitor_event_queued(w);
            assert_eq!(
                shim::pulseaudio_monitor_event_queued(w),
                c,
                "event_queued({w:#x})"
            );
            assert_eq!(core::event_queued(w), c);
        }
    }
    let states = (0..=4096u32).chain([u32::MAX, u32::MAX - 1, 0x8000_0000, 0x7fff_ffff]);
    for s in states {
        // SAFETY: a plain value.
        let c = unsafe { oracle_pulseaudio_monitor_context_good(s) };
        assert_eq!(
            shim::pulseaudio_monitor_context_good(s),
            c,
            "context_good({s})"
        );
        assert_eq!(core::context_good(s), c, "context_good({s})");
    }
}

#[test]
fn formats_and_ids() {
    let ids = ids();
    let formats = (-1000..=1000).chain([i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX]);
    for f in formats {
        // SAFETY: plain values; the ids are NUL-terminated.
        unsafe {
            let c = oracle_pulseaudio_monitor_format_invalid(f);
            assert_eq!(
                shim::pulseaudio_monitor_format_invalid(f),
                c,
                "format_invalid({f})"
            );
            assert_eq!(core::format_invalid(f), c);
            for id in &ids {
                let c = oracle_pulseaudio_monitor_fallback(f, id.as_ptr());
                assert_eq!(
                    shim::pulseaudio_monitor_fallback(f, id.as_ptr()),
                    c,
                    "fallback({f}, {id:?})"
                );
                assert_eq!(core::fallback(f, id.as_bytes()), c, "fallback({f}, {id:?})");
            }
            // The fallback with a valid format never reads the id: NULL is
            // fine on every side.
            if f != SAMPLE_INVALID {
                assert!(!oracle_pulseaudio_monitor_fallback(f, ptr::null()));
                assert!(!shim::pulseaudio_monitor_fallback(f, ptr::null()));
            }
        }
    }
    for id in &ids {
        let p = id.as_ptr();
        let b = Some(id.as_bytes());
        // SAFETY: the ids are NUL-terminated.
        unsafe {
            let c = oracle_pulseaudio_monitor_is_default(p);
            assert_eq!(
                shim::pulseaudio_monitor_is_default(p),
                c,
                "is_default({id:?})"
            );
            assert_eq!(core::is_default(id.as_bytes()), c);
            let c = oracle_pulseaudio_monitor_has_device(p);
            assert_eq!(
                shim::pulseaudio_monitor_has_device(p),
                c,
                "has_device({id:?})"
            );
            assert_eq!(core::has_device(b), c);
            let c = oracle_pulseaudio_monitor_sink_named(p);
            assert_eq!(
                shim::pulseaudio_monitor_sink_named(p),
                c,
                "sink_named({id:?})"
            );
            assert_eq!(core::sink_named(b), c);
            for fb in [false, true] {
                let c = oracle_pulseaudio_monitor_open_flags(fb, p);
                assert_eq!(
                    shim::pulseaudio_monitor_open_flags(fb, p),
                    c,
                    "open_flags({fb}, {id:?})"
                );
                assert_eq!(core::open_flags(fb, b), c, "open_flags({fb}, {id:?})");
            }
        }
    }
    // NULL where the C reads only the pointer.
    // SAFETY: NULL is allowed for these.
    unsafe {
        let c = oracle_pulseaudio_monitor_has_device(ptr::null());
        assert_eq!(shim::pulseaudio_monitor_has_device(ptr::null()), c);
        assert_eq!(core::has_device(None), c);
        let c = oracle_pulseaudio_monitor_sink_named(ptr::null());
        assert_eq!(shim::pulseaudio_monitor_sink_named(ptr::null()), c);
        assert_eq!(core::sink_named(None), c);
        for fb in [false, true] {
            let c = oracle_pulseaudio_monitor_open_flags(fb, ptr::null());
            assert_eq!(
                shim::pulseaudio_monitor_open_flags(fb, ptr::null()),
                c,
                "open_flags({fb}, NULL)"
            );
            assert_eq!(core::open_flags(fb, None), c);
        }
    }
}

// ---- the callers, on the oracle and on the shims ----

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Write {
    queued: usize,
    tlength: u32,
    corked: bool,
    grew: u32,
    uncorked: bool,
    written: usize,
    writes: u32,
    cancelled: bool,
}

/// `do_stream_write()` as the C test's stand-in, through one side.
fn run_write(side: Side, mut w: Write, asks: &[usize]) -> Write {
    // SAFETY: plain values.
    unsafe {
        if (side.grow)(w.queued, w.tlength) {
            w.tlength = (side.grown_tlength)(w.queued);
            w.grew += 1;
        }
        if w.corked {
            if !(side.uncork)(w.queued, w.tlength) {
                return w;
            }
            w.corked = false;
            w.uncorked = true;
        }
        let mut k = 0;
        while (side.write_more)(w.queued) {
            let Some(&ask) = asks.get(k) else {
                return w;
            };
            k += 1;
            if (side.write_short)(ask, w.queued) {
                w.cancelled = true;
                return w;
            }
            w.queued -= ask;
            w.written += ask;
            w.writes += 1;
        }
    }
    w
}

#[test]
fn write_loop_on_both_sides() {
    let tls = [0u32, 1, 4800, 9600, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff];
    let mut runs = 0u64;
    for &tl in &tls {
        let t = tl as usize;
        let queues = [
            0,
            1,
            t.saturating_sub(1),
            t,
            t + 1,
            2 * t,
            2 * t + 1,
            3 * t,
            5000,
            usize::MAX / 2,
        ];
        for &q in &queues {
            for corked in [false, true] {
                let ask_sets: [&[usize]; 7] = [
                    &[],
                    &[q],
                    &[q.saturating_add(1)],
                    &[q / 2, q - q / 2],
                    &[q / 2, q - q / 2 + 1],
                    &[1, 1, 1],
                    &[0, q],
                ];
                for asks in ask_sets {
                    let w = Write {
                        queued: q,
                        tlength: tl,
                        corked,
                        ..Write::default()
                    };
                    let c = run_write(ORACLE, w.clone(), asks);
                    let r = run_write(SHIMS, w, asks);
                    assert_eq!(
                        r, c,
                        "queue {q}, tlength {tl}, corked {corked}, asks {asks:?}"
                    );
                    runs += 1;
                }
            }
        }
    }
    assert_eq!(runs, 7 * 10 * 2 * 7);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Fail,
    Ignore,
    Default,
    Named,
    Fallback,
}

/// `audio_monitor_init()`'s decisions, as the C test's stand-in, through
/// one side.
fn run_open(
    side: Side,
    id: *const c_char,
    flags: u32,
    matched: bool,
    fmt: c_int,
    default_id: *const c_char,
    fallback_fmt: c_int,
) -> (Route, c_uint) {
    // SAFETY: the ids are NULL or NUL-terminated; is_default() and
    // fallback() are asked only after has_device().
    unsafe {
        if !(side.has_device)(id) {
            return (Route::Fail, 0);
        }
        if (side.self_monitor_check)(flags) && matched {
            return (Route::Ignore, 0);
        }
        let default = (side.is_default)(id);
        let mut fell_back = false;
        let mut fmt = fmt;
        if (side.fallback)(fmt, id) {
            fell_back = true;
            if !(side.sink_named)(default_id) {
                return (Route::Fail, 0);
            }
            fmt = fallback_fmt;
        }
        if (side.format_invalid)(fmt) {
            return (Route::Fail, 0);
        }
        let route = if fell_back {
            Route::Fallback
        } else if default {
            Route::Default
        } else {
            Route::Named
        };
        (route, (side.open_flags)(fell_back, id))
    }
}

#[test]
fn open_routes_on_both_sides() {
    let ids = ids();
    let mut id_ptrs: Vec<*const c_char> = ids.iter().map(|s| s.as_ptr()).collect();
    id_ptrs.push(ptr::null());
    let flags = [
        0u32,
        DO_NOT_SELF_MONITOR,
        !DO_NOT_SELF_MONITOR,
        u32::MAX,
        1 << 8,
        1 << 10,
    ];
    let fmts = [SAMPLE_INVALID, 0, 3, 5, 7, 12, 13, -2, 100];
    let mut routes = [0u32; 5];
    for &id in &id_ptrs {
        for &f in &flags {
            for matched in [false, true] {
                for &fmt in &fmts {
                    for &def in &id_ptrs {
                        for &ffmt in &fmts {
                            let c = run_open(ORACLE, id, f, matched, fmt, def, ffmt);
                            let r = run_open(SHIMS, id, f, matched, fmt, def, ffmt);
                            assert_eq!(
                                r, c,
                                "id {id:?}, flags {f:#x}, match {matched}, fmt {fmt}, default {def:?}, fallback fmt {ffmt}"
                            );
                            routes[c.0 as usize] += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(
        routes.iter().all(|&n| n > 0),
        "every route reached: {routes:?}"
    );
}

#[test]
fn reset_routes_on_both_sides() {
    for s in (0..=64u32).chain([u32::MAX, 0x8000_0000]) {
        for has_context in [false, true] {
            // `bool context_good = context && pulseaudio_monitor_context_good(state)`
            // SAFETY: a plain value.
            let c = has_context && unsafe { oracle_pulseaudio_monitor_context_good(s) };
            let r = has_context && shim::pulseaudio_monitor_context_good(s);
            assert_eq!(r, c, "state {s}, context {has_context}");
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20_000))]

    #[test]
    fn prop_decisions_on_all_three_sides(
        q in any::<usize>(),
        a in any::<usize>(),
        t in any::<u32>(),
        bpf in any::<u64>(),
        frames in any::<u32>(),
        bits in any::<u32>(),
        eol in any::<i32>(),
        refs in any::<i64>(),
        word in any::<u32>(),
        fmt in prop_oneof![Just(SAMPLE_INVALID), any::<i32>()],
        id in proptest::collection::vec(1u8..=255, 0..12),
        fb in any::<bool>(),
    ) {
        let id = CString::new(id).expect("no NUL");
        let p = id.as_ptr();
        let r = refs as c_long;
        let v = f32::from_bits(bits);
        let bf = bpf as uint_fast32_t;
        let wide = size_of::<uint_fast32_t>() == 8;
        // SAFETY: plain values; the id is NUL-terminated.
        unsafe {
            prop_assert_eq!(shim::pulseaudio_monitor_grow(q, t), oracle_pulseaudio_monitor_grow(q, t));
            prop_assert_eq!(core::grow(q, t), oracle_pulseaudio_monitor_grow(q, t));
            prop_assert_eq!(shim::pulseaudio_monitor_grown_tlength(q), oracle_pulseaudio_monitor_grown_tlength(q));
            prop_assert_eq!(core::grown_tlength(q), oracle_pulseaudio_monitor_grown_tlength(q));
            prop_assert_eq!(shim::pulseaudio_monitor_uncork(q, t), oracle_pulseaudio_monitor_uncork(q, t));
            prop_assert_eq!(core::uncork(q, t), oracle_pulseaudio_monitor_uncork(q, t));
            prop_assert_eq!(shim::pulseaudio_monitor_write_more(q), oracle_pulseaudio_monitor_write_more(q));
            prop_assert_eq!(shim::pulseaudio_monitor_write_short(a, q), oracle_pulseaudio_monitor_write_short(a, q));
            prop_assert_eq!(core::write_short(a, q), oracle_pulseaudio_monitor_write_short(a, q));
            prop_assert_eq!(shim::pulseaudio_monitor_playback_idle(r), oracle_pulseaudio_monitor_playback_idle(r));
            prop_assert_eq!(core::playback_idle(i64::from(r)), oracle_pulseaudio_monitor_playback_idle(r));
            let c = oracle_pulse_monitor_playback_bytes(bpf, frames);
            prop_assert_eq!(shim::pulseaudio_monitor_playback_bytes(bf, frames), c);
            prop_assert_eq!(core::playback_bytes(u64::from(bf), frames, wide), c);
            prop_assert_eq!(shim::pulseaudio_monitor_needs_volume(v), oracle_pulseaudio_monitor_needs_volume(v));
            prop_assert_eq!(core::needs_volume(v), oracle_pulseaudio_monitor_needs_volume(v));
            prop_assert_eq!(shim::pulseaudio_monitor_sink_result(eol), oracle_pulseaudio_monitor_sink_result(eol));
            prop_assert_eq!(core::sink_result(eol) as c_uint, oracle_pulseaudio_monitor_sink_result(eol));
            prop_assert_eq!(shim::pulseaudio_monitor_self_monitor_check(word), oracle_pulseaudio_monitor_self_monitor_check(word));
            prop_assert_eq!(shim::pulseaudio_monitor_event_queued(word), oracle_pulseaudio_monitor_event_queued(word));
            prop_assert_eq!(core::event_queued(word), oracle_pulseaudio_monitor_event_queued(word));
            prop_assert_eq!(shim::pulseaudio_monitor_context_good(word), oracle_pulseaudio_monitor_context_good(word));
            prop_assert_eq!(shim::pulseaudio_monitor_format_invalid(fmt), oracle_pulseaudio_monitor_format_invalid(fmt));
            prop_assert_eq!(shim::pulseaudio_monitor_fallback(fmt, p), oracle_pulseaudio_monitor_fallback(fmt, p));
            prop_assert_eq!(core::fallback(fmt, id.as_bytes()), oracle_pulseaudio_monitor_fallback(fmt, p));
            prop_assert_eq!(shim::pulseaudio_monitor_is_default(p), oracle_pulseaudio_monitor_is_default(p));
            prop_assert_eq!(shim::pulseaudio_monitor_sink_named(p), oracle_pulseaudio_monitor_sink_named(p));
            prop_assert_eq!(shim::pulseaudio_monitor_open_flags(fb, p), oracle_pulseaudio_monitor_open_flags(fb, p));
            prop_assert_eq!(core::open_flags(fb, Some(id.as_bytes())), oracle_pulseaudio_monitor_open_flags(fb, p));
            prop_assert_eq!(shim::pulseaudio_monitor_last(q), oracle_pulseaudio_monitor_last(q));
        }
    }

    #[test]
    fn prop_write_loop_on_both_sides(
        queued in prop_oneof![0usize..40_000, any::<usize>()],
        tlength in prop_oneof![0u32..20_000, any::<u32>()],
        corked in any::<bool>(),
        asks in proptest::collection::vec(prop_oneof![0usize..20_000, any::<usize>()], 0..6),
    ) {
        let w = Write { queued, tlength, corked, ..Write::default() };
        prop_assert_eq!(run_write(SHIMS, w.clone(), &asks), run_write(ORACLE, w, &asks));
    }
}
