//! Safe core of `libobs/obs-source-audio-output.c.h`: the path that puts one
//! audio packet of a source in its buffer, moved out of
//! `libobs/obs-source.c.h`.
//!
//! The decisions are [`audio_timing`](crate::audio_timing) and
//! [`audio_place`](crate::audio_place), on plain values. This module makes the
//! C's calls around them in its order and under its locks, generic over a
//! [`Source`]: one method per field read or write, lock, buffer operation and
//! call into the rest of libobs. The C shims live in the `obs-source-seam`
//! crate, which implements [`Source`] over
//! `libobs/obs-source-audio-output-seam.h`, so this crate calls nothing in
//! libobs. The fields stay in `struct obs_source`, and each read and write is
//! C, so ThreadSanitizer keeps seeing them. The packet's samples do not cross:
//! [`Packet::data`] is opaque here.
//!
//! What the C does and this keeps (characterized, not endorsed;
//! `test_source_audio_output` pins most of it):
//!
//! - [`output_audio_data`] has two `audio_buf_mutex` regions, with the jump's
//!   own region between them. The graphics thread can set a new
//!   `timing_adjust` in those gaps, and the second region reads the new value.
//! - `timing_adjust` is read twice in the second region, because it is
//!   volatile in the C: once for the packet, once as the value to keep.
//! - The first placed packet leaves `next_audio_sys_ts_min` at the packet's
//!   start, not at its end: [`reset_audio_data`] runs after the store.
//! - A packet that does not fit is dropped with no log, and
//!   `last_audio_input_buf_size` keeps its value then.
//! - The capture callback gets the packet's own timestamp, and runs with
//!   `audio_buf_mutex` released.
//!
//! The `DEBUG_AUDIO` log line of `source_output_audio_place()` is compiled out
//! on every C build and is not here.

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

use crate::audio_place;
use crate::audio_timing::{self, Action, PreLockInput, UnderLockInput};
use crate::own_clock_drift::{self, OwnClockDrift, OwnClockDriftRaw};

/// `MAX_AUDIO_CHANNELS`: the buffers [`reset_audio_data`] empties.
pub const MAX_CHANNELS: usize = 8;

const NSEC_PER_SEC: u64 = 1_000_000_000;

/// One packet, as `struct audio_data` holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packet<D> {
    /// The samples, opaque here.
    pub data: D,
    /// `data->timestamp`.
    pub timestamp: u64,
    /// `data->frames`.
    pub frames: u32,
}

/// The source audio output seam: one method per field read or write, lock,
/// buffer operation and libobs call. A lock method is one lock call, and none
/// of the others takes a lock of its own.
///
/// The caller holds `audio_buf_mutex` around every field except
/// `next_audio_ts_min`, `async_unbuffered`, `async_decoupled` and the
/// resampler's, and around every buffer operation.
pub trait Source {
    /// The samples of a packet, opaque to the core.
    type Data: Copy;

    /// `os_gettime_ns()`.
    fn now(&self) -> u64;
    /// `audio_output_get_sample_rate(obs->audio.audio)`.
    fn sample_rate(&self) -> u32;
    /// `audio_output_get_channels(obs->audio.audio)`.
    fn channels(&self) -> usize;

    /// `pthread_mutex_lock(&source->audio_buf_mutex)`.
    fn lock_audio_buf(&self);
    /// `pthread_mutex_unlock(&source->audio_buf_mutex)`.
    fn unlock_audio_buf(&self);

