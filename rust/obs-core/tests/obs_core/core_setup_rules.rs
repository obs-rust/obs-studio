//! Tier 1: the safe core of `libobs/obs-core-setup-rules.c.h`.
//!
//! The first eighteen tests mirror the cases of
//! `test/cmocka/test_core_setup_rules_steps.c` one to one, in the same
//! order, with the same values and expectations. The first eleven call the
//! rules directly, as the C test calls the moved functions. The last seven
//! make the decisions `obs_reset_video()`, `obs_reset_audio2()`,
//! `obs_set_cmdline_args()`, `obs_get_latest_input_type_id()`,
//! `obs_set_audio_monitoring_device()`, `obs_get_video_frame_counts()` and
//! `obs_find_data_file()` make for the C test's calls.
//!
//! Cases marked "characterized, not endorsed" are odd but real (#2878) and
//! kept as the C has them. The tests after those pin the edges of each
//! function, and proptests cover any input.

use obs_core::core_setup_rules::{
    AUDIO_OUTPUT_FRAMES, AudioStep, DEFAULT_TICKS, DO_NOT_SELF_MONITOR, LATEST_NONE, Mix,
    VideoGate, VideoStep, aoc_check, audio_refused, buffer_default_ticks, buffer_set, buffer_ticks,
    cmdline_len, cmdline_set, cmdline_table, data_file_found, data_path_match, latest_newer,
    mix_step, monitor_missing, monitor_same, video_gate,
};
use proptest::prelude::*;

/// The gates in `obs_reset_video()`'s order: the first that does not go on
/// decides.
fn video_gates(
    core: bool,
    active: bool,
    starting: bool,
    output_ok: bool,
    base_ok: bool,
) -> VideoGate {
    [
        (VideoStep::Core, core),
        (VideoStep::Active, active),
        (VideoStep::Starting, starting),
        (VideoStep::OutputSize, output_ok),
        (VideoStep::BaseSize, base_ok),
    ]
    .into_iter()
    .map(|(step, value)| video_gate(step, value))
    .find(|&gate| gate != VideoGate::Go)
    .unwrap_or(VideoGate::Go)
}

/// The latest of a list of versions, as `obs_get_latest_input_type_id()`
/// walks it: the index of the winner.
fn latest_of(versions: &[u32]) -> Option<usize> {
    let mut best = LATEST_NONE;
    let mut found = None;
    for (i, &version) in versions.iter().enumerate() {
        if latest_newer(version, best) {
            found = Some(i);
            best = version.cast_signed();
        }
    }
    found
}

/// The frame-count rule as `obs_get_video_frame_counts()` asks it: whether
/// the mix counts, and how many reads it made.
fn counted(is_main: bool, encoder_only: bool, same_view: bool, has_video: bool) -> (bool, u32) {
    let mut step = mix_step(Mix::Main, is_main);
    let mut reads = 1;
    if step == Mix::EncoderOnly {
        step = mix_step(step, encoder_only);
        reads += 1;
    }
    if step == Mix::View {
        step = mix_step(step, same_view);
        reads += 1;
    }
    if step == Mix::Video {
        step = mix_step(step, has_video);
        reads += 1;
    }
    assert!(step == Mix::Count || step == Mix::Skip);
    (step == Mix::Count, reads)
}

/// The buffering `obs_get_audio_info2()` reports from the ticks: ticks *
/// 1024 * 1000 / rate, in `i32` as the C computes it.
fn buffering_ms(rate: u32, ms: u32) -> i32 {
    let ticks = if buffer_set(ms) {
        buffer_ticks(ms, rate)
    } else {
        buffer_default_ticks()
    };
    let ticks = i32::try_from(ticks).unwrap();
    ticks * 1024 * 1000 / i32::try_from(rate).unwrap()
}

// ---------------------------------------------------------------- steps

