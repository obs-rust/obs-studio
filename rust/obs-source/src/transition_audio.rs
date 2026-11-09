//! Safe core of the rest of `libobs/obs-source-transition.c.h`: the audio
//! render, init and free of a transition.
//!
//! - [`init`]: `obs_transition_init()`, the transition part of a source's
//!   create: both mutexes, the alignment, the two texrenders and slot A
//!   counted as active;
//! - [`free`]: `obs_transition_free()`, the transition part of its destroy:
//!   both mutexes, then the texrenders inside the graphics context;
//! - [`audio_render`]: `obs_transition_audio_render()`, with the C's
//!   `transition_live_sources()`, `transition_calc_min_ts()`,
//!   `transition_process_audio()` and `mix_child()`: the transition's audio
//!   for one tick, mixed from its children with the type's two mix
//!   callbacks while its audio transitions, else slot A's copied.
//!
//! Each is generic over a [`Transition`], which extends
//! [`crate::transition_stop::Transition`] (the slots, clock fields, lock,
//! stop callback, signals and release) with one method per further field
//! read or write, mutex call, child read, buffer access and call into the
//! rest of libobs, so a function makes the C's calls in the C's order and
//! under the C's locks. The rules are the Rust ones the C calls:
//! [`crate::transition_render`] (the audio stop, mix, valid and place
//! plans), [`crate::transition_sample_time`] (each sample's time) and
//! [`crate::transition_clock::min_ts`], and the stop helpers of
//! [`crate::transition_stop`] (`transition_calc_time()`,
//! `transition_stop()` and `transition_handle_stop()`), called here
//! directly.
//!
//! The C shims live in the `obs-source-seam` crate, which implements
//! [`Transition`] over `libobs/obs-source-transition-seam.h`, the caller's
//! and the children's audio buffers and the libobs calls; only libobs-rust
//! links it, so this crate calls nothing in libobs.
//!
//! Kept from the C:
//! - the mix reads only the clock copied under `transition_mutex` (#3499):
//!   [`audio_render`] copies the flags and clock with the children's
//!   references while it holds the lock, and every sample's time comes from
//!   that copy;
//! - the children's `audio_pending` and `audio_is_duplicated` are read with
//!   `transition_mutex` held and none of the child's locks;
//! - after an audio stop, the children are read again before the copy: the
//!   stop can move a removed child into slot A (#2185);
//! - slot A's audio is copied as one block of `TOTAL_AUDIO_SIZE` bytes from
//!   its `audio_output_buf[0][0]`, every mix and channel;
//! - [`init`] returns on a failed mutex init before the texrenders are made;
//!   [`free`] destroys the mutexes before it enters the graphics context.
//!
//! No intentional differences from the C.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::todo
)]
use std::ffi::CStr;

use crate::transition_clock::{self, Clock};
use crate::transition_render::{self, AUDIO_FRAMES, AudioStep, Place};
use crate::transition_sample_time;
use crate::transition_stop::{self, SOURCE_A, SOURCE_B};

/// The name [`audio_render`] hands `transition_valid()`, which logs it for a
/// NULL source.
pub const AUDIO_RENDER_LOG_NAME: &CStr = c"obs_transition_audio_render";

/// `MAX_AUDIO_MIXES` (`media-io/audio-io.h`): the mixes [`audio_render`]
/// may write, one bit of the mixer mask each.
pub const MAX_AUDIO_MIXES: usize = 6;

/// `OBS_ALIGN_LEFT | OBS_ALIGN_TOP`: the alignment [`init`] sets.
pub const INIT_ALIGNMENT: u32 = (1 << 0) | (1 << 2);

/// The slots of a transition, as [`transition_stop`] numbers them.
pub const SLOTS: [usize; 2] = [transition_stop::SOURCE_A, transition_stop::SOURCE_B];

