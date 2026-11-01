//! Tier 3: `obs_core::core_setup_rules` against the original
//! `libobs/obs-core-setup-rules.c.h` and `libobs/obs-core-setup-rules-ms.c.h`,
//! compiled as an oracle.
//!
//! Every shim is compared with its `oracle_*` twin: every step the C enums
//! name, and steps they do not (the C's `switch` falls through), with both
//! values; every gate order of `obs_reset_video()` and every mix of
//! `obs_get_video_frame_counts()`; every search of `obs_find_data_file()`
//! over up to eight added paths; the buffering ticks at the edges of
//! `u32` and around whole ticks; the command line sizes, the compares, the
//! latest version and the capture device flag at the edges of their types;
//! the constants and enum values of the header; the buffering milliseconds
//! over every tick from -2097 to 4195 at the usual rates and the edges of
//! the C's `(int)` cast; the task queue's route, every step over the type
//! values -2 to 5 and the edges of `i32`, with both values, and every
//! walk of `obs_queue_task()`; the task thread; the video level checks
//! over every special `f32` (both zeros, the denormal and normal edges,
//! both infinities, quiet and signalling NaNs of both signs); and
//! proptests over any input.
//!
//! Intentional differences, in `buffer_ms` (#3272), where the C is
//! undefined: a rate that casts to an `int` of 0 (the C's divide traps)
//! gives 0, and a product past `i32` (the C's `int` overflows, from 2098
//! ticks) is computed in `i64` and its quotient clamped to `i32`. Exactly
//! those inputs skip the oracle and are checked against that contract
//! instead (`c_defined`, `fixed_ms`); every other input is compared.
//!
//! And two in the task route and the video levels (#3397):
//! - `task_step`'s type step with a type outside the four: the C goes on
//!   to `IN_THREAD`, so `obs_queue_task()` asks `obs_in_task_thread()`,
//!   which asserts, and with `wait` waits forever for a task it never
//!   queues; the core answers `UNKNOWN`, and `obs_queue_task()` logs it and
//!   returns. Exactly that step skips the oracle and is checked against
//!   that contract instead (`known_type`); every later step, and the
//!   thread rule, are compared for every type;
//! - `levels_core(false)`: the C goes on with no core and reads
//!   `obs->video` through NULL; the core stops. `levels_core(true)` is
//!   compared.

use core::ffi::c_int;

use obs_c_oracle::core_setup_rules::{
    CONSTANT_NAMES, constant, oracle_obs_core_setup_rules_aoc_check,
    oracle_obs_core_setup_rules_audio_refused, oracle_obs_core_setup_rules_buffer_default_ticks,
    oracle_obs_core_setup_rules_buffer_ms, oracle_obs_core_setup_rules_buffer_set,
    oracle_obs_core_setup_rules_buffer_ticks, oracle_obs_core_setup_rules_cmdline_len,
    oracle_obs_core_setup_rules_cmdline_set, oracle_obs_core_setup_rules_cmdline_table,
    oracle_obs_core_setup_rules_data_file_found, oracle_obs_core_setup_rules_data_path_match,
    oracle_obs_core_setup_rules_latest_newer, oracle_obs_core_setup_rules_level_ok,
    oracle_obs_core_setup_rules_levels_core, oracle_obs_core_setup_rules_mix_step,
    oracle_obs_core_setup_rules_monitor_missing, oracle_obs_core_setup_rules_monitor_same,
    oracle_obs_core_setup_rules_task_step, oracle_obs_core_setup_rules_task_thread,
    oracle_obs_core_setup_rules_video_gate,
};
use obs_core::core_setup_rules::{
    AUDIO_OUTPUT_FRAMES, DEFAULT_TICKS, DO_NOT_SELF_MONITOR, LATEST_NONE, SEC_TO_MSEC, TASK_AUDIO,
    TASK_DESTROY, TASK_GRAPHICS, TASK_UI,
};
use obs_core::ffi::core_setup_rules::{
    VIDEO_CURRENTLY_ACTIVE, VIDEO_FAIL, VIDEO_GO, VIDEO_INVALID_PARAM,
    obs_core_setup_rules_aoc_check, obs_core_setup_rules_audio_refused,
    obs_core_setup_rules_buffer_default_ticks, obs_core_setup_rules_buffer_ms,
    obs_core_setup_rules_buffer_set, obs_core_setup_rules_buffer_ticks,
    obs_core_setup_rules_cmdline_len, obs_core_setup_rules_cmdline_set,
    obs_core_setup_rules_cmdline_table, obs_core_setup_rules_data_file_found,
    obs_core_setup_rules_data_path_match, obs_core_setup_rules_latest_newer,
    obs_core_setup_rules_level_ok, obs_core_setup_rules_levels_core, obs_core_setup_rules_mix_step,
    obs_core_setup_rules_monitor_missing, obs_core_setup_rules_monitor_same,
    obs_core_setup_rules_task_step, obs_core_setup_rules_task_thread,
    obs_core_setup_rules_video_gate,
};
use proptest::prelude::*;

