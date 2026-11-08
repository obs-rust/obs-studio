//! Safe core of `libobs/obs-audio.c.h`: the audio tick, `audio_callback()`,
//! which the audio output calls once per tick of
//! [`AUDIO_OUTPUT_FRAMES`] frames.
//!
//! One tick ([`audio_callback`]):
//! 1. builds the render order and the root nodes: each view's active,
//!    unremoved channel sources (a view shared by several mixes once), the
//!    tree under each ([`push_tree2`], which tags an individual audio
//!    source reached twice and mixes it as a root node), and every
//!    unremoved audio source, each taken with a reference;
//! 2. renders each source in that order, silences a monitored source when
//!    the deduplication source records the monitoring device, and drops the
//!    audio of a source that has gone backward in time;
//! 3. finds the minimum timestamp of the audio sources, marking the short
//!    ones pending, and adds buffering when it is before the tick (or fills
//!    the fixed buffering);
//! 4. mixes the root nodes into the output, unless it is still waiting out
//!    buffering ticks;
//! 5. discards the tick from every audio source's input, releases the
//!    render order, and counts down a wait tick or runs the queued audio
//!    tasks.
//!
//! The decisions are the cores of [`crate::audio_render`],
//! [`crate::audio_min_ts`], [`crate::audio_mix`] and
//! [`crate::audio_discard`]; this module is the order of the steps, the
//! locks around them and the log lines. Every field, lock, queue and libobs
//! call is one [`Tick`] method, called where and under the condition the C
//! has it.
//!
//! One intentional difference: the C checks a root source's
//! `audio_pending` before it takes that source's `audio_buf_mutex`, while
//! every other read and write of the flag is under that lock. Here the mix
//! takes the lock first and checks the flag under it, so another thread
//! can't change it between the check and the mix.

#![deny(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::unreachable
)]

use crate::audio_min_ts::{self, BufferingChoice};
use crate::audio_mix::{AUDIO_OUTPUT_FRAMES, Step, TsInfo};
use crate::audio_render::{
    BackwardsChoice, MonitorDedup, TreeChoice, backwards, clear_mixes, silence, tree,
};

/// `MAX_CHANNELS`: the source channels of a view (not audio channels).
pub const MAX_CHANNELS: u32 = 64;

/// The bytes of one channel of one tick: `AUDIO_OUTPUT_FRAMES *
/// sizeof(float)`, the size `obs_source_audio_render()` is handed.
pub const AUDIO_SIZE: usize = AUDIO_OUTPUT_FRAMES * 4;

/// What `obs_audio_discard_ignore()` reports to `ignore_audio`: the fields
/// of `struct obs_audio_ignore_step` the C reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ignored {
    /// The source is rendered again.
    pub rerender: bool,
    /// The source had no timestamp.
    pub no_timestamp: bool,
    /// Bytes buffered in channel 0 before the drop.
    pub buffered: usize,
    /// The lag left after the drop, in ns.
    pub lag: u64,
    /// The "lagging" warning is logged.
    pub warn: bool,
    /// The lag it names, in ns.
    pub warn_lag: u64,
}

/// The core, the sources and libobs as the tick sees them: one method per
/// field, lock, container call and libobs call of `obs-audio.c.h`.
///
/// None checks its arguments or takes a lock of its own: the tick takes the
/// locks the C takes, in its order (`mixes_mutex`, then a view's
/// `channels_mutex`; `audio_sources_mutex`, then a source's
/// `audio_buf_mutex`; `task_mutex` around the queue only).
pub trait Tick {
    /// An `obs_source_t *`, never NULL.
    type Source: Copy + PartialEq;
    /// An `obs_view *`, never NULL.
    type View: Copy + PartialEq;
    /// What `obs_source_get_name()` gives.
    type Name: Copy;
    /// A `struct obs_task_info` popped from the queue.
    type Task;

    /// `audio_output_get_sample_rate(audio->audio)`.
    fn sample_rate(&mut self) -> usize;
    /// `audio_output_get_channels(audio->audio)`.
    fn channels(&mut self) -> usize;