/// The fields, mutexes, buffers and libobs calls of a transition that its
/// audio render, init and free use beyond those of
/// [`transition_stop::Transition`]: in libobs, the transition seam
/// (`obs-source-transition-seam.h`), the audio buffers and the libobs
/// functions, one call per method. Every method takes `&self`, as the C
/// writes through its `obs_source_t *`.
pub trait Transition: transition_stop::Transition {
    /// A texrender, as `transition_texrender[idx]` holds it.
    type Texrender: Copy + PartialEq;
    /// The graphics context, `obs->video.graphics`.
    type Graphics: Copy;
    /// The transition's `context.data`, which the mix callbacks get.
    type Data: Copy;
    /// One of the type's mix callbacks
    /// (`obs_transition_audio_mix_callback_t`).
    type MixCallback: Copy;
    /// The caller's mix, the `struct obs_source_audio_mix *` the render
    /// writes.
    type Mix: Copy;
    /// A child's audio, as `obs_source_get_audio_mix()` fills a
    /// `struct obs_source_audio_mix`.
    type ChildAudio;
    /// The samples of one channel: a `float *`.
    type Samples: Copy;

    /// `pthread_mutex_init_value()` on `transition_mutex`, then on
    /// `transition_tex_mutex`.
    fn init_mutex_values(&self);
    /// `pthread_mutex_init(&transition_mutex, NULL)`, its result.
    fn mutex_init(&self) -> i32;
    /// `pthread_mutex_init(&transition_tex_mutex, NULL)`, its result.
    fn tex_mutex_init(&self) -> i32;
    /// `pthread_mutex_destroy(&transition_mutex)`.
    fn mutex_destroy(&self);
    /// `pthread_mutex_destroy(&transition_tex_mutex)`.
    fn tex_mutex_destroy(&self);
    /// Writes `transition_alignment`.
    fn set_alignment(&self, value: u32);
    /// `transition_texrender[idx]`.
    fn texrender(&self, idx: usize) -> Option<Self::Texrender>;
    /// Writes `transition_texrender[idx]`.
    fn set_texrender(&self, idx: usize, value: Option<Self::Texrender>);
    /// `transitioning_video`, an atomic load.
    fn transitioning_video(&self) -> bool;
    /// `transitioning_audio`, an atomic load.
    fn transitioning_audio(&self) -> bool;
    /// Stores `transitioning_audio`, atomically.
    fn set_transitioning_audio(&self, value: bool);
    /// `context.data`.
    fn context_data(&self) -> Self::Data;

    /// `obs->video.graphics`.
    fn graphics(&self) -> Option<Self::Graphics>;
    /// `gs_texrender_create(GS_RGBA, GS_ZS_NONE)`.
    fn texrender_create(&self) -> Option<Self::Texrender>;
    /// `gs_texrender_destroy(texrender)`, which does nothing for `None`.
    fn texrender_destroy(&self, texrender: Option<Self::Texrender>);
    /// `gs_enter_context(graphics)`.
    fn enter_context(&self, graphics: Option<Self::Graphics>);
    /// `gs_leave_context()`.
    fn leave_context(&self);

    /// `obs_source_removed(child)`.
    fn removed(&self, child: Self::Child) -> bool;
    /// The child's `audio_pending`, read with no lock of the child's.
    fn child_audio_pending(&self, child: Self::Child) -> bool;
    /// The child's `audio_is_duplicated`, read with no lock of the child's.
    fn child_audio_is_duplicated(&self, child: Self::Child) -> bool;
    /// `obs_source_get_audio_timestamp(child)`.
    fn audio_timestamp(&self, child: Self::Child) -> u64;
    /// `obs_source_get_ref(child)`, `None` for `None` or a source being
    /// destroyed.
    fn get_ref(&self, child: Option<Self::Child>) -> Option<Self::Child>;
    /// `obs_source_get_audio_mix(child, &child_audio)`.
    fn audio_mix(&self, child: Self::Child) -> Self::ChildAudio;
    /// The child's `audio_output_buf[0][0]`, the start of the block all its
    /// mixes and channels share.
    fn child_audio_output_buf(&self, child: Self::Child) -> Self::Samples;

    /// `mix(data, t)`: the type's callback.
    fn call_mix(&self, mix: Self::MixCallback, data: Self::Data, t: f32) -> f32;
    /// `audio->output[mix_idx].data[ch]` of the caller's mix.
    fn output_samples(&self, audio: Self::Mix, mix_idx: usize, ch: usize) -> Self::Samples;
    /// `child_audio.output[mix_idx].data[ch]` of a child's audio.
    fn input_samples(&self, audio: &Self::ChildAudio, mix_idx: usize, ch: usize) -> Self::Samples;
    /// `samples[frame]`.
    fn sample(&self, samples: Self::Samples, frame: usize) -> f32;
    /// Writes `samples[frame]`.
    fn set_sample(&self, samples: Self::Samples, frame: usize, value: f32);
    /// `memcpy(audio->output[0].data[0], from, TOTAL_AUDIO_SIZE)`.
    fn copy_audio(&self, audio: Self::Mix, from: Self::Samples);
}