/// Steps past the ones each C enum names, and the edges of `u32`.
const EXTRA_STEPS: [u32; 4] = [6, 7, 0x7fff_ffff, u32::MAX];

fn constant_of(name: &str) -> i64 {
    let i = CONSTANT_NAMES
        .iter()
        .position(|&n| n == name)
        .unwrap_or_else(|| panic!("no constant {name}"));
    constant(i)
}

fn check_video_gate(step: u32, value: bool) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_core_setup_rules_video_gate(step, value) };
    assert_eq!(
        obs_core_setup_rules_video_gate(step, value),
        c,
        "{step} {value}"
    );
}

fn check_audio_refused(step: u32, value: bool) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_core_setup_rules_audio_refused(step, value) };
    assert_eq!(
        obs_core_setup_rules_audio_refused(step, value),
        c,
        "{step} {value}"
    );
}

fn check_mix_step(step: u32, value: bool) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_core_setup_rules_mix_step(step, value) };
    assert_eq!(
        obs_core_setup_rules_mix_step(step, value),
        c,
        "{step} {value}"
    );
}

fn check_ticks(ms: u32, rate: u32) {
    // SAFETY: plain values.
    let (set, ticks) = unsafe {
        (
            oracle_obs_core_setup_rules_buffer_set(ms),
            oracle_obs_core_setup_rules_buffer_ticks(ms, rate),
        )
    };
    assert_eq!(obs_core_setup_rules_buffer_set(ms), set, "{ms}");
    assert_eq!(
        obs_core_setup_rules_buffer_ticks(ms, rate),
        ticks,
        "{ms} {rate}"
    );
}

/// The C's `ticks * 1024 * 1000 / (int)rate` is defined: the rate does not
/// cast to 0 and the product fits an `int` (`INT_MIN / -1` needs a product
/// of `INT_MIN`, which is not a multiple of 1024000).
fn c_defined(ticks: c_int, rate: u32) -> bool {
    rate.cast_signed() != 0 && i32::try_from(i64::from(ticks) * 1_024_000).is_ok()
}

/// The fixed contract where the C is undefined: 0 for a zero rate, else the
/// `i64` quotient clamped to `i32`.
fn fixed_ms(ticks: c_int, rate: u32) -> c_int {
    let rate = i64::from(rate.cast_signed());
    if rate == 0 {
        return 0;
    }
    let ms = i64::from(ticks) * 1_024_000 / rate;
    i32::try_from(ms).unwrap_or(if ms < 0 { i32::MIN } else { i32::MAX })
}

fn check_ms(ticks: c_int, rate: u32) {
    let rust = obs_core_setup_rules_buffer_ms(ticks, rate);
    if c_defined(ticks, rate) {
        // SAFETY: plain values, and c_defined() keeps the C defined.
        let c = unsafe { oracle_obs_core_setup_rules_buffer_ms(ticks, rate) };
        assert_eq!(rust, c, "{ticks} {rate}");
    } else {
        assert_eq!(rust, fixed_ms(ticks, rate), "{ticks} {rate}");
    }
}