/// `test_video_gate`.
#[test]
fn test_video_gate() {
    assert_eq!(video_gate(VideoStep::Core, false), VideoGate::Fail);
    assert_eq!(video_gate(VideoStep::Core, true), VideoGate::Go);
    assert_eq!(
        video_gate(VideoStep::Active, true),
        VideoGate::CurrentlyActive
    );
    assert_eq!(video_gate(VideoStep::Active, false), VideoGate::Go);
    assert_eq!(
        video_gate(VideoStep::Starting, true),
        VideoGate::CurrentlyActive
    );
    assert_eq!(video_gate(VideoStep::Starting, false), VideoGate::Go);
    assert_eq!(
        video_gate(VideoStep::OutputSize, false),
        VideoGate::InvalidParam
    );
    assert_eq!(video_gate(VideoStep::OutputSize, true), VideoGate::Go);
    assert_eq!(
        video_gate(VideoStep::BaseSize, false),
        VideoGate::InvalidParam
    );
    assert_eq!(video_gate(VideoStep::BaseSize, true), VideoGate::Go);
}

/// `test_video_gate_order`.
#[test]
fn test_video_gate_order() {
    assert_eq!(
        video_gates(false, true, true, false, false),
        VideoGate::Fail
    );
    assert_eq!(
        video_gates(true, true, false, false, false),
        VideoGate::CurrentlyActive
    );
    assert_eq!(
        video_gates(true, false, true, false, false),
        VideoGate::CurrentlyActive
    );
    assert_eq!(
        video_gates(true, false, false, false, true),
        VideoGate::InvalidParam
    );
    assert_eq!(
        video_gates(true, false, false, true, false),
        VideoGate::InvalidParam
    );
    assert_eq!(video_gates(true, false, false, true, true), VideoGate::Go);
}

/// `test_audio_gate`.
#[test]
fn test_audio_gate() {
    assert!(audio_refused(AudioStep::Core, false));
    assert!(!audio_refused(AudioStep::Core, true));
    assert!(audio_refused(AudioStep::Active, true));
    assert!(!audio_refused(AudioStep::Active, false));
}

/// `test_buffer_ticks`.
#[test]
fn test_buffer_ticks() {
    assert!(!buffer_set(0));
    assert!(buffer_set(1));
    assert!(buffer_set(u32::MAX));
    assert_eq!(buffer_default_ticks(), 45);
    assert_eq!(AUDIO_OUTPUT_FRAMES, 1024);

    assert_eq!(buffer_ticks(1, 48000), 1);
    assert_eq!(buffer_ticks(1, 999), 0);
    assert_eq!(buffer_ticks(1, 1000), 1);
    assert_eq!(buffer_ticks(1000, 48000), 47);
    assert_eq!(buffer_ticks(1024, 1000), 1);
    assert_eq!(buffer_ticks(1024, 2000), 2);
    assert_eq!(buffer_ticks(1025, 1000), 2);
    assert_eq!(buffer_ticks(21333, 48000), 1000);
    assert_eq!(buffer_ticks(3, 341), 1);
    assert_eq!(buffer_ticks(0, 48000), 0);

    // Characterized, not endorsed: the product wraps in u32.
    assert_eq!(buffer_ticks(22369, 192_000), 4195);
    assert_eq!(buffer_ticks(22370, 192_000), 1);
    assert_eq!(buffer_ticks(u32::MAX, u32::MAX), 0);
}

/// `test_cmdline`.
#[test]
fn test_cmdline() {
    assert!(!cmdline_set(0));
    assert!(cmdline_set(1));
    assert!(cmdline_set(3));
    // Characterized, not endorsed: a negative argc counts as set.
    assert!(cmdline_set(-1));

    assert_eq!(cmdline_len(0, 0), 1);
    assert_eq!(cmdline_len(0, 3), 4);
    assert_eq!(cmdline_len(4, 9), 14);

    assert_eq!(cmdline_table(0, 8), 8);
    assert_eq!(cmdline_table(3, 8), 32);
    assert_eq!(cmdline_table(3, 4), 16);
    assert_eq!(
        cmdline_table(3, size_of::<*const u8>()),
        4 * size_of::<*const u8>()
    );
    // Characterized, not endorsed: an argc of -1 gives an empty table.
    assert_eq!(cmdline_table(-1, 8), 0);
}

