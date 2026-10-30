//! Tier 3: `obs_core::core_setup_rules` against the original
//! `libobs/obs-core-setup-rules.c.h`, compiled as an oracle.
//!
//! Every shim is compared with its `oracle_*` twin: every step the C enums
//! name, and steps they do not (the C's `switch` falls through), with both
//! values; every gate order of `obs_reset_video()` and every mix of
//! `obs_get_video_frame_counts()`; every search of `obs_find_data_file()`
//! over up to eight added paths; the buffering ticks at the edges of
//! `u32` and around whole ticks; the command line sizes, the compares, the
//! latest version and the capture device flag at the edges of their types;
//! the constants and enum values of the header; and proptests over any
//! input.
//!
//! No intentional difference: nothing is excluded.

use core::ffi::c_int;

use obs_c_oracle::core_setup_rules::{
    CONSTANT_NAMES, constant, oracle_obs_core_setup_rules_aoc_check,
    oracle_obs_core_setup_rules_audio_refused, oracle_obs_core_setup_rules_buffer_default_ticks,
    oracle_obs_core_setup_rules_buffer_set, oracle_obs_core_setup_rules_buffer_ticks,
    oracle_obs_core_setup_rules_cmdline_len, oracle_obs_core_setup_rules_cmdline_set,
    oracle_obs_core_setup_rules_cmdline_table, oracle_obs_core_setup_rules_data_file_found,
    oracle_obs_core_setup_rules_data_path_match, oracle_obs_core_setup_rules_latest_newer,
    oracle_obs_core_setup_rules_mix_step, oracle_obs_core_setup_rules_monitor_missing,
    oracle_obs_core_setup_rules_monitor_same, oracle_obs_core_setup_rules_video_gate,
};
use obs_core::core_setup_rules::{
    AUDIO_OUTPUT_FRAMES, DEFAULT_TICKS, DO_NOT_SELF_MONITOR, LATEST_NONE,
};
use obs_core::ffi::core_setup_rules::{
    VIDEO_CURRENTLY_ACTIVE, VIDEO_FAIL, VIDEO_GO, VIDEO_INVALID_PARAM,
    obs_core_setup_rules_aoc_check, obs_core_setup_rules_audio_refused,
    obs_core_setup_rules_buffer_default_ticks, obs_core_setup_rules_buffer_set,
    obs_core_setup_rules_buffer_ticks, obs_core_setup_rules_cmdline_len,
    obs_core_setup_rules_cmdline_set, obs_core_setup_rules_cmdline_table,
    obs_core_setup_rules_data_file_found, obs_core_setup_rules_data_path_match,
    obs_core_setup_rules_latest_newer, obs_core_setup_rules_mix_step,
    obs_core_setup_rules_monitor_missing, obs_core_setup_rules_monitor_same,
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
}