/// The usual audio rates, and the edges of the C's `(int)` cast.
const MS_RATES: [u32; 14] = [
    0,
    1,
    7,
    8000,
    22050,
    44100,
    48000,
    96000,
    192_000,
    0x7fff_ffff,
    0x8000_0000,
    0x8000_0001,
    0xffff_fc00,
    u32::MAX,
];

fn check_cmdline(argc: c_int, ptr: usize, len: usize, arg_len: usize) {
    // SAFETY: plain values.
    let (set, sum) = unsafe {
        (
            oracle_obs_core_setup_rules_cmdline_set(argc),
            oracle_obs_core_setup_rules_cmdline_len(len, arg_len),
        )
    };
    assert_eq!(obs_core_setup_rules_cmdline_set(argc), set, "{argc}");
    assert_eq!(
        obs_core_setup_rules_cmdline_len(len, arg_len),
        sum,
        "{len} {arg_len}"
    );
    // SAFETY: plain values.
    let table = unsafe { oracle_obs_core_setup_rules_cmdline_table(argc, ptr) };
    assert_eq!(
        obs_core_setup_rules_cmdline_table(argc, ptr),
        table,
        "{argc} {ptr}"
    );
}

fn check_compares(cmp: c_int, version: u32, flags: u32, present: bool) {
    // SAFETY: plain values.
    let (path, same, latest, aoc, missing) = unsafe {
        (
            oracle_obs_core_setup_rules_data_path_match(cmp),
            oracle_obs_core_setup_rules_monitor_same(cmp),
            oracle_obs_core_setup_rules_latest_newer(version, cmp),
            oracle_obs_core_setup_rules_aoc_check(flags),
            oracle_obs_core_setup_rules_monitor_missing(present),
        )
    };
    assert_eq!(obs_core_setup_rules_data_path_match(cmp), path, "{cmp}");
    assert_eq!(obs_core_setup_rules_monitor_same(cmp), same, "{cmp}");
    assert_eq!(
        obs_core_setup_rules_latest_newer(version, cmp),
        latest,
        "{version} {cmp}"
    );
    assert_eq!(obs_core_setup_rules_aoc_check(flags), aoc, "{flags:#x}");
    assert_eq!(
        obs_core_setup_rules_monitor_missing(present),
        missing,
        "{present}"
    );
}