    /// `audio->render_order.num`.
    fn render_order_len(&mut self) -> usize;
    /// `audio->render_order.array[idx]`, `idx` below the length.
    fn render_order_get(&mut self, idx: usize) -> Self::Source;
    /// `da_find(audio->render_order, &source, 0)`, `None` for
    /// `DARRAY_INVALID`.
    fn render_order_find(&mut self, source: Self::Source) -> Option<usize>;
    /// `da_push_back(audio->render_order, &source)`.
    fn render_order_push(&mut self, source: Self::Source);
    /// `da_resize(audio->render_order, 0)`.
    fn render_order_clear(&mut self);
    /// `audio->root_nodes.num`.
    fn root_nodes_len(&mut self) -> usize;
    /// `audio->root_nodes.array[idx]`, `idx` below the length.
    fn root_nodes_get(&mut self, idx: usize) -> Self::Source;
    /// `da_push_back(audio->root_nodes, &source)`.
    fn root_nodes_push(&mut self, source: Self::Source);
    /// `da_resize(audio->root_nodes, 0)`.
    fn root_nodes_clear(&mut self);
    /// `deque_push_back(&audio->buffered_timestamps, &ts, sizeof(ts))`.
    fn timestamps_push_back(&mut self, ts: TsInfo);
    /// `deque_peek_front(&audio->buffered_timestamps, &ts, sizeof(ts))`.
    fn timestamps_peek_front(&mut self) -> TsInfo;
    /// `deque_pop_front(&audio->buffered_timestamps, NULL, sizeof(ts))`.
    fn timestamps_pop_front(&mut self);

    /// `obs_audio_buffering_maxed()` on the core's buffering fields.
    fn buffering_maxed(&mut self) -> bool;
    /// `obs_audio_buffering_fixed()` on the core's buffering fields.
    fn buffering_fixed(&mut self, sample_rate: usize, ts: &mut TsInfo) -> Step;
    /// `obs_audio_buffering_add()` on the core's buffering fields.
    fn buffering_add(&mut self, sample_rate: usize, ts: &mut TsInfo, min_ts: u64) -> Step;
    /// `audio->fixed_buffer`.
    fn fixed_buffer(&mut self) -> bool;
    /// `audio->buffering_wait_ticks`.
    fn wait_ticks(&mut self) -> u64;
    /// `audio->buffering_wait_ticks = value`.
    fn set_wait_ticks(&mut self, value: u64);
    /// `*out_ts = value`: the tick's start, for the audio output.
    fn set_out_ts(&mut self, value: u64);
    /// `audio->monitoring_duplicating_source`.
    fn monitoring_duplicating_source(&mut self) -> Option<Self::Source>;

    /// `pthread_mutex_lock(&audio->task_mutex)`.
    fn lock_tasks(&mut self);
    /// `pthread_mutex_unlock(&audio->task_mutex)`.
    fn unlock_tasks(&mut self);
    /// `audio->tasks.size`, in bytes.
    fn tasks_size(&mut self) -> usize;
    /// `deque_pop_front(&audio->tasks, &info, sizeof(info))`.
    fn pop_task(&mut self) -> Self::Task;
    /// `info.task(info.param)`.
    fn run_task(&mut self, task: Self::Task);