/// `test_data_path_match`.
#[test]
fn test_data_path_match() {
    assert!(data_path_match(0));
    assert!(!data_path_match(1));
    assert!(!data_path_match(-1));
    assert!(!data_path_match(i32::MIN));
}

/// `test_latest`.
#[test]
fn test_latest() {
    assert_eq!(LATEST_NONE, -1);
    assert!(latest_newer(0, -1));
    assert!(!latest_newer(0, 0));
    assert!(latest_newer(2, 1));
    assert!(!latest_newer(1, 2));

    assert_eq!(latest_of(&[0, 1, 1]), Some(1));
    assert_eq!(latest_of(&[0, 2, 1]), Some(1));
    assert_eq!(latest_of(&[]), None);

    // Characterized, not endorsed: a version above i32::MAX is negative as
    // i32, so it is never chosen, even alone.
    assert!(!latest_newer(0x8000_0000, -1));
    assert!(!latest_newer(u32::MAX, -1));
    assert!(latest_newer(0x7fff_ffff, 0));
    assert_eq!(latest_of(&[0x8000_0000]), None);
    assert_eq!(latest_of(&[0x8000_0000, 0, u32::MAX]), Some(1));
}

/// `test_aoc_check`.
#[test]
fn test_aoc_check() {
    assert_eq!(DO_NOT_SELF_MONITOR, 1 << 9);
    for bit in 0..32 {
        assert_eq!(aoc_check(1 << bit), 1u32 << bit == DO_NOT_SELF_MONITOR);
    }
    assert!(!aoc_check(0));
    assert!(aoc_check(u32::MAX));
    assert!(!aoc_check(!DO_NOT_SELF_MONITOR));
}

/// `test_monitor`.
#[test]
fn test_monitor() {
    assert!(monitor_missing(false));
    assert!(!monitor_missing(true));
    assert!(monitor_same(0));
    assert!(!monitor_same(1));
    assert!(!monitor_same(-1));
}

/// `test_mix_step`.
#[test]
fn test_mix_step() {
    assert_eq!(counted(true, false, false, true), (true, 2));
    assert!(counted(true, true, false, true).0);
    assert_eq!(counted(true, false, false, false), (false, 2));

    assert_eq!(counted(false, true, true, true), (true, 4));
    assert_eq!(counted(false, true, false, true), (false, 3));
    assert_eq!(counted(false, false, true, true), (false, 2));
    assert_eq!(counted(false, true, true, false), (false, 4));

    assert_eq!(mix_step(Mix::Count, false), Mix::Count);
    assert_eq!(mix_step(Mix::Skip, true), Mix::Skip);
}

/// `test_before_startup`: no core, so the video reset fails and the audio
/// reset is refused; the data path removal takes the first equal path.
#[test]
fn test_before_startup() {
    assert_eq!(
        video_gates(false, false, false, true, true),
        VideoGate::Fail
    );
    assert!(audio_refused(AudioStep::Core, false));

    // Paths a, b, a; removing c, a, a, a, b, b.
    let mut paths = vec![
        "core_setup_rules_a/",
        "core_setup_rules_b/",
        "core_setup_rules_a/",
    ];
    let mut remove = |path: &str| -> bool {
        let at = paths
            .iter()
            .position(|p| data_path_match(i32::from(*p != path)));
        if let Some(at) = at {
            paths.remove(at);
        }
        at.is_some()
    };
    assert!(!remove("core_setup_rules_c/"));
    assert!(remove("core_setup_rules_a/"));
    assert!(remove("core_setup_rules_a/"));
    assert!(!remove("core_setup_rules_a/"));
    assert!(remove("core_setup_rules_b/"));
    assert!(!remove("core_setup_rules_b/"));
}