#[test]
fn constants_match_the_header() {
    assert_eq!(constant(CONSTANT_NAMES.len()), i64::MIN);
    assert_eq!(
        constant_of("OBS_CORE_SETUP_RULES_VIDEO_GO"),
        i64::from(VIDEO_GO)
    );
    assert_eq!(
        constant_of("OBS_CORE_SETUP_RULES_VIDEO_FAIL"),
        i64::from(VIDEO_FAIL)
    );
    assert_eq!(
        constant_of("OBS_CORE_SETUP_RULES_VIDEO_INVALID_PARAM"),
        i64::from(VIDEO_INVALID_PARAM)
    );
    assert_eq!(
        constant_of("OBS_CORE_SETUP_RULES_VIDEO_CURRENTLY_ACTIVE"),
        i64::from(VIDEO_CURRENTLY_ACTIVE)
    );
    assert_eq!(constant_of("OBS_VIDEO_SUCCESS"), i64::from(VIDEO_GO));
    assert_eq!(constant_of("OBS_VIDEO_FAIL"), i64::from(VIDEO_FAIL));
    assert_eq!(
        constant_of("OBS_VIDEO_INVALID_PARAM"),
        i64::from(VIDEO_INVALID_PARAM)
    );
    assert_eq!(
        constant_of("OBS_VIDEO_CURRENTLY_ACTIVE"),
        i64::from(VIDEO_CURRENTLY_ACTIVE)
    );
    for name in [
        "OBS_CORE_SETUP_RULES_DO_NOT_SELF_MONITOR",
        "OBS_SOURCE_DO_NOT_SELF_MONITOR",
    ] {
        assert_eq!(constant_of(name), i64::from(DO_NOT_SELF_MONITOR), "{name}");
    }
    for name in [
        "OBS_CORE_SETUP_RULES_AUDIO_OUTPUT_FRAMES",
        "AUDIO_OUTPUT_FRAMES",
    ] {
        assert_eq!(constant_of(name), i64::from(AUDIO_OUTPUT_FRAMES), "{name}");
    }
    assert_eq!(
        constant_of("OBS_CORE_SETUP_RULES_DEFAULT_TICKS"),
        i64::from(DEFAULT_TICKS)
    );
    assert_eq!(
        constant_of("OBS_CORE_SETUP_RULES_SEC_TO_MSEC"),
        i64::from(SEC_TO_MSEC)
    );
    assert_eq!(
        constant_of("OBS_CORE_SETUP_RULES_LATEST_NONE"),
        i64::from(LATEST_NONE)
    );
    // SAFETY: no arguments.
    let ticks = unsafe { oracle_obs_core_setup_rules_buffer_default_ticks() };
    assert_eq!(obs_core_setup_rules_buffer_default_ticks(), ticks);

    // The enums count from 0 in the order the shims take them, and are 32
    // bits wide.
    let enums: [&[&str]; 3] = [
        &[
            "OBS_CORE_SETUP_RULES_VIDEO_CORE",
            "OBS_CORE_SETUP_RULES_VIDEO_ACTIVE",
            "OBS_CORE_SETUP_RULES_VIDEO_STARTING",
            "OBS_CORE_SETUP_RULES_VIDEO_OUTPUT_SIZE",
            "OBS_CORE_SETUP_RULES_VIDEO_BASE_SIZE",
        ],
        &[
            "OBS_CORE_SETUP_RULES_AUDIO_CORE",
            "OBS_CORE_SETUP_RULES_AUDIO_ACTIVE",
        ],
        &[
            "OBS_CORE_SETUP_RULES_MIX_MAIN",
            "OBS_CORE_SETUP_RULES_MIX_ENCODER_ONLY",
            "OBS_CORE_SETUP_RULES_MIX_VIEW",
            "OBS_CORE_SETUP_RULES_MIX_VIDEO",
            "OBS_CORE_SETUP_RULES_MIX_COUNT",
            "OBS_CORE_SETUP_RULES_MIX_SKIP",
        ],
    ];
    for names in enums {
        for (i, name) in names.iter().enumerate() {
            assert_eq!(constant_of(name), i64::try_from(i).unwrap(), "{name}");
        }
    }
    for name in [
        "sizeof(enum obs_core_setup_rules_video_step)",
        "sizeof(enum obs_core_setup_rules_audio_step)",
        "sizeof(enum obs_core_setup_rules_mix)",
    ] {
        assert_eq!(constant_of(name), 4, "{name}");
    }
    assert_eq!(
        constant_of("sizeof(void *)"),
        i64::try_from(size_of::<*const u8>()).unwrap()
    );
}

#[test]
fn task_constants_match_the_header() {
    // The task types the header copies from obs.h.
    for (t, value) in ["UI", "GRAPHICS", "AUDIO", "DESTROY"].into_iter().zip([
        TASK_UI,
        TASK_GRAPHICS,
        TASK_AUDIO,
        TASK_DESTROY,
    ]) {
        let copy = format!("OBS_CORE_SETUP_RULES_TASK_{t}");
        assert_eq!(constant_of(&copy), i64::from(value), "{copy}");
        let original = format!("OBS_TASK_{t}");
        assert_eq!(constant_of(&original), i64::from(value), "{original}");
    }
    for (i, name) in TASK_NAMES.iter().enumerate() {
        let name = format!("OBS_CORE_SETUP_RULES_TASK_{name}");
        assert_eq!(constant_of(&name), i64::try_from(i).unwrap(), "{name}");
    }
    assert_eq!(constant_of("sizeof(enum obs_core_setup_rules_task)"), 4);
}