    /// `obs->data.first_audio_source`.
    fn first_audio_source(&mut self) -> Option<Self::Source>;
    /// `pthread_mutex_lock(&obs->data.audio_sources_mutex)`.
    fn lock_audio_sources(&mut self);
    /// `pthread_mutex_unlock(&obs->data.audio_sources_mutex)`.
    fn unlock_audio_sources(&mut self);
    /// `pthread_mutex_lock(&obs->video.mixes_mutex)`.
    fn lock_mixes(&mut self);
    /// `pthread_mutex_unlock(&obs->video.mixes_mutex)`.
    fn unlock_mixes(&mut self);
    /// `obs->video.mixes.num`.
    fn mixes_len(&mut self) -> usize;
    /// `obs->video.mixes.array[idx]->view`.
    fn mix_view(&mut self, idx: usize) -> Option<Self::View>;
    /// `obs->video.mixes.array[idx]->mix_audio`.
    fn mix_audio(&mut self, idx: usize) -> bool;
    /// `pthread_mutex_lock(&view->channels_mutex)`.
    fn lock_channels(&mut self, view: Self::View);
    /// `pthread_mutex_unlock(&view->channels_mutex)`.
    fn unlock_channels(&mut self, view: Self::View);
    /// `view->channels[idx]`, `idx` below [`MAX_CHANNELS`].
    fn channel(&mut self, view: Self::View, idx: u32) -> Option<Self::Source>;

    /// `source->next_audio_source`.
    fn next_audio_source(&mut self, source: Self::Source) -> Option<Self::Source>;
    /// `pthread_mutex_lock(&source->audio_buf_mutex)`.
    fn lock_buf(&mut self, source: Self::Source);
    /// `pthread_mutex_unlock(&source->audio_buf_mutex)`.
    fn unlock_buf(&mut self, source: Self::Source);
    /// `source->audio_is_duplicated`.
    fn is_duplicated(&mut self, source: Self::Source) -> bool;
    /// `source->audio_is_duplicated = value`.
    fn set_is_duplicated(&mut self, source: Self::Source, value: bool);
    /// `source->audio_pending`.
    fn pending(&mut self, source: Self::Source) -> bool;
    /// `source->audio_pending = value`.
    fn set_pending(&mut self, source: Self::Source, value: bool);
    /// `source->audio_ts`.
    fn audio_ts(&mut self, source: Self::Source) -> u64;
    /// `source->audio_input_buf[0].size`.
    fn input_size(&mut self, source: Self::Source) -> usize;
    /// `source->info.type`.
    fn kind(&mut self, source: Self::Source) -> u32;
    /// `source->info.output_flags`.
    fn output_flags(&mut self, source: Self::Source) -> u32;
    /// `source->info.audio_render != NULL`.
    fn has_audio_render(&mut self, source: Self::Source) -> bool;
    /// `source->audio_output_buf[0][0] != NULL`.
    fn has_output_buf(&mut self, source: Self::Source) -> bool;
    /// `source->muted`.
    fn muted(&mut self, source: Self::Source) -> bool;
    /// `source->volume`.
    fn volume(&mut self, source: Self::Source) -> f32;
    /// `source->audio_mixers`.
    fn audio_mixers(&mut self, source: Self::Source) -> u32;
    /// `os_atomic_load_bool(&source->monitoring_enabled)`.
    fn monitoring_enabled(&mut self, source: Self::Source) -> bool;
    /// `clear_audio_output_buf`'s loop: zero [`AUDIO_OUTPUT_FRAMES`] floats
    /// of every non-NULL `audio_output_buf[mix][ch]` of each mix in `mixes`.
    fn clear_output(&mut self, source: Self::Source, mixes: u32);
    /// `obs_audio_discard_ignore()` on the source's input.
    fn ignore(
        &mut self,
        source: Self::Source,
        channels: usize,
        sample_rate: usize,
        start_ts: u64,
    ) -> Ignored;
    /// `obs_audio_discard_tick()` on the source's input.
    fn discard_tick(
        &mut self,
        source: Self::Source,
        audio_render: bool,
        channels: usize,
        sample_rate: usize,
        ts: TsInfo,
    );
    /// `obs_audio_mix_add_mixers()` of the source's output into the
    /// tick's mixes.
    fn mix(
        &mut self,
        source: Self::Source,
        channels: usize,
        sample_rate: usize,
        ts: TsInfo,
        mixers: u32,
    );