    /// `source->timing_set`.
    fn timing_set(&self) -> bool;
    /// `source->timing_set = value`.
    fn set_timing_set(&self, value: bool);
    /// `source->timing_adjust`.
    fn timing_adjust(&self) -> u64;
    /// `source->timing_adjust = value`.
    fn set_timing_adjust(&self, value: u64);
    /// `source->next_audio_ts_min`.
    fn next_audio_ts_min(&self) -> u64;
    /// `source->next_audio_ts_min = value`.
    fn set_next_audio_ts_min(&self, value: u64);
    /// `source->next_audio_sys_ts_min`.
    fn next_audio_sys_ts_min(&self) -> u64;
    /// `source->next_audio_sys_ts_min = value`.
    fn set_next_audio_sys_ts_min(&self, value: u64);
    /// `source->audio_ts`.
    fn audio_ts(&self) -> u64;
    /// `source->audio_ts = value`.
    fn set_audio_ts(&self, value: u64);
    /// `source->last_audio_input_buf_size = 0`.
    fn clear_last_buf_size(&self);
    /// `source->sync_offset`.
    fn sync_offset(&self) -> i64;
    /// `source->last_sync_offset`.
    fn last_sync_offset(&self) -> i64;
    /// `source->last_sync_offset = value`.
    fn set_last_sync_offset(&self, value: i64);
    /// `source->resample_offset`.
    fn resample_offset(&self) -> u64;
    /// `source->monitoring_type`.
    fn monitoring_type(&self) -> i32;
    /// `source->async_unbuffered`.
    fn async_unbuffered(&self) -> bool;
    /// `source->async_decoupled`.
    fn async_decoupled(&self) -> bool;
    /// `source->last_output_frame_ts`: the timestamp of the latest frame the
    /// source output, 0 for none. Read under `audio_buf_mutex` (#3747).
    fn last_output_frame_ts(&self) -> u64;
    /// `source->own_clock_timing_adjust`: the timing of audio on a clock of
    /// its own, which the video's frames do not move.
    fn own_clock_timing_adjust(&self) -> u64;
    /// `source->own_clock_timing_adjust = value`.
    fn set_own_clock_timing_adjust(&self, value: u64);
    /// `source->own_clock_timing_set`: whether `own_clock_timing_adjust` holds
    /// a timing yet.
    fn own_clock_timing_set(&self) -> bool;
    /// `source->own_clock_timing_set = value`.
    fn set_own_clock_timing_set(&self, value: bool);

    /// `source->own_clock_compensation`: whether libobs turned clock drift
    /// compensation on for audio on a clock of its own (#4182).
    fn own_clock_compensation(&self) -> bool;
    /// `source->own_clock_compensation = value`.
    fn set_own_clock_compensation(&self, value: bool);
    /// `source->own_clock_drift`: the drift error filter of audio on a clock
    /// of its own (#4182).
    fn own_clock_drift(&self) -> OwnClockDriftRaw;
    /// `source->own_clock_drift = value`.
    fn set_own_clock_drift(&self, value: OwnClockDriftRaw);

    /// `source->resampler != NULL && source->last_async_compensation`.
    fn compensating(&self) -> bool;
    /// `audio_resampler_set_compensation_error(source->resampler, error)`.
    fn set_compensation_error(&self, error: i64);

    /// `source->audio_input_buf[channel].size`.
    fn buf_size(&self, channel: usize) -> usize;
    /// `deque_pop_front(&source->audio_input_buf[channel], NULL, size)`.
    fn pop_front(&self, channel: usize, size: usize);
    /// `deque_pop_back(&source->audio_input_buf[channel], NULL, size)`.
    fn pop_back(&self, channel: usize, size: usize);
    /// `deque_place(&source->audio_input_buf[channel], placement,
    /// data->data[channel], size)`.
    fn place(&self, channel: usize, placement: usize, data: Self::Data, size: usize);
    /// `deque_push_back(&source->audio_input_buf[channel],
    /// data->data[channel], size)`.
    fn push_back(&self, channel: usize, data: Self::Data, size: usize);

    /// The debug log line of `handle_ts_jump()`.
    fn log_ts_jump(&self, diff: u64, expected: u64, timestamp: u64);
    /// The debug log line of a timestamp past the smoothing threshold.
    fn log_ts_exceeded(&self, diff: u64, expected: u64, timestamp: u64);

    /// `source_muted(source, os_time)`.
    fn muted(&self, os_time: u64) -> bool;
    /// `source_signal_audio_data(source, data, muted)`.
    fn signal_audio_data(&self, data: Self::Data, muted: bool);
}