/// `enum obs_core_setup_rules_task`, in order from 0.
const TASK_NAMES: [&str; 13] = [
    "TYPE",
    "UI_HANDLER",
    "IN_THREAD",
    "WAIT",
    "UI_CALL",
    "UI_NONE",
    "RUN",
    "WAIT_QUEUE",
    "PUSH_GRAPHICS",
    "PUSH_AUDIO",
    "PUSH_DESTROY",
    "DROP",
    "UNKNOWN",
];
const TASK_TYPE: u32 = 0;
const TASK_UNKNOWN: u32 = 12;

/// Task types: the four, the ones around them and the edges of `i32`.
const TASK_TYPES: [c_int; 10] = [-2, -1, 0, 1, 2, 3, 4, 5, c_int::MIN, c_int::MAX];

fn known_type(task_type: c_int) -> bool {
    (TASK_UI..=TASK_DESTROY).contains(&task_type)
}

fn check_task_step(step: u32, task_type: c_int, value: bool) {
    let rust = obs_core_setup_rules_task_step(step, task_type, value);
    if step == TASK_TYPE && !known_type(task_type) {
        // #3397: the C sends it on to obs_in_task_thread().
        assert_eq!(rust, TASK_UNKNOWN, "{task_type} {value}");
        return;
    }
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_core_setup_rules_task_step(step, task_type, value) };
    assert_eq!(rust, c, "{step} {task_type} {value}");
}

fn check_task_thread(task_type: c_int) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_core_setup_rules_task_thread(task_type) };
    assert_eq!(
        obs_core_setup_rules_task_thread(task_type),
        c,
        "{task_type}"
    );
}

fn check_level_ok(level: f32) {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_core_setup_rules_level_ok(level) };
    assert_eq!(
        obs_core_setup_rules_level_ok(level),
        c,
        "{level:?} {:#010x}",
        level.to_bits()
    );
}

/// `obs_queue_task()`'s walk, through each side: the route and the steps
/// it asked.
fn task_walk(
    task_step: impl Fn(u32, c_int, bool) -> u32,
    task_type: c_int,
    values: [bool; 3],
) -> (u32, Vec<u32>) {
    let mut asked = Vec::new();
    let mut step = task_step(TASK_TYPE, task_type, false);
    for (at, value) in (1..=3).zip(values) {
        if step == at {
            asked.push(step);
            step = task_step(step, task_type, value);
        }
    }
    (step, asked)
}

#[test]
fn every_task_step_matches() {
    for value in [false, true] {
        for step in (0..=14).chain(EXTRA_STEPS) {
            for task_type in TASK_TYPES {
                check_task_step(step, task_type, value);
            }
        }
    }
    for task_type in TASK_TYPES {
        check_task_thread(task_type);
    }
}

#[test]
fn every_task_walk_matches() {
    for m in 0u32..8 {
        let values = [m & 1 != 0, m & 2 != 0, m & 4 != 0];
        for task_type in TASK_TYPES {
            let rust = task_walk(
                |s, t, v| obs_core_setup_rules_task_step(s, t, v),
                task_type,
                values,
            );
            if known_type(task_type) {
                // SAFETY: plain values.
                let c = task_walk(
                    |s, t, v| unsafe { oracle_obs_core_setup_rules_task_step(s, t, v) },
                    task_type,
                    values,
                );
                assert_eq!(rust, c, "{task_type} {m}");
            } else {
                // #3397: refused before any read.
                assert_eq!(rust, (TASK_UNKNOWN, vec![]), "{task_type} {m}");
            }
        }
    }
}