    /// `obs_source_get_ref(source)`, `None` for NULL.
    fn get_ref(&mut self, source: Self::Source) -> Option<Self::Source>;
    /// `obs_source_release(source)`.
    fn release(&mut self, source: Self::Source);
    /// `obs_source_get_name(source)`.
    fn name(&mut self, source: Self::Source) -> Self::Name;
    /// `obs_source_removed(source)`.
    fn removed(&mut self, source: Self::Source) -> bool;
    /// `obs_source_active(source)`.
    fn active(&mut self, source: Self::Source) -> bool;
    /// `obs_source_audio_render(source, mixers, channels, sample_rate,
    /// size)`. It runs plugin code: the tick holds no lock across it.
    fn audio_render(
        &mut self,
        source: Self::Source,
        mixers: u32,
        channels: usize,
        sample_rate: usize,
        size: usize,
    );
    /// `obs_source_enum_active_tree(source, push_audio_tree2, audio)`: the
    /// implementation calls [`push_tree2`] on itself for each source of the
    /// tree, in libobs's order.
    fn enum_active_tree(&mut self, source: Self::Source);

    /// `blog(LOG_DEBUG, "render audio source %s timestamp has gone
    /// backwards", name)`.
    fn log_backwards(&mut self, name: Self::Name);
    /// `blog(LOG_DEBUG, "[src: %s] ts lag after ignoring: %" PRIu64, name,
    /// lag)`.
    fn log_ts_lag(&mut self, name: Self::Name, lag: u64);
    /// `blog(LOG_WARNING, "Source %s audio is lagging (over by %.02f ms) at
    /// max audio buffering. Restarting source audio.", name, (double)lag /
    /// 1000000.)`.
    fn log_lagging(&mut self, name: Self::Name, lag: u64);
    /// `blog(LOG_INFO, "Enabling fixed audio buffering, total audio
    /// buffering is now %d milliseconds", total_ms)`.
    fn log_fixed(&mut self, total_ms: i32);
    /// `blog(LOG_WARNING, "Max audio buffering reached!")`.
    fn log_max_reached(&mut self);
    /// `blog(LOG_INFO, "adding %d milliseconds of audio buffering, total
    /// audio buffering is now %d milliseconds (source: %s)\n", ms,
    /// total_ms, name)`, with NULL for `None`.
    fn log_adding(&mut self, ms: i32, total_ms: i32, name: Option<Self::Name>);
}

/// `push_audio_tree`: a source not yet in the render order is added, with a
/// reference, and its duplicate tag reset.
fn push_tree<T: Tick + ?Sized>(t: &mut T, source: T::Source) {
    if t.render_order_find(source).is_none()
        && let Some(s) = t.get_ref(source)
    {
        t.render_order_push(s);
        t.set_is_duplicated(s, false);
    }
}

/// `push_audio_tree2`: one source of a channel's active tree. A source not
/// yet in the render order is added; an individual audio source reached
/// again is tagged and mixed as a root node (the choice is
/// [`crate::audio_render::tree`]).
pub fn push_tree2<T: Tick + ?Sized>(t: &mut T, source: T::Source) {
    let removed = t.removed(source);
    let idx = if removed {
        None
    } else {
        t.render_order_find(source)
    };
    let present = idx.is_some();
    let s = match idx {
        Some(idx) => t.render_order_get(idx),
        None => source,
    };
    let kind = t.kind(s);
    let output_flags = t.output_flags(s);
    let duplicated = t.is_duplicated(s);

    match tree(removed, present, kind, output_flags, duplicated) {
        TreeChoice::Add => {
            if let Some(s) = t.get_ref(source) {
                t.render_order_push(s);
                t.set_is_duplicated(s, false);
            }
        }
        TreeChoice::Duplicate => {
            t.root_nodes_push(source);
            t.set_is_duplicated(s, true);
        }
        TreeChoice::Skip | TreeChoice::None => {}
    }
}

/// `ignore_audio`: drops the audio of a source that has gone backward in
/// time, under its buffer lock; whether it is rendered again.
fn ignore_audio<T: Tick + ?Sized>(
    t: &mut T,
    source: T::Source,
    channels: usize,
    sample_rate: usize,
    start_ts: u64,
) -> bool {
    let name = t.name(source);
    let step = t.ignore(source, channels, sample_rate, start_ts);

    if step.no_timestamp {
        return false;
    }
    if step.buffered != 0 {
        t.log_ts_lag(name, step.lag);
    }
    if step.warn {
        t.log_lagging(name, step.warn_lag);
    }
    step.rerender
}