/// What the caller of [`audio_render`] asks for: its mix, the mixes and
/// channels to write, the output's sample rate and the type's two mix
/// callbacks.
pub struct Request<T: Transition> {
    /// The caller's mix.
    pub audio: T::Mix,
    /// The mixer mask: bit `i` selects mix `i`.
    pub mixers: u32,
    /// The channels of each mix to write.
    pub channels: usize,
    /// The output's sample rate.
    pub sample_rate: usize,
    /// The callback slot A's child mixes with.
    pub mix_a: T::MixCallback,
    /// The callback slot B's child mixes with.
    pub mix_b: T::MixCallback,
}

impl<T: Transition> Clone for Request<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Transition> Copy for Request<T> {}

/// `obs_transition_init()`: true when both mutexes and both texrenders
/// were made.
pub fn init<T: Transition>(transition: &T) -> bool {
    transition.init_mutex_values();
    if transition.mutex_init() != 0 {
        return false;
    }
    if transition.tex_mutex_init() != 0 {
        return false;
    }

    transition.set_alignment(INIT_ALIGNMENT);
    transition.set_texrender(SOURCE_A, transition.texrender_create());
    transition.set_texrender(SOURCE_B, transition.texrender_create());
    transition.set_source_active(SOURCE_A, true);

    transition.texrender(SOURCE_A).is_some() && transition.texrender(SOURCE_B).is_some()
}

/// `obs_transition_free()`.
pub fn free<T: Transition>(transition: &T) {
    transition.mutex_destroy();
    transition.tex_mutex_destroy();

    transition.enter_context(transition.graphics());
    transition.texrender_destroy(transition.texrender(SOURCE_A));
    transition.texrender_destroy(transition.texrender(SOURCE_B));
    transition.leave_context();
}

/// `struct transition_state` as the audio render fills it under the lock:
/// the children's references, the audio flag and the clock. The C copies
/// `transitioning_video` too, and nothing reads it; the read is kept.
struct Copied<C> {
    sources: [Option<C>; 2],
    transitioning_audio: bool,
    clock: Clock,
}

/// `mix_child()`: `count` samples of one channel of the child's audio
/// `input`, from its first, times the callback's value at each sample's
/// time, added into `output` from frame `pos`. The context data is read
/// once, before the first sample.
#[allow(clippy::too_many_arguments)]
fn mix_child<T: Transition>(
    transition: &T,
    clock: Clock,
    output: T::Samples,
    pos: usize,
    input: T::Samples,
    count: usize,
    sample_rate: usize,
    ts: u64,
    mix: T::MixCallback,
) {
    let context_data = transition.context_data();

    for i in 0..count {
        let t = transition_sample_time::sample_time(clock, ts, sample_rate, i);
        let value = transition.call_mix(mix, context_data, t);
        // pos + i < AUDIO_FRAMES: the place rule caps pos, count is the rest.
        let frame = pos.wrapping_add(i);
        let sum = transition.sample(output, frame) + transition.sample(input, i) * value;
        transition.set_sample(output, frame, sum);
    }
}