#[test]
fn levels_core_matches() {
    // SAFETY: plain values.
    let c = unsafe { oracle_obs_core_setup_rules_levels_core(true) };
    assert_eq!(obs_core_setup_rules_levels_core(true), c);
    // #3397: the C goes on with no core.
    // SAFETY: plain values.
    assert!(unsafe { oracle_obs_core_setup_rules_levels_core(false) });
    assert!(!obs_core_setup_rules_levels_core(false));
}

#[test]
fn level_ok_special_floats_match() {
    let bits: [u32; 14] = [
        0x0000_0000, // 0
        0x0000_0001, // the smallest denormal
        0x007f_ffff, // the largest denormal
        0x0080_0000, // FLT_MIN
        0x3f80_0000, // 1
        0x4396_0000, // 300
        0x447a_0000, // 1000
        0x7f7f_ffff, // FLT_MAX
        0x7f80_0000, // infinity
        0x7f80_0001, // the smallest signalling NaN
        0x7fbf_ffff, // the largest signalling NaN
        0x7fc0_0000, // the quiet NaN
        0x7fc0_0001, // a quiet NaN with a payload
        0x7fff_ffff, // the largest NaN
    ];
    for b in bits {
        check_level_ok(f32::from_bits(b));
        check_level_ok(f32::from_bits(b | 0x8000_0000));
    }
}

#[test]
fn every_step_matches() {
    for value in [false, true] {
        for step in (0..=8).chain(EXTRA_STEPS) {
            check_video_gate(step, value);
            check_audio_refused(step, value);
            check_mix_step(step, value);
        }
    }
}

/// `obs_reset_video()`'s gates in order, through each side.
fn gates(video_gate: impl Fn(u32, bool) -> c_int, values: [bool; 5]) -> c_int {
    (0u32..)
        .zip(values)
        .map(|(step, value)| video_gate(step, value))
        .find(|&code| code != VIDEO_GO)
        .unwrap_or(VIDEO_GO)
}

/// `obs_get_video_frame_counts()`'s rule, through each side: the end state
/// and the steps read.
fn mix(mix_step: impl Fn(u32, bool) -> u32, values: [bool; 4]) -> (u32, Vec<u32>) {
    let mut step = mix_step(0, values[0]);
    let mut read = vec![0];
    for (ask, value) in (1u32..=3).zip(&values[1..]) {
        if step == ask {
            read.push(ask);
            step = mix_step(step, *value);
        }
    }
    (step, read)
}

#[test]
fn every_gate_order_and_mix_matches() {
    // SAFETY (both closures): plain values.
    let c_gate = |step, value| unsafe { oracle_obs_core_setup_rules_video_gate(step, value) };
    let c_mix = |step, value| unsafe { oracle_obs_core_setup_rules_mix_step(step, value) };
    for m in 0u32..32 {
        let values = [0, 1, 2, 3, 4].map(|n| m & (1 << n) != 0);
        assert_eq!(
            gates(|s, v| obs_core_setup_rules_video_gate(s, v), values),
            gates(c_gate, values),
            "{m}"
        );
        if m < 16 {
            let values = [0, 1, 2, 3].map(|n| m & (1 << n) != 0);
            assert_eq!(
                mix(|s, v| obs_core_setup_rules_mix_step(s, v), values),
                mix(c_mix, values),
                "{m}"
            );
        }
    }
}

/// `obs_find_data_file()`'s search through each side: where the walk over
/// the libobs directory and the added paths stopped, and how many places it
/// looked in.
fn search(found: impl Fn(bool) -> bool, has_file: &[bool]) -> (Option<usize>, usize) {
    for (i, &has) in has_file.iter().enumerate() {
        if found(has) {
            return (Some(i), i + 1);
        }
    }
    (None, has_file.len())
}