/// `set_fixed_audio_buffering`.
fn set_fixed_buffering<T: Tick + ?Sized>(t: &mut T, sample_rate: usize, ts: &mut TsInfo) {
    let step = t.buffering_fixed(sample_rate, ts);
    if !step.added {
        return;
    }
    t.log_fixed(c_int(step.total_ms));
}

/// `add_audio_buffering`.
fn add_buffering<T: Tick + ?Sized>(
    t: &mut T,
    sample_rate: usize,
    ts: &mut TsInfo,
    min_ts: u64,
    buffering_name: Option<T::Name>,
) {
    let step = t.buffering_add(sample_rate, ts, min_ts);
    if !step.added {
        return;
    }
    if step.max_reached {
        t.log_max_reached();
    }
    t.log_adding(c_int(step.ms), c_int(step.total_ms), buffering_name);
}

/// The C's `(int)` cast of a `size_t`: the low 32 bits.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
fn c_int(value: usize) -> i32 {
    value as i32
}

/// `audio_buffer_insufficient`: whether a source is short for a tick at
/// `min_ts` (marking it pending), under its buffer lock.
fn buffer_insufficient<T: Tick + ?Sized>(
    t: &mut T,
    source: T::Source,
    sample_rate: usize,
    min_ts: u64,
) -> bool {
    t.lock_buf(source);
    let audio_render = t.has_audio_render(source);
    let mut pending = t.pending(source);
    let audio_ts = t.audio_ts(source);
    let size = t.input_size(source);
    let insufficient = audio_min_ts::insufficient(
        audio_render,
        &mut pending,
        audio_ts,
        size,
        sample_rate,
        min_ts,
    );
    if insufficient {
        t.set_pending(source, pending);
    }
    t.unlock_buf(source);
    insufficient
}

/// `find_min_ts`: lowers `min_ts` to the earliest audio source; the name of
/// the last source that lowered it.
fn find_min_ts<T: Tick + ?Sized>(t: &mut T, min_ts: &mut u64) -> Option<T::Name> {
    let mut buffering_source = None;
    let mut source = t.first_audio_source();
    while let Some(s) = source {
        t.lock_buf(s);
        let pending = t.pending(s);
        let audio_ts = t.audio_ts(s);
        if audio_min_ts::earlier(pending, audio_ts, min_ts) {
            buffering_source = Some(s);
        }
        t.unlock_buf(s);

        source = t.next_audio_source(s);
    }
    buffering_source.map(|s| t.name(s))
}

/// `mark_invalid_sources`: marks the short sources pending; whether any
/// was.
fn mark_invalid_sources<T: Tick + ?Sized>(t: &mut T, sample_rate: usize, min_ts: u64) -> bool {
    let mut recalculate = false;
    let mut source = t.first_audio_source();
    while let Some(s) = source {
        recalculate |= buffer_insufficient(t, s, sample_rate, min_ts);
        source = t.next_audio_source(s);
    }
    recalculate
}

/// `calc_min_ts`, under `audio_sources_mutex`.
fn calc_min_ts<T: Tick + ?Sized>(
    t: &mut T,
    sample_rate: usize,
    min_ts: &mut u64,
) -> Option<T::Name> {
    let mut buffering_name = find_min_ts(t, min_ts);
    if mark_invalid_sources(t, sample_rate, *min_ts) {
        buffering_name = find_min_ts(t, min_ts);
    }
    buffering_name
}

/// `release_audio_sources`.
fn release_audio_sources<T: Tick + ?Sized>(t: &mut T) {
    let mut i = 0;
    while i < t.render_order_len() {
        let s = t.render_order_get(i);
        t.release(s);
        i = i.wrapping_add(1);
    }
}