// -------------------------------------------------------------- callers

/// `test_reset_video_gates`: no video active, no output starting, so the
/// size gates decide (the fps gate stays in C).
#[test]
fn test_reset_video_gates() {
    assert_eq!(
        video_gates(true, false, false, false, true),
        VideoGate::InvalidParam
    );
    assert_eq!(
        video_gates(true, false, false, true, false),
        VideoGate::InvalidParam
    );
    assert_eq!(video_gates(true, false, false, true, true), VideoGate::Go);
}

/// `test_reset_audio_ticks`.
#[test]
fn test_reset_audio_ticks() {
    assert!(!audio_refused(AudioStep::Core, true));
    assert_eq!(buffering_ms(48000, 0), 45 * 1024 * 1000 / 48000);
    assert_eq!(buffering_ms(48000, 1), 1024 * 1000 / 48000);
    assert_eq!(buffering_ms(48000, 1000), 47 * 1024 * 1000 / 48000);
    assert_eq!(buffering_ms(44100, 1000), 44 * 1024 * 1000 / 44100);
    // Characterized, not endorsed: the frames wrap.
    assert_eq!(buffering_ms(192_000, 22370), 1024 * 1000 / 192_000);
}

/// `test_cmdline_once`: "obs", "--multi", "" set once, the second call
/// ignored, laid out as a table of four pointers and then the strings.
#[test]
fn test_cmdline_once() {
    let argv = ["obs", "--multi", ""];
    assert!(!cmdline_set(0));
    let len = argv.iter().fold(0, |len, arg| cmdline_len(len, arg.len()));
    assert_eq!(len, 13);
    let ptr = size_of::<*const u8>();
    let table = cmdline_table(3, ptr);
    assert_eq!(table, 4 * ptr);
    // The second call sees the stored argc of 3.
    assert!(cmdline_set(3));
    // Each string starts where the one before ended.
    assert_eq!(cmdline_len(0, argv[0].len()), 4);
    assert_eq!(cmdline_len(0, argv[1].len()), 8);
}

/// `test_latest_input_type`.
#[test]
fn test_latest_input_type() {
    assert_eq!(latest_of(&[0, 2, 1]), Some(1));
    assert_eq!(latest_of(&[0x7fff_ffff, 0]), Some(0));
    // Characterized, not endorsed: version 0 wins over 0x80000000.
    assert_eq!(latest_of(&[0, 0x8000_0000]), Some(0));
}

/// `test_monitoring_device`: NULL or empty names and ids are refused
/// before the backend is asked; then the same id needs no change.
#[test]
fn test_monitoring_device() {
    for (name, id) in [
        (None, Some("x")),
        (Some("x"), None),
        (Some(""), Some("x")),
        (Some("x"), Some("")),
        (None, None),
    ] {
        let refused = monitor_missing(name.is_some())
            || monitor_missing(id.is_some())
            || monitor_missing(!name.unwrap_or_default().is_empty())
            || monitor_missing(!id.unwrap_or_default().is_empty());
        assert!(refused, "{name:?} {id:?}");
    }
    assert!(monitor_missing(false));
    assert!(monitor_same(0));
    assert!(!monitor_same(1));
}

/// `test_frame_counts_without_mix`: with no main mix, no mix is asked
/// about; a main mix without a video counts nothing.
#[test]
fn test_frame_counts_without_mix() {
    assert_eq!(counted(true, false, false, false), (false, 2));
}

/// The search of `obs_find_data_file()`: the libobs data directory, then
/// each added path in order, asking after each look whether it ends there.
/// The index of the place the file was found in (0 is the libobs
/// directory), and how many places were looked in.
fn search(has_file: &[bool]) -> (Option<usize>, usize) {
    for (i, &has) in has_file.iter().enumerate() {
        if data_file_found(has) {
            return (Some(i), i + 1);
        }
    }
    (None, has_file.len())
}