#[test]
fn every_data_file_search_matches() {
    for value in [false, true] {
        // SAFETY: plain value.
        let c = unsafe { oracle_obs_core_setup_rules_data_file_found(value) };
        assert_eq!(obs_core_setup_rules_data_file_found(value), c, "{value}");
    }
    // The libobs directory and up to eight added paths, each with the file
    // or without it.
    for places in 1usize..=9 {
        for m in 0u32..(1 << places) {
            let has: Vec<bool> = (0..places).map(|i| m & (1 << i) != 0).collect();
            // SAFETY (the closure): plain value.
            let c = search(
                |f| unsafe { oracle_obs_core_setup_rules_data_file_found(f) },
                &has,
            );
            assert_eq!(
                search(|f| obs_core_setup_rules_data_file_found(f), &has),
                c,
                "{has:?}"
            );
        }
    }
}

/// Every tick from -2097 to 4195 (the most `buffer_ticks()` gives) at each
/// rate: compared with the C up to 2097, against the fixed contract past
/// it and for a zero rate.
#[test]
fn buffer_ms_every_tick() {
    // The most ticks: u32::MAX frames' worth, which no product passes.
    assert_eq!(obs_core_setup_rules_buffer_ticks(u32::MAX, 1), 4195);
    let mut compared = 0u32;
    for rate in MS_RATES {
        for ticks in -2097..=4195 {
            compared += u32::from(c_defined(ticks, rate));
            check_ms(ticks, rate);
        }
    }
    // At each of the 13 non-zero rates, every tick from -2097 to 2097 is
    // compared with the C.
    assert_eq!(compared, 13 * (2 * 2097 + 1));
}

#[test]
fn buffer_ms_edges() {
    for ticks in [
        i32::MIN,
        i32::MIN + 1,
        -2099,
        -2098,
        2098,
        2099,
        i32::MAX - 1,
        i32::MAX,
    ] {
        for rate in MS_RATES {
            check_ms(ticks, rate);
        }
    }
    // The fixed contract itself, where the C is undefined.
    assert_eq!(obs_core_setup_rules_buffer_ms(45, 0), 0);
    assert_eq!(obs_core_setup_rules_buffer_ms(2098, 48000), 44757);
    assert_eq!(obs_core_setup_rules_buffer_ms(2098, 1), i32::MAX);
    assert_eq!(obs_core_setup_rules_buffer_ms(2098, u32::MAX), i32::MIN);
}

#[test]
fn buffer_ticks_edges_match() {
    let values = [
        0,
        1,
        2,
        999,
        1000,
        1001,
        1023,
        1024,
        1025,
        22369,
        22370,
        44100,
        48000,
        192_000,
        1_024_000,
        1_024_001,
        0x7fff_ffff,
        0x8000_0000,
        u32::MAX - 1,
        u32::MAX,
    ];
    for ms in values {
        for rate in values {
            check_ticks(ms, rate);
        }
    }
    // Around every whole tick at 1 kHz.
    for ticks in 0u32..64 {
        for ms in (ticks * 1024).saturating_sub(2)..=ticks * 1024 + 2 {
            check_ticks(ms, 1000);
        }
    }
}

#[test]
fn cmdline_edges_match() {
    let argcs = [
        c_int::MIN,
        c_int::MIN + 1,
        -2,
        -1,
        0,
        1,
        2,
        3,
        255,
        c_int::MAX - 1,
        c_int::MAX,
    ];
    let sizes = [
        0,
        1,
        2,
        3,
        4,
        8,
        16,
        usize::MAX / 2,
        usize::MAX - 1,
        usize::MAX,
    ];
    for argc in argcs {
        for ptr in sizes {
            for len in sizes {
                check_cmdline(
                    argc,
                    ptr,
                    len,
                    sizes[usize::try_from(argc.unsigned_abs()).unwrap() % sizes.len()],
                );
            }
        }
    }
}

