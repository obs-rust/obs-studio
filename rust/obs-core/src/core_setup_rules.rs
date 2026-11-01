//! Safe core of `libobs/obs-core-setup-rules.c.h`,
//! `libobs/obs-core-setup-rules-ms.c.h` and
//! `libobs/obs-core-setup-rules-task.c.h`: the setup rules of `obs.c`.
//!
//! Each rule is asked where the C decided it, from the value `obs.c` reads
//! at that point, so nothing is read or called that was not before:
//! - [`video_gate`]: `obs_reset_video()`'s gates, one [`VideoStep`] at a
//!   time: no core fails, an active video or a starting output is busy, a
//!   bad output or base size is an invalid parameter;
//! - [`audio_refused`]: `obs_reset_audio2()`'s gates, no core, then an
//!   active audio output (asked only when there is one);
//! - [`buffer_set`], [`buffer_ticks`] and [`buffer_default_ticks`]: the
//!   audio buffering in ticks of [`AUDIO_OUTPUT_FRAMES`]: 0 ms takes the
//!   default, any other is rounded up from `ms * rate / 1000` frames, in
//!   `u32` (the product wraps);
//! - [`buffer_ms`]: those ticks back in milliseconds, for the reset's log
//!   and `obs_get_audio_info2()`, the rate cast to `i32` as the C does; a
//!   zero rate gives 0 and the result is clamped to `i32`, where the C traps
//!   and overflows (#3272);
//! - [`cmdline_set`], [`cmdline_len`] and [`cmdline_table`]: the command
//!   line is set once (while its stored `argc` is 0), and its copy's
//!   strings (each with its terminator) follow a table of `argc + 1`
//!   pointers;
//! - [`data_file_found`]: the data file search ends at its first hit, in
//!   the libobs data directory first, then in each added path in order;
//! - [`data_path_match`]: the first data path equal to the one removed;
//! - [`latest_newer`]: a source type's version is above the best so far,
//!   compared as `i32` from [`LATEST_NONE`], so the first of equal versions
//!   wins and a version above `i32::MAX` never does;
//! - [`aoc_check`]: a source whose output flags ask for its capture device
//!   to be checked;
//! - [`monitor_missing`] and [`monitor_same`]: the monitoring device's
//!   refusals and its same-id shortcut;
//! - [`mix_step`]: the frame-count rule, one read of the mix at a time: a
//!   mix counts when it is the main mix, or an encoder-only mix of the main
//!   mix's view, and has a video output;
//! - [`task_step`]: `obs_queue_task()`'s route, one read at a time: a UI
//!   task goes to the UI task handler when there is one, any other runs in
//!   place on its own thread, waits for itself when asked to, or is queued
//!   for its thread; a type outside the four is refused (#3397);
//! - [`task_thread`]: the thread `obs_in_task_thread()` asks about;
//! - [`levels_core`] and [`level_ok`]: `obs_set_video_levels()` needs a
//!   core (#3397), and keeps a level that is finite and greater than 0
//!   (#2209).
//!
//! `obs.c` keeps every lock, every read and call, the fps gate, every
//! allocation, copy, free and signal, every divide by a variable but the
//! buffering's milliseconds, and every log message. Nothing here panics, indexes or overflows for any input.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable
)]

/// `AUDIO_OUTPUT_FRAMES`: the frames in one audio tick.
pub const AUDIO_OUTPUT_FRAMES: u32 = 1024;

/// The buffering `obs_reset_audio2()` takes for a `max_buffering_ms` of 0,
/// in ticks.
pub const DEFAULT_TICKS: u32 = 45;

/// `SEC_TO_MSEC`: the milliseconds in a second.
pub const SEC_TO_MSEC: i32 = 1000;

/// The version `obs_get_latest_input_type_id()` starts from.
pub const LATEST_NONE: i32 = -1;

/// `OBS_SOURCE_DO_NOT_SELF_MONITOR`.
pub const DO_NOT_SELF_MONITOR: u32 = 1 << 9;