/// `conv_frames_to_time()`: 0 for no sample rate. `frames` is 32 bits, so the
/// product fits.
#[must_use]
pub fn duration(sample_rate: u32, frames: u32) -> u64 {
    u64::from(frames)
        .wrapping_mul(NSEC_PER_SEC)
        .checked_div(u64::from(sample_rate))
        .unwrap_or(0)
}

/// `reset_audio_timing()`. The caller holds `audio_buf_mutex`.
pub fn reset_audio_timing<S: Source>(source: &S, timestamp: u64, os_time: u64) {
    source.set_timing_set(true);
    source.set_timing_adjust(os_time.wrapping_sub(timestamp));
}

/// `reset_audio_data()`. The caller holds `audio_buf_mutex`.
pub fn reset_audio_data<S: Source>(source: &S, os_time: u64) {
    for channel in 0..MAX_CHANNELS {
        if source.buf_size(channel) != 0 {
            source.pop_front(channel, source.buf_size(channel));
        }
    }
    source.clear_last_buf_size();
    source.set_audio_ts(os_time);
    source.set_next_audio_sys_ts_min(os_time);
}

/// `reset_audio_data_at_sys_time()`: empties the buffer for packets that
/// continue at system time `os_time`. The caller holds `audio_buf_mutex`.
pub fn reset_audio_data_at_sys_time<S: Source>(source: &S, os_time: u64) {
    reset_audio_data(source, os_time);
    source.set_audio_ts(audio_timing::buffer_start(
        os_time,
        source.sync_offset(),
        source.resample_offset(),
    ));
}

fn handle_ts_jump<S: Source>(source: &S, expected: u64, timestamp: u64, diff: u64, os_time: u64) {
    source.log_ts_jump(diff, expected, timestamp);

    source.lock_audio_buf();
    reset_audio_timing(source, timestamp, os_time);
    reset_audio_data_at_sys_time(source, os_time);
    source.unlock_audio_buf();
}

/// `source_output_audio_place()`. `timestamp` is the system time of the packet.
fn place<S: Source>(source: &S, data: S::Data, frames: u32, timestamp: u64) {
    let channels = source.channels();
    let size = audio_place::size(frames);

    if !audio_place::started(source.audio_ts()) || audio_place::older(timestamp, source.audio_ts())
    {
        reset_audio_data(source, timestamp);
    }

    let audio_ts = source.audio_ts();
    let placement = audio_place::offset(timestamp, audio_ts, source.sample_rate());

    // do not allow the circular buffers to become too big
    if !audio_place::fits(placement, size) {
        return;
    }

    for channel in 0..channels {
        source.place(channel, placement, data, size);
        source.pop_back(
            channel,
            audio_place::pop_back(source.buf_size(channel), placement, size),
        );
    }

    source.clear_last_buf_size();
}

/// `source_output_audio_push_back()`.
fn push_back<S: Source>(source: &S, data: S::Data, frames: u32) {
    let channels = source.channels();
    let size = audio_place::size(frames);

    // do not allow the circular buffers to become too big
    if !audio_place::fits(source.buf_size(0), size) {
        return;
    }

    for channel in 0..channels {
        source.push_back(channel, data, size);
    }

    // reset audio input buffer size to ensure that audio doesn't get
    // perpetually cut
    source.clear_last_buf_size();
}

/// [`own_clock_drift::buffered_end_gap`] of the source's buffer: how far the
/// end of its first channel is ahead of `os_time`. The caller holds
/// `audio_buf_mutex`.
fn buffered_end_gap<S: Source>(source: &S, sample_rate: u32, os_time: u64) -> i64 {
    let frames = source
        .buf_size(0)
        .checked_div(size_of::<f32>())
        .unwrap_or(0);
    own_clock_drift::buffered_end_gap(
        source.audio_ts(),
        u64::try_from(frames).unwrap_or(u64::MAX),
        sample_rate,
        os_time,
    )
}