/// `test_find_data_file_order`.
#[test]
fn test_find_data_file_order() {
    // Nothing added and nothing in the libobs directory.
    assert_eq!(search(&[false]), (None, 1));
    // a/ and b/ both have it: a/.
    assert_eq!(search(&[false, true, true]), (Some(1), 2));
    // a/ without it is passed over: b/.
    assert_eq!(search(&[false, false, true]), (Some(2), 3));
    // a/ removed: b/; added again after b/: still b/.
    assert_eq!(search(&[false, true]), (Some(1), 2));
    assert_eq!(search(&[false, true, true]), (Some(1), 2));
    // The libobs directory has it: found there, no added path looked at.
    assert_eq!(search(&[true, true, true]), (Some(0), 1));
}

// ---------------------------------------------------------------- edges

#[test]
fn data_file_found_each_value() {
    assert!(data_file_found(true));
    assert!(!data_file_found(false));
}

#[test]
fn video_gate_every_step_and_value() {
    let steps = [
        VideoStep::Core,
        VideoStep::Active,
        VideoStep::Starting,
        VideoStep::OutputSize,
        VideoStep::BaseSize,
    ];
    let go_when = [true, false, false, true, true];
    for (step, go) in steps.into_iter().zip(go_when) {
        assert_eq!(video_gate(step, go), VideoGate::Go, "{step:?}");
        assert_ne!(video_gate(step, !go), VideoGate::Go, "{step:?}");
    }
}

#[test]
fn every_video_gate_combination() {
    for m in 0u32..32 {
        let bit = |n: u32| m & (1 << n) != 0;
        let (core, active, starting, output_ok, base_ok) = (bit(0), bit(1), bit(2), bit(3), bit(4));
        let want = if !core {
            VideoGate::Fail
        } else if active || starting {
            VideoGate::CurrentlyActive
        } else if !output_ok || !base_ok {
            VideoGate::InvalidParam
        } else {
            VideoGate::Go
        };
        assert_eq!(
            video_gates(core, active, starting, output_ok, base_ok),
            want,
            "{m}"
        );
    }
}

#[test]
fn buffer_ticks_edges() {
    assert_eq!(DEFAULT_TICKS, 45);
    // The largest product: u32::MAX frames * 1000 cannot happen, the
    // product wraps first; the ticks never pass (u32::MAX / 1000 + 1023) /
    // 1024.
    assert_eq!(buffer_ticks(1, u32::MAX), (u32::MAX / 1000).div_ceil(1024));
    assert_eq!(buffer_ticks(u32::MAX, 1), (u32::MAX / 1000).div_ceil(1024));
    assert_eq!(buffer_ticks(0, 0), 0);
    assert_eq!(buffer_ticks(1000, 0), 0);
    // 1024 frames exactly, then one more.
    assert_eq!(buffer_ticks(1024, 1000), 1);
    assert_eq!(buffer_ticks(1, 1_024_001), 1);
    assert_eq!(buffer_ticks(1, 1_025_000), 2);
}

#[test]
fn cmdline_edges() {
    assert!(cmdline_set(i32::MIN));
    assert!(cmdline_set(i32::MAX));
    assert_eq!(cmdline_len(usize::MAX, 0), 0);
    assert_eq!(cmdline_len(usize::MAX - 1, 0), usize::MAX);
    assert_eq!(cmdline_len(0, usize::MAX), 0);
    assert_eq!(cmdline_table(0, 0), 0);
    assert_eq!(cmdline_table(-2, 1), usize::MAX);
    assert_eq!(cmdline_table(-2, 8), usize::MAX.wrapping_mul(8));
    assert_eq!(
        cmdline_table(i32::MIN, 1),
        (i32::MIN as isize + 1).cast_unsigned()
    );
    assert_eq!(cmdline_table(1, usize::MAX), usize::MAX.wrapping_mul(2));
}