/// `transition_process_audio()`: one child's audio mixed into the caller's
/// mix with `mix`, for each mix the mask selects and each channel asked
/// for, when the child is there, its audio neither pending nor duplicated,
/// and it starts within the tick.
fn process_audio<T: Transition>(
    transition: &T,
    clock: Clock,
    child: Option<T::Child>,
    request: &Request<T>,
    min_ts: u64,
    mix: T::MixCallback,
) {
    let pending = child.is_some_and(|c| transition.child_audio_pending(c));
    let duplicated = child.is_some_and(|c| transition.child_audio_is_duplicated(c));
    let valid = transition_render::audio_valid(child.is_some(), pending, duplicated);
    let Some(child) = child.filter(|_| valid) else {
        return;
    };

    let ts = transition.audio_timestamp(child);
    let place = transition_render::audio_place(ts, min_ts, request.sample_rate);
    if place == Place::None {
        return;
    }
    let child_audio = transition.audio_mix(child);

    let Place::Mix(pos) = place else {
        // PLACE_LATE
        return;
    };

    let mut bit = 1u32;
    for mix_idx in 0..MAX_AUDIO_MIXES {
        let selected = request.mixers & bit != 0;
        bit = bit.wrapping_shl(1);
        if !selected {
            continue;
        }

        for ch in 0..request.channels {
            let output = transition.output_samples(request.audio, mix_idx, ch);
            let input = transition.input_samples(&child_audio, mix_idx, ch);

            mix_child(
                transition,
                clock,
                output,
                pos,
                input,
                AUDIO_FRAMES.saturating_sub(pos),
                request.sample_rate,
                ts,
                mix,
            );
        }
    }
}

/// `transition_live_sources()`: the children in the slots, leaving out any
/// that was removed.
fn live_sources<T: Transition>(transition: &T) -> [Option<T::Child>; 2] {
    SLOTS.map(|idx| {
        transition
            .source(idx)
            .filter(|&child| !transition.removed(child))
    })
}

/// `transition_calc_min_ts()`: the earliest audio timestamp of the
/// children whose audio is not pending, 0 for none.
fn calc_min_ts<T: Transition>(transition: &T, sources: [Option<T::Child>; 2]) -> u64 {
    let timestamps = sources.map(|source| match source {
        Some(child) if !transition.child_audio_pending(child) => transition.audio_timestamp(child),
        _ => 0,
    });

    transition_clock::min_ts(timestamps)
}

/// `obs_transition_audio_render()`: `None` for no transition or a source
/// that is not one (nothing is read or written, and the C returns false
/// with `*ts_out` unwritten); else the earliest audio timestamp of the
/// children, which the C writes to `*ts_out` and returns as `ts != 0`.
pub fn audio_render<T: Transition>(transition: Option<&T>, request: Request<T>) -> Option<u64> {
    if !T::valid(transition, AUDIO_RENDER_LOG_NAME) {
        return None;
    }
    let transition = transition?;
    let mut stopped = false;
    let mut t = 0.0f32;
    let mut state = None;

    transition.lock();

    let mut sources = live_sources(transition);
    let mut min_ts = calc_min_ts(transition, sources);

    if min_ts != 0 {
        t = transition_stop::calc_time(transition, min_ts);
    }

    let step = transition_render::audio_stop(
        (min_ts != 0).then_some(t),
        transition.transitioning_video(),
        transition.transitioning_audio(),
    );
    if step != AudioStep::Keep {
        transition.set_transitioning_audio(false);
        if step == AudioStep::Stop {
            transition_stop::stop(transition);
            stopped = true;
        }
    }

    if min_ts != 0 {
        // The audio stop can make a removed child source A (#2185).
        sources = live_sources(transition);
        min_ts = calc_min_ts(transition, sources);
        if min_ts != 0 {
            let [a, b] = sources;
            let sources = [transition.get_ref(a), transition.get_ref(b)];
            let _transitioning_video = transition.transitioning_video();
            let transitioning_audio = transition.transitioning_audio();
            let clock = Clock {
                mode: transition.mode(),
                manual_val: transition.manual_val(),
                start_time: transition.start_time(),
                duration: transition.duration(),
            };
            state = Some(Copied {
                sources,
                transitioning_audio,
                clock,
            });
        }
    }

    transition.unlock();

    if let Some(state) = state {
        let [a, b] = state.sources;
        let mix = transition_render::audio_mix(state.transitioning_audio, a.is_some(), b.is_some());
        if mix.mix_a {
            process_audio(transition, state.clock, a, &request, min_ts, request.mix_a);
        }
        if mix.mix_b {
            process_audio(transition, state.clock, b, &request, min_ts, request.mix_b);
        }
        if mix.copy_a
            && let Some(a) = a
        {
            transition.copy_audio(request.audio, transition.child_audio_output_buf(a));
        }

        transition.release(a);
        transition.release(b);
    }

    if stopped {
        transition_stop::handle_stop(transition);
    }

    Some(min_ts)
}