/// `OBS_TASK_UI`.
pub const TASK_UI: i32 = 0;
/// `OBS_TASK_GRAPHICS`.
pub const TASK_GRAPHICS: i32 = 1;
/// `OBS_TASK_AUDIO`.
pub const TASK_AUDIO: i32 = 2;
/// `OBS_TASK_DESTROY`.
pub const TASK_DESTROY: i32 = 3;

/// A step of `obs_reset_video()`'s gates, in order, with the value each one
/// is asked with (`enum obs_core_setup_rules_video_step`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoStep {
    /// `obs != NULL`.
    Core,
    /// `obs_video_active()`.
    Active,
    /// `outputs_starting()`.
    Starting,
    /// The output size is valid.
    OutputSize,
    /// The base size is valid.
    BaseSize,
}

/// What a video gate answers: go on, or the `OBS_VIDEO_*` code to return.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoGate {
    /// Go on to the next step (`OBS_VIDEO_SUCCESS`).
    Go,
    /// No core (`OBS_VIDEO_FAIL`).
    Fail,
    /// Video is active, or an output is starting
    /// (`OBS_VIDEO_CURRENTLY_ACTIVE`).
    CurrentlyActive,
    /// A size is not valid (`OBS_VIDEO_INVALID_PARAM`).
    InvalidParam,
}

/// A step of `obs_reset_audio2()`'s gates, in order
/// (`enum obs_core_setup_rules_audio_step`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioStep {
    /// `obs != NULL`.
    Core,
    /// `audio_output_active()`, asked only when there is an audio output.
    Active,
}

/// What `obs_get_video_frame_counts()` reads next about a mix, each value
/// asked with that read, or what it does with the mix
/// (`enum obs_core_setup_rules_mix`). The steps only go forward, in this
/// order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mix {
    /// `mix == main_mix`.
    Main,
    /// `mix->encoder_only_mix`.
    EncoderOnly,
    /// `mix->view == main_mix->view`.
    View,
    /// `mix->video != NULL`.
    Video,
    /// Count the mix's frames.
    Count,
    /// Skip the mix.
    Skip,
}

/// What `obs_queue_task()` reads next about a task, each value asked with
/// that read and the type with every step, or the route it takes
/// (`enum obs_core_setup_rules_task`). The steps only go forward, in this
/// order; a route stays where it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    /// The type alone.
    Type,
    /// `obs->ui_task_handler != NULL`, asked only for [`TASK_UI`].
    UiHandler,
    /// `obs_in_task_thread(type)`, never asked for [`TASK_UI`].
    InThread,
    /// `wait`.
    Wait,
    /// Hand the task to the UI task handler.
    UiCall,
    /// Log that there is no UI task handler.
    UiNone,
    /// Run the task in place: the caller is on its thread.
    Run,
    /// Queue the task again without waiting, then wait for it.
    WaitQueue,
    /// Push the task to the graphics thread's queue.
    PushGraphics,
    /// Push the task to the audio thread's queue.
    PushAudio,
    /// Queue the task on the destruction task queue.
    PushDestroy,
    /// Queue nothing: a type with no queue, asked at the wait step.
    Drop,
    /// Log the unknown type and queue nothing.
    Unknown,
}

/// Same contract as C `obs_core_setup_rules_video_gate`.
pub fn video_gate(step: VideoStep, value: bool) -> VideoGate {
    match step {
        VideoStep::Core if !value => VideoGate::Fail,
        VideoStep::Active | VideoStep::Starting if value => VideoGate::CurrentlyActive,
        VideoStep::OutputSize | VideoStep::BaseSize if !value => VideoGate::InvalidParam,
        _ => VideoGate::Go,
    }
}

/// Same contract as C `obs_core_setup_rules_audio_refused`: refused with no
/// core, or with an active audio output.
pub fn audio_refused(step: AudioStep, value: bool) -> bool {
    match step {
        AudioStep::Core => !value,
        AudioStep::Active => value,
    }
}