#[test]
fn latest_edges() {
    assert!(latest_newer(0x7fff_ffff, 0x7fff_fffe));
    assert!(!latest_newer(0x7fff_ffff, i32::MAX));
    assert!(!latest_newer(0x8000_0000, i32::MIN + 1));
    assert!(latest_newer(0x8000_0001, i32::MIN));
    assert!(!latest_newer(0, 0));
    assert!(latest_newer(0, i32::MIN));
}

#[test]
fn mix_step_every_step_and_value() {
    use Mix::{Count, EncoderOnly, Main, Skip, Video, View};
    let table = [
        (Main, true, Video),
        (Main, false, EncoderOnly),
        (EncoderOnly, true, View),
        (EncoderOnly, false, Skip),
        (View, true, Video),
        (View, false, Skip),
        (Video, true, Count),
        (Video, false, Skip),
        (Count, true, Count),
        (Count, false, Count),
        (Skip, true, Skip),
        (Skip, false, Skip),
    ];
    for (step, value, next) in table {
        assert_eq!(mix_step(step, value), next, "{step:?} {value}");
    }
}

#[test]
fn every_mix_combination() {
    for m in 0u32..16 {
        let bit = |n: u32| m & (1 << n) != 0;
        let (is_main, encoder_only, same_view, has_video) = (bit(0), bit(1), bit(2), bit(3));
        let want = (is_main || (encoder_only && same_view)) && has_video;
        assert_eq!(
            counted(is_main, encoder_only, same_view, has_video).0,
            want,
            "{m}"
        );
    }
}

proptest! {
    #[test]
    fn buffer_ticks_any(ms in any::<u32>(), rate in any::<u32>()) {
        let frames = ms.wrapping_mul(rate) / 1000;
        let ticks = buffer_ticks(ms, rate);
        prop_assert_eq!(ticks, frames.div_ceil(AUDIO_OUTPUT_FRAMES));
        prop_assert!(ticks <= (u32::MAX / 1000).div_ceil(AUDIO_OUTPUT_FRAMES));
        prop_assert_eq!(buffer_set(ms), ms != 0);
    }

    #[test]
    fn cmdline_sizes_any(lens in prop::collection::vec(0usize..4096, 0..16), ptr in 1usize..=16) {
        let len = lens.iter().fold(0, |len, &arg| cmdline_len(len, arg));
        prop_assert_eq!(len, lens.iter().sum::<usize>() + lens.len());
        let argc = i32::try_from(lens.len()).unwrap();
        prop_assert_eq!(cmdline_table(argc, ptr), (lens.len() + 1) * ptr);
    }

    #[test]
    fn cmdline_set_any(argc in any::<i32>()) {
        prop_assert_eq!(cmdline_set(argc), argc != 0);
    }

    #[test]
    fn latest_any(version in any::<u32>(), best in any::<i32>()) {
        prop_assert_eq!(latest_newer(version, best), i64::from(version.cast_signed()) > i64::from(best));
    }

    #[test]
    fn latest_of_any(versions in prop::collection::vec(any::<u32>(), 0..12)) {
        // The first of the highest version that is not negative as i32.
        let best = versions
            .iter()
            .enumerate()
            .filter(|(_, v)| v.cast_signed() >= 0)
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(&a.0)))
            .map(|(i, _)| i);
        prop_assert_eq!(latest_of(&versions), best);
    }

    #[test]
    fn flags_and_compares_any(flags in any::<u32>(), cmp in any::<i32>(), present in any::<bool>()) {
        prop_assert_eq!(aoc_check(flags), flags & (1 << 9) != 0);
        prop_assert_eq!(data_path_match(cmp), cmp == 0);
        prop_assert_eq!(monitor_same(cmp), cmp == 0);
        prop_assert_eq!(monitor_missing(present), !present);
    }
}