/// The own timing of a packet on a clock of its own, before
/// `under_lock()`: the first such packet sets it from its arrival and turns
/// libobs's own clock drift compensation on (#4182); later, a buffer more
/// than `TS_SMOOTHING_THRESHOLD` over a placement at the arrival takes the
/// timing again. Returns whether the packet must be placed at its arrival.
/// The caller holds `audio_buf_mutex`.
fn own_clock_timing<S: Source>(source: &S, arrival: u64, sample_rate: u32, os_time: u64) -> bool {
    if !source.own_clock_timing_set() {
        // reset_audio_timing() for the audio's own timing
        source.set_own_clock_timing_adjust(arrival);
        source.set_own_clock_timing_set(true);
        // its clock drifts from the system clock: compensate it (#4182)
        source.set_own_clock_compensation(true);
        return false;
    }
    if audio_place::started(source.audio_ts())
        && own_clock_drift::excess(
            buffered_end_gap(source, sample_rate, os_time),
            source.sync_offset(),
            source.resample_offset(),
        ) > audio_timing::TS_SMOOTHING_THRESHOLD.cast_signed()
    {
        // The buffer holds more than TS_SMOOTHING_THRESHOLD more audio than
        // right after a packet placed at its arrival: a sender clock that
        // drifts faster than the compensation follows (#4143, #4182). Take
        // the timing of this packet again and place it there.
        source.set_own_clock_timing_adjust(arrival);
        return true;
    }
    false
}

/// The error of audio on a clock of its own after its packet went into the
/// buffer: the end of the buffered audio against the arrival, filtered
/// (#4182). A placed packet starts the filter again. The caller holds
/// `audio_buf_mutex`.
fn own_clock_error<S: Source>(
    source: &S,
    appended: bool,
    sample_rate: u32,
    os_time: u64,
) -> Option<i64> {
    let mut drift = OwnClockDrift::from_raw(&source.own_clock_drift());
    let error = if appended {
        drift.sample(os_time, buffered_end_gap(source, sample_rate, os_time))
    } else {
        drift.reset();
        None
    };
    source.set_own_clock_drift(drift.to_raw());
    error
}

/// The timing of the second `audio_buf_mutex` region: `under_lock()` on the
/// packet's timing (the video's or its own), with a packet on a clock of its
/// own that is placed instead of appended taken from its arrival (#4182), and
/// the stores. The caller holds `audio_buf_mutex`.
fn second_region_timing<S: Source>(
    source: &S,
    pre: &audio_timing::PreLock,
    timestamp: u64,
    os_time: u64,
    on_video_clock: bool,
    mut place_at_arrival: bool,
) -> audio_timing::UnderLock {
    let arrival = os_time.wrapping_sub(timestamp);
    let packet_timing_adjust = if on_video_clock {
        source.timing_adjust()
    } else {
        source.own_clock_timing_adjust()
    };

    let mut input = UnderLockInput {
        timestamp: pre.timestamp,
        packet_timing_adjust,
        data_timestamp: timestamp,
        os_time,
        next_audio_ts_min: source.next_audio_ts_min(),
        resample_offset: source.resample_offset(),
        sync_offset: source.sync_offset(),
        timing_adjust: if on_video_clock {
            source.timing_adjust()
        } else {
            source.own_clock_timing_adjust()
        },
        next_audio_sys_ts_min: source.next_audio_sys_ts_min(),
        last_sync_offset: source.last_sync_offset(),
        continuous: pre.continuous && on_video_clock,
    };
    let mut under = audio_timing::under_lock(input);

    // With compensation on, the sender's time runs away from the arrival. A
    // packet on a clock of its own that is placed instead of appended takes
    // its timing from its arrival, so it lands there (#4182).
    if !on_video_clock
        && !place_at_arrival
        && !under.push_back
        && !under.reset_timing
        && packet_timing_adjust != arrival
    {
        source.set_own_clock_timing_adjust(arrival);
        input.packet_timing_adjust = arrival;
        input.timing_adjust = arrival;
        under = audio_timing::under_lock(input);
        place_at_arrival = true;
    }
    if place_at_arrival {
        under.push_back = false;
    }

    if under.reset_timing {
        source.set_timing_set(true);
        if on_video_clock {
            source.set_timing_adjust(under.timing_adjust);
        } else {
            source.set_own_clock_timing_adjust(under.timing_adjust);
        }
    }
    source.set_next_audio_sys_ts_min(under.next_audio_sys_ts_min);
    source.set_last_sync_offset(under.last_sync_offset);

    under
}

