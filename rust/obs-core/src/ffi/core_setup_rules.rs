//! C ABI for `libobs/obs-core-setup-rules.c.h`.
//!
//! `obs_reset_video()`, `obs_reset_audio2()`, `obs_set_cmdline_args()`,
//! `obs_find_data_file()`, `obs_remove_data_path()`, `obs_get_latest_input_type_id()`,
//! `check_all_aoc_sources()`, `obs_set_audio_monitoring_device()` and
//! `obs_get_video_frame_counts()` in `obs.c` call these inside libobs. They
//! are declared in `obs-core-setup-rules.h` without `EXPORT`, so the shims
//! stay hidden (`libobs/cmake/rust-hidden.txt`).
//!
//! The C's enums are 32-bit integers on every target libobs builds for
//! (`unsigned int` with GCC and Clang, `int` with MSVC), taken and returned
//! as `u32` here. A step the C enum does not name is answered as the C's
//! `switch` falls through: the video gate goes on, the audio gate does not
//! refuse, and the mix step comes back unchanged. The video gate returns
//! the `OBS_VIDEO_*` code as an `int`.

use core::ffi::c_int;

use crate::core_setup_rules::{self, AudioStep, Mix, VideoGate, VideoStep};

/// `OBS_CORE_SETUP_RULES_VIDEO_GO` (`OBS_VIDEO_SUCCESS`).
pub const VIDEO_GO: c_int = 0;
/// `OBS_CORE_SETUP_RULES_VIDEO_FAIL` (`OBS_VIDEO_FAIL`).
pub const VIDEO_FAIL: c_int = -1;
/// `OBS_CORE_SETUP_RULES_VIDEO_INVALID_PARAM` (`OBS_VIDEO_INVALID_PARAM`).
pub const VIDEO_INVALID_PARAM: c_int = -3;
/// `OBS_CORE_SETUP_RULES_VIDEO_CURRENTLY_ACTIVE`
/// (`OBS_VIDEO_CURRENTLY_ACTIVE`).
pub const VIDEO_CURRENTLY_ACTIVE: c_int = -4;

/// `enum obs_core_setup_rules_video_step`, in order from 0.
const VIDEO_STEPS: [VideoStep; 5] = [
    VideoStep::Core,
    VideoStep::Active,
    VideoStep::Starting,
    VideoStep::OutputSize,
    VideoStep::BaseSize,
];

/// `enum obs_core_setup_rules_audio_step`, in order from 0.
const AUDIO_STEPS: [AudioStep; 2] = [AudioStep::Core, AudioStep::Active];

/// `enum obs_core_setup_rules_mix`, in order from 0.
const MIX_STEPS: [Mix; 6] = [
    Mix::Main,
    Mix::EncoderOnly,
    Mix::View,
    Mix::Video,
    Mix::Count,
    Mix::Skip,
];

fn step_of<T: Copy>(steps: &[T], raw: u32) -> Option<T> {
    usize::try_from(raw)
        .ok()
        .and_then(|i| steps.get(i))
        .copied()
}

/// Same contract as C `obs_core_setup_rules_video_gate`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_video_gate(step: u32, value: bool) -> c_int {
    let Some(step) = step_of(&VIDEO_STEPS, step) else {
        return VIDEO_GO;
    };
    match core_setup_rules::video_gate(step, value) {
        VideoGate::Go => VIDEO_GO,
        VideoGate::Fail => VIDEO_FAIL,
        VideoGate::CurrentlyActive => VIDEO_CURRENTLY_ACTIVE,
        VideoGate::InvalidParam => VIDEO_INVALID_PARAM,
    }
}

/// Same contract as C `obs_core_setup_rules_audio_refused`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_audio_refused(step: u32, value: bool) -> bool {
    step_of(&AUDIO_STEPS, step).is_some_and(|step| core_setup_rules::audio_refused(step, value))
}

/// Same contract as C `obs_core_setup_rules_buffer_set`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_buffer_set(max_buffering_ms: u32) -> bool {
    core_setup_rules::buffer_set(max_buffering_ms)
}

/// Same contract as C `obs_core_setup_rules_buffer_ticks`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_buffer_ticks(
    max_buffering_ms: u32,
    samples_per_sec: u32,
) -> u32 {
    core_setup_rules::buffer_ticks(max_buffering_ms, samples_per_sec)
}

/// Same contract as C `obs_core_setup_rules_buffer_default_ticks`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_buffer_default_ticks() -> u32 {
    core_setup_rules::buffer_default_ticks()
}

/// Same contract as C `obs_core_setup_rules_cmdline_set`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_cmdline_set(stored_argc: c_int) -> bool {
    core_setup_rules::cmdline_set(stored_argc)
}

/// Same contract as C `obs_core_setup_rules_cmdline_len`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_cmdline_len(len: usize, arg_len: usize) -> usize {
    core_setup_rules::cmdline_len(len, arg_len)
}

/// Same contract as C `obs_core_setup_rules_cmdline_table`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_cmdline_table(argc: c_int, ptr_size: usize) -> usize {
    core_setup_rules::cmdline_table(argc, ptr_size)
}

/// Same contract as C `obs_core_setup_rules_data_file_found`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_data_file_found(found: bool) -> bool {
    core_setup_rules::data_file_found(found)
}

/// Same contract as C `obs_core_setup_rules_data_path_match`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_data_path_match(cmp: c_int) -> bool {
    core_setup_rules::data_path_match(cmp)
}

/// Same contract as C `obs_core_setup_rules_latest_newer`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_latest_newer(version: u32, best: c_int) -> bool {
    core_setup_rules::latest_newer(version, best)
}

/// Same contract as C `obs_core_setup_rules_aoc_check`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_aoc_check(output_flags: u32) -> bool {
    core_setup_rules::aoc_check(output_flags)
}

/// Same contract as C `obs_core_setup_rules_monitor_missing`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_monitor_missing(present: bool) -> bool {
    core_setup_rules::monitor_missing(present)
}

/// Same contract as C `obs_core_setup_rules_monitor_same`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_monitor_same(cmp: c_int) -> bool {
    core_setup_rules::monitor_same(cmp)
}

/// Same contract as C `obs_core_setup_rules_mix_step`.
#[unsafe(no_mangle)]
pub extern "C" fn obs_core_setup_rules_mix_step(step: u32, value: bool) -> u32 {
    let Some(mix) = step_of(&MIX_STEPS, step) else {
        return step;
    };
    let next = core_setup_rules::mix_step(mix, value);
    MIX_STEPS
        .iter()
        .position(|&m| m == next)
        .and_then(|i| u32::try_from(i).ok())
        .unwrap_or(step)
}