/// `execute_audio_tasks`: `task_mutex` is let go around each task, so a
/// task that queues another sees it run.
fn execute_audio_tasks<T: Tick + ?Sized>(t: &mut T) {
    t.lock_tasks();
    while t.tasks_size() != 0 {
        let task = t.pop_task();
        t.unlock_tasks();
        t.run_task(task);
        t.lock_tasks();
    }
    t.unlock_tasks();
}

/// `monitor_dedup`: the deduplication source's fields.
fn monitor_dedup<T: Tick + ?Sized>(t: &mut T) -> MonitorDedup {
    let dup_src = t.monitoring_duplicating_source();
    let mut dedup = MonitorDedup {
        set: dup_src.is_some(),
        ..MonitorDedup::default()
    };
    if let Some(d) = dup_src {
        dedup.active = t.active(d);
        dedup.muted = t.muted(d);
        dedup.volume = t.volume(d);
        dedup.mixers = t.audio_mixers(d);
    }
    dedup
}

/// `should_silence_monitored_source`.
fn should_silence_monitored_source<T: Tick + ?Sized>(t: &mut T, source: T::Source) -> bool {
    let dedup = monitor_dedup(t);
    let monitoring_enabled = t.monitoring_enabled(source);
    let is_dedup_source = t.monitoring_duplicating_source() == Some(source);
    silence(&dedup, monitoring_enabled, is_dedup_source)
}

/// `clear_audio_output_buf`.
fn clear_audio_output_buf<T: Tick + ?Sized>(t: &mut T, source: T::Source) {
    let Some(dup_src) = t.monitoring_duplicating_source() else {
        return;
    };
    let dup_mixers = t.audio_mixers(dup_src);
    let source_mixers = t.audio_mixers(source);
    t.clear_output(source, clear_mixes(dup_mixers, source_mixers));
}

/// The render order and root nodes over the views of the mixes, under
/// `mixes_mutex` and each view's `channels_mutex`.
fn build_from_views<T: Tick + ?Sized>(t: &mut T) {
    t.lock_mixes();
    let mut j = 0;
    while j < t.mixes_len() {
        let view = t.mix_view(j);
        let mut duplicate_view = false;
        let mut k = 0;
        while k < j {
            if t.mix_view(k) == view {
                duplicate_view = true;
                break;
            }
            k = k.wrapping_add(1);
        }
        if let Some(view) = view
            && !duplicate_view
        {
            t.lock_channels(view);
            for i in 0..MAX_CHANNELS {
                let Some(channel_source) = t.channel(view, i) else {
                    continue;
                };
                if !t.active(channel_source) {
                    continue;
                }
                if t.removed(channel_source) {
                    continue;
                }

                if t.mix_audio(j) {
                    t.root_nodes_push(channel_source);
                }
                t.enum_active_tree(channel_source);
                push_tree(t, channel_source);
            }
            t.unlock_channels(view);
        }
        j = j.wrapping_add(1);
    }
    t.unlock_mixes();
}

/// The render pass: each source of the render order is rendered, silenced
/// when monitored, and its audio dropped when it has gone backward in time.
fn render<T: Tick + ?Sized>(
    t: &mut T,
    mixers: u32,
    channels: usize,
    sample_rate: usize,
    ts: TsInfo,
) {
    let mut i = 0;
    while i < t.render_order_len() {
        let render_source = t.render_order_get(i);
        t.audio_render(render_source, mixers, channels, sample_rate, AUDIO_SIZE);
        if should_silence_monitored_source(t, render_source) {
            clear_audio_output_buf(t, render_source);
        }

        let mut rerender = false;
        t.lock_buf(render_source);
        let maxed = t.buffering_maxed();
        let audio_ts = t.audio_ts(render_source);
        let audio_render = t.has_audio_render(render_source);
        match backwards(maxed, audio_ts, ts.start, audio_render) {
            BackwardsChoice::Pending => {
                let name = t.name(render_source);
                t.log_backwards(name);
                t.set_pending(render_source, true);
            }
            BackwardsChoice::Ignore => {
                rerender = ignore_audio(t, render_source, channels, sample_rate, ts.start);
            }
            BackwardsChoice::None => {}
        }
        t.unlock_buf(render_source);

        /* Rendering can call plugins; the buffer lock is let go first. */
        if rerender {
            t.audio_render(render_source, mixers, channels, sample_rate, AUDIO_SIZE);
        }
        i = i.wrapping_add(1);
    }
}