/// Appends the packet or places it at `timestamp`, if the source outputs
/// its audio. Returns whether it was appended, or `None` for a source that
/// buffers nothing. The caller holds `audio_buf_mutex`.
fn buffer_packet<S: Source>(
    source: &S,
    packet: Packet<S::Data>,
    append: bool,
    timestamp: u64,
) -> Option<bool> {
    if !audio_place::outputs(source.monitoring_type()) {
        return None;
    }
    if append && audio_place::started(source.audio_ts()) {
        push_back(source, packet.data, packet.frames);
        Some(true)
    } else {
        place(source, packet.data, packet.frames, timestamp);
        Some(false)
    }
}

/// `source_output_audio_data()`. The caller holds `audio_mutex`.
pub fn output_audio_data<S: Source>(source: &S, packet: Packet<S::Data>) {
    let sample_rate = source.sample_rate();
    let os_time = source.now();

    source.lock_audio_buf();

    let pre = audio_timing::pre_lock(PreLockInput {
        timestamp: packet.timestamp,
        os_time,
        duration: duration(sample_rate, packet.frames),
        next_audio_ts_min: source.next_audio_ts_min(),
        timing_set: source.timing_set(),
        async_unbuffered: source.async_unbuffered(),
        async_decoupled: source.async_decoupled(),
    });

    if pre.direct {
        source.set_timing_adjust(0);
        source.set_timing_set(true);
    }

    if pre.action == Action::Reset {
        source.set_timing_set(true);
        source.set_timing_adjust(pre.timing_adjust);
    } else if pre.action == Action::Smooth && pre.reanchor {
        source.set_timing_adjust(pre.timing_adjust);
    }

    // libobs's own compensation of audio on a clock of its own (#4182)
    let mut own_compensation = source.own_clock_compensation();

    source.unlock_audio_buf();

    match pre.action {
        Action::Keep | Action::Reset | Action::Smooth => {}
        Action::Jump => handle_ts_jump(
            source,
            source.next_audio_ts_min(),
            packet.timestamp,
            pre.diff,
            os_time,
        ),
        Action::Exceeded => {
            source.log_ts_exceeded(pre.diff, source.next_audio_ts_min(), packet.timestamp);
        }
    }

    // Feed the error to the resampler so that the samples keep pace with the
    // timestamps instead of drifting until the threshold above is exceeded.
    // After a resync the error is unknown, so hold. Audio that libobs
    // compensates itself gets its error after it is buffered (#4182).
    if !own_compensation && source.compensating() {
        source.set_compensation_error(pre.drift_error);
    }

    source.set_next_audio_ts_min(pre.next_audio_ts_min);

    source.lock_audio_buf();

    // a continuous packet follows the video's timing only on the video's
    // clock (#3747); audio on a clock of its own keeps its own timing
    let on_video_clock =
        audio_timing::follows_video(packet.timestamp, source.last_output_frame_ts());
    let arrival = os_time.wrapping_sub(packet.timestamp);
    let place_at_arrival =
        !on_video_clock && own_clock_timing(source, arrival, sample_rate, os_time);
    own_compensation = own_compensation || !on_video_clock && source.own_clock_compensation();
    let under = second_region_timing(
        source,
        &pre,
        packet.timestamp,
        os_time,
        on_video_clock,
        place_at_arrival,
    );

    // the error of audio on a clock of its own (#4182)
    let own_error = buffer_packet(source, packet, under.push_back, under.timestamp)
        .filter(|_| !on_video_clock)
        .and_then(|appended| own_clock_error(source, appended, sample_rate, os_time));

    source.unlock_audio_buf();

    // until the filter has its baseline, 0: the loop holds
    if !on_video_clock && own_compensation && source.compensating() {
        source.set_compensation_error(own_error.unwrap_or(0));
    }

    let muted = source.muted(os_time);
    source.signal_audio_data(packet.data, muted);
}