#[test]
fn compare_edges_match() {
    let cmps = [
        c_int::MIN,
        c_int::MIN + 1,
        -2,
        -1,
        0,
        1,
        2,
        c_int::MAX - 1,
        c_int::MAX,
    ];
    let versions = [
        0,
        1,
        2,
        0x7fff_fffe,
        0x7fff_ffff,
        0x8000_0000,
        0x8000_0001,
        u32::MAX,
    ];
    for cmp in cmps {
        for version in versions {
            for present in [false, true] {
                check_compares(cmp, version, version, present);
            }
        }
    }
    for bit in 0..32 {
        check_compares(0, 0, 1 << bit, true);
        check_compares(0, 0, !(1 << bit), false);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(obs_c_oracle::parity::cases(20_000)))]

    #[test]
    fn steps_any(step in any::<u32>(), value in any::<bool>()) {
        check_video_gate(step, value);
        check_audio_refused(step, value);
        check_mix_step(step, value);
    }

    #[test]
    fn buffer_ticks_any(ms in any::<u32>(), rate in any::<u32>()) {
        check_ticks(ms, rate);
    }

    #[test]
    fn buffer_ticks_audio_rates(ms in 0u32..100_000, rate in prop::sample::select(vec![8000u32, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 192_000])) {
        check_ticks(ms, rate);
    }

    #[test]
    fn buffer_ms_defined(ticks in -2097i32..=2097, rate in 1u32..=u32::MAX) {
        prop_assert!(c_defined(ticks, rate));
        check_ms(ticks, rate);
    }

    #[test]
    fn buffer_ms_any(ticks in any::<c_int>(), rate in any::<u32>()) {
        check_ms(ticks, rate);
    }

    #[test]
    fn buffer_ms_from_buffering(ms in any::<u32>(), rate in prop::sample::select(MS_RATES.to_vec())) {
        // The ticks obs_reset_audio2() stores, then the milliseconds of them.
        let ticks = obs_core_setup_rules_buffer_ticks(ms, rate).cast_signed();
        check_ms(ticks, rate);
    }

    #[test]
    fn cmdline_any(argc in any::<c_int>(), ptr in any::<usize>(), len in any::<usize>(), arg_len in any::<usize>()) {
        check_cmdline(argc, ptr, len, arg_len);
    }

    #[test]
    fn cmdline_lists(lens in prop::collection::vec(0usize..4096, 0..16)) {
        let argc = c_int::try_from(lens.len()).unwrap();
        let ptr = size_of::<*const u8>();
        let mut rust = 0usize;
        let mut c = 0usize;
        for &arg in &lens {
            rust = obs_core_setup_rules_cmdline_len(rust, arg);
            // SAFETY: plain values.
            c = unsafe { oracle_obs_core_setup_rules_cmdline_len(c, arg) };
        }
        prop_assert_eq!(rust, c);
        check_cmdline(argc, ptr, rust, 0);
    }

    #[test]
    fn compares_any(cmp in any::<c_int>(), version in any::<u32>(), flags in any::<u32>(), present in any::<bool>()) {
        check_compares(cmp, version, flags, present);
    }

    #[test]
    fn latest_walk_any(versions in prop::collection::vec(any::<u32>(), 0..12)) {
        let walk = |newer: &dyn Fn(u32, c_int) -> bool| {
            let mut best = LATEST_NONE;
            let mut found = None;
            for (i, &v) in versions.iter().enumerate() {
                if newer(v, best) {
                    found = Some(i);
                    best = v.cast_signed();
                }
            }
            found
        };
        // SAFETY: plain values.
        let c = walk(&|v, best| unsafe { oracle_obs_core_setup_rules_latest_newer(v, best) });
        prop_assert_eq!(walk(&|v, best| obs_core_setup_rules_latest_newer(v, best)), c);
    }

    #[test]
    fn task_any(step in any::<u32>(), task_type in any::<c_int>(), value in any::<bool>()) {
        check_task_step(step, task_type, value);
        check_task_step(step % 16, task_type, value);
        check_task_thread(task_type);
    }

    #[test]
    fn level_ok_any(bits in any::<u32>()) {
        check_level_ok(f32::from_bits(bits));
    }
}