/// Same contract as C `obs_core_setup_rules_buffer_set`: a buffering was
/// asked for (not 0 ms).
pub fn buffer_set(max_buffering_ms: u32) -> bool {
    max_buffering_ms != 0
}

/// Same contract as C `obs_core_setup_rules_buffer_ticks`: the frames in
/// `max_buffering_ms` at `samples_per_sec`, rounded down, then rounded up
/// to whole ticks. The product wraps in `u32`, as the C's.
pub fn buffer_ticks(max_buffering_ms: u32, samples_per_sec: u32) -> u32 {
    let frames = max_buffering_ms.wrapping_mul(samples_per_sec) / 1000;
    frames.wrapping_add(AUDIO_OUTPUT_FRAMES - 1) / AUDIO_OUTPUT_FRAMES
}

/// Same contract as C `obs_core_setup_rules_buffer_default_ticks`.
pub fn buffer_default_ticks() -> u32 {
    DEFAULT_TICKS
}

/// Same contract as C `obs_core_setup_rules_buffer_ms`: `ticks` of
/// [`AUDIO_OUTPUT_FRAMES`] in milliseconds at `samples_per_sec`, rounded
/// toward zero, with the rate cast to `i32` as the C casts it to `int`.
///
/// Two intentional differences from the C (#3272): a zero rate gives 0,
/// where the C's divide traps, so `obs_reset_audio2()` goes on to
/// `obs_init_audio()`, which refuses the rate; and the product is an
/// `i64` and the quotient is clamped to `i32`, where the C's `int` product
/// overflows from 2098 ticks.
pub fn buffer_ms(ticks: i32, samples_per_sec: u32) -> i32 {
    // |ticks| * 1024 * 1000 < 2^51: no i64 product wraps.
    let product = i64::from(ticks)
        .wrapping_mul(i64::from(AUDIO_OUTPUT_FRAMES))
        .wrapping_mul(i64::from(SEC_TO_MSEC));
    // The divisor is an i32 widened, never -1 with an i64::MIN product, so
    // only a zero rate has no quotient.
    let ms = product
        .checked_div(i64::from(samples_per_sec.cast_signed()))
        .unwrap_or(0);
    i32::try_from(ms).unwrap_or(if ms < 0 { i32::MIN } else { i32::MAX })
}

/// Same contract as C `obs_core_setup_rules_cmdline_set`: the command line
/// was set already, so a new one is ignored.
pub fn cmdline_set(stored_argc: i32) -> bool {
    stored_argc != 0
}

/// Same contract as C `obs_core_setup_rules_cmdline_len`: `len` with one
/// more string of `arg_len` bytes and its terminator. Wraps, as the C's
/// `size_t`.
pub fn cmdline_len(len: usize, arg_len: usize) -> usize {
    len.wrapping_add(arg_len).wrapping_add(1)
}

/// Same contract as C `obs_core_setup_rules_cmdline_table`: the size of a
/// table of `argc + 1` pointers of `ptr_size` bytes. `argc + 1` is taken as
/// the C converts it to `size_t` (a negative count wraps), and the product
/// wraps, as the C's.
pub fn cmdline_table(argc: i32, ptr_size: usize) -> usize {
    (argc as isize)
        .cast_unsigned()
        .wrapping_add(1)
        .wrapping_mul(ptr_size)
}

/// Same contract as C `obs_core_setup_rules_data_file_found`:
/// `obs_find_data_file()` stops at a hit, asked after the libobs data
/// directory and after each added path.
pub fn data_file_found(found: bool) -> bool {
    found
}

/// Same contract as C `obs_core_setup_rules_data_path_match`: `dstr_cmp()`
/// found the path equal.
pub fn data_path_match(cmp: i32) -> bool {
    cmp == 0
}