/// The mix of the root nodes. A root source's `audio_pending` is read
/// under its buffer lock (the C reads it before taking the lock).
fn mix_roots<T: Tick + ?Sized>(
    t: &mut T,
    mixers: u32,
    channels: usize,
    sample_rate: usize,
    ts: TsInfo,
) {
    let mut i = 0;
    while i < t.root_nodes_len() {
        let root_source = t.root_nodes_get(i);

        t.lock_buf(root_source);
        if !t.pending(root_source) && t.has_output_buf(root_source) && t.audio_ts(root_source) != 0
        {
            t.mix(root_source, channels, sample_rate, ts, mixers);
        }
        t.unlock_buf(root_source);
        i = i.wrapping_add(1);
    }
}

/// `audio_callback()`: one tick from `start_ts` to `end_ts`, mixed into the
/// mixes in `mixers`. It sets `*out_ts` ([`Tick::set_out_ts`]) before it
/// counts a wait tick down or runs the tasks, as the C does. Returns the
/// result and the `*out_ts` it set.
pub fn audio_callback<T: Tick>(t: &mut T, start_ts: u64, end_ts: u64, mixers: u32) -> (bool, u64) {
    let sample_rate = t.sample_rate();
    let channels = t.channels();
    let mut ts = TsInfo {
        start: start_ts,
        end: end_ts,
    };

    t.render_order_clear();
    t.root_nodes_clear();

    t.timestamps_push_back(ts);
    ts = t.timestamps_peek_front();
    let mut min_ts = ts.start;

    /* build audio render order */
    build_from_views(t);

    t.lock_audio_sources();
    let mut source = t.first_audio_source();
    while let Some(s) = source {
        if !t.removed(s) {
            push_tree(t, s);
        }
        source = t.next_audio_source(s);
    }
    t.unlock_audio_sources();

    /* render audio data */
    render(t, mixers, channels, sample_rate, ts);

    /* get minimum audio timestamp */
    t.lock_audio_sources();
    let buffering_name = calc_min_ts(t, sample_rate, &mut min_ts);
    t.unlock_audio_sources();

    /* if a source has gone backward in time, buffer */
    let fixed_buffer = t.fixed_buffer();
    let maxed = t.fixed_buffer() && t.buffering_maxed();
    match audio_min_ts::buffering(fixed_buffer, maxed, min_ts, ts.start) {
        BufferingChoice::Fixed => set_fixed_buffering(t, sample_rate, &mut ts),
        BufferingChoice::Add => add_buffering(t, sample_rate, &mut ts, min_ts, buffering_name),
        BufferingChoice::None => {}
    }

    /* mix audio */
    if t.wait_ticks() == 0 {
        mix_roots(t, mixers, channels, sample_rate, ts);
    }

    /* discard audio */
    t.lock_audio_sources();
    let mut source = t.first_audio_source();
    while let Some(s) = source {
        t.lock_buf(s);
        let audio_render = t.has_audio_render(s);
        t.discard_tick(s, audio_render, channels, sample_rate, ts);
        t.unlock_buf(s);

        source = t.next_audio_source(s);
    }
    t.unlock_audio_sources();

    /* release audio sources */
    release_audio_sources(t);

    t.timestamps_pop_front();

    let out_ts = ts.start;
    t.set_out_ts(out_ts);

    let wait = t.wait_ticks();
    if wait != 0 {
        t.set_wait_ticks(wait.wrapping_sub(1));
        return (false, out_ts);
    }

    execute_audio_tasks(t);
    (true, out_ts)
}