/// Same contract as C `obs_core_setup_rules_latest_newer`: `version`, as
/// `i32`, is above `best`.
pub fn latest_newer(version: u32, best: i32) -> bool {
    version.cast_signed() > best
}

/// Same contract as C `obs_core_setup_rules_aoc_check`.
pub fn aoc_check(output_flags: u32) -> bool {
    output_flags & DO_NOT_SELF_MONITOR != 0
}

/// Same contract as C `obs_core_setup_rules_monitor_missing`: a name, an
/// id, either's first character, or the monitoring backend is missing.
pub fn monitor_missing(present: bool) -> bool {
    !present
}

/// Same contract as C `obs_core_setup_rules_monitor_same`: `strcmp()`
/// found the id equal to the current one.
pub fn monitor_same(cmp: i32) -> bool {
    cmp == 0
}

/// Same contract as C `obs_core_setup_rules_mix_step`: the next read after
/// `step` answered `value`, or what to do with the mix. [`Mix::Count`] and
/// [`Mix::Skip`] stay where they are.
pub fn mix_step(step: Mix, value: bool) -> Mix {
    match (step, value) {
        (Mix::Main, true) | (Mix::View, true) => Mix::Video,
        (Mix::Main, false) => Mix::EncoderOnly,
        (Mix::EncoderOnly, true) => Mix::View,
        (Mix::EncoderOnly | Mix::View | Mix::Video, false) => Mix::Skip,
        (Mix::Video, true) => Mix::Count,
        (Mix::Count | Mix::Skip, _) => step,
    }
}

/// Same contract as C `obs_core_setup_rules_task_step`: the next read after
/// `step` answered `value` for a task of `task_type`, or its route. Unlike
/// the C (#3397), the type step routes a type outside the four to
/// [`Task::Unknown`], so `obs_queue_task()` never asks
/// `obs_in_task_thread()` about it, which asserts, or waits for a task it
/// never queues. A route stays where it is.
pub fn task_step(step: Task, task_type: i32, value: bool) -> Task {
    match step {
        Task::Type => match task_type {
            TASK_UI => Task::UiHandler,
            TASK_GRAPHICS | TASK_AUDIO | TASK_DESTROY => Task::InThread,
            _ => Task::Unknown,
        },
        Task::UiHandler if value => Task::UiCall,
        Task::UiHandler => Task::UiNone,
        Task::InThread if value => Task::Run,
        Task::InThread => Task::Wait,
        Task::Wait if value => Task::WaitQueue,
        Task::Wait => match task_type {
            TASK_GRAPHICS => Task::PushGraphics,
            TASK_AUDIO => Task::PushAudio,
            TASK_DESTROY => Task::PushDestroy,
            _ => Task::Drop,
        },
        Task::UiCall
        | Task::UiNone
        | Task::Run
        | Task::WaitQueue
        | Task::PushGraphics
        | Task::PushAudio
        | Task::PushDestroy
        | Task::Drop
        | Task::Unknown => step,
    }
}

/// Same contract as C `obs_core_setup_rules_task_thread`: the thread
/// `obs_in_task_thread()` asks about, or -1, which still reaches its
/// assert.
pub fn task_thread(task_type: i32) -> i32 {
    match task_type {
        TASK_GRAPHICS | TASK_AUDIO | TASK_UI | TASK_DESTROY => task_type,
        _ => -1,
    }
}

/// Same contract as C `obs_core_setup_rules_levels_core`:
/// `obs_set_video_levels()` goes on. Unlike the C (#3397), not without a
/// core: it warns and returns before it reads `obs->video`.
pub fn levels_core(core: bool) -> bool {
    core
}

/// Same contract as C `obs_core_setup_rules_level_ok`: a video level the
/// draw paths can divide by, finite and greater than 0 (#2209). -0.0 and
/// NaN are refused, as `isfinite(level) && level > 0.f` refuses them.
pub fn level_ok(level: f32) -> bool {
    level.is_finite() && level > 0.0
}
