/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#pragma once

#include <stdbool.h>
#include <stdint.h>

/* The audio timestamp placement of source_output_audio_data() in
 * obs-source.c, on plain values (obs-source-audio-timing.c). It decides
 * where a source's audio packet goes; obs-source.c keeps everything else.
 *
 * source_output_audio_data() reads the source's fields into a struct, calls
 * a step and writes the results back itself, so the stores of the volatile
 * timing_set and timing_adjust (the async video path reads and writes them
 * too) stay where they were and in the same order:
 * - obs_source_audio_timing_pre_lock(), before audio_buf_mutex: the direct
 *   timestamp check, then the choice between resetting the timing, a
 *   timestamp jump (handle_ts_jump() stays in obs-source.c: it takes the
 *   lock and empties the buffers), smoothing onto the expected timestamp
 *   and logging; the drift error for the resampler's compensation
 *   (audio_resampler_set_compensation_error() stays in obs-source.c) and
 *   next_audio_ts_min. The caller computes the packet's duration
 *   (conv_frames_to_time()), so no division by the sample rate is here.
 * - obs_source_audio_timing_under_lock(), under audio_buf_mutex: whether
 *   the packet is appended to the buffer or placed by its timestamp, the
 *   reset when its system time jumped (unless the audio was continuous),
 *   the sync and resampler offsets, next_audio_sys_ts_min, and the "sync
 *   offset changed, so place" rule. The deque place and push-back stay in
 *   obs-source.c.
 * - obs_source_audio_timing_buffer_start(): where
 *   reset_audio_data_at_sys_time() starts an emptied buffer.
 *
 * None of them takes a lock, calls into libobs or keeps state. All
 * arithmetic is uint64_t and wraps, as in source_output_audio_data().
 *
 * Hidden in libobs: not EXPORT. */

/* in nanoseconds; obs-source.c checks that they are its TS_SMOOTHING_THRESHOLD
 * and obs-internal.h's MAX_TS_VAR */
#define OBS_SOURCE_AUDIO_TS_SMOOTHING_THRESHOLD 70000000ULL
#define OBS_SOURCE_AUDIO_MAX_TS_VAR 2000000000ULL

/* What source_output_audio_data() does with the packet's timestamp before
 * the lock. */
enum obs_source_audio_ts_action {
	/* timing set but no expected timestamp yet (next_audio_ts_min 0):
	 * keep the packet's */
	OBS_SOURCE_AUDIO_TS_KEEP,
	/* timing not set: reset_audio_timing(), timing_set = true and
	 * timing_adjust = os_time - timestamp */
	OBS_SOURCE_AUDIO_TS_RESET,
	/* more than MAX_TS_VAR from the expected timestamp, not direct:
	 * handle_ts_jump() */
	OBS_SOURCE_AUDIO_TS_JUMP,
	/* within TS_SMOOTHING_THRESHOLD: continuous, placed on the expected
	 * timestamp */
	OBS_SOURCE_AUDIO_TS_SMOOTH,
	/* neither: log that it exceeded TS_SMOOTHING_THRESHOLD, keep the
	 * packet's */
	OBS_SOURCE_AUDIO_TS_EXCEEDED,
};

struct obs_source_audio_timing_pre {
	/* in: the packet's timestamp; out: its timestamp after smoothing, before
	 * timing_adjust */
	uint64_t timestamp;
	/* in: os_gettime_ns() for this packet */
	uint64_t os_time;
	/* in: conv_frames_to_time() of the packet's frames */
	uint64_t duration;
	/* in: source->next_audio_ts_min; out: the value to store, the packet's
	 * end */
	uint64_t next_audio_ts_min;
	/* out: the timing_adjust that RESET, JUMP (handle_ts_jump()) and the
	 * re-anchor store; 0 otherwise */
	uint64_t timing_adjust;
	/* out: JUMP and EXCEEDED: how far the timestamp is from the expected one;
	 * 0 otherwise */
	uint64_t diff;
	/* out: SMOOTH: how far the samples run ahead of the timestamps, for the
	 * resampler's compensation; 0 otherwise */
	int64_t drift_error;
	/* out */
	enum obs_source_audio_ts_action action;
	/* in: source->timing_set, source->async_unbuffered and
	 * source->async_decoupled */
	bool timing_set;
	bool async_unbuffered;
	bool async_decoupled;
	/* out: a timestamp on the system clock (within MAX_TS_VAR of os_time):
	 * store timing_adjust = 0, then timing_set = true, before the action */
	bool direct;
	/* out: SMOOTH of an unbuffered, decoupled async source: store
	 * timing_adjust */
	bool reanchor;
	/* out: SMOOTH: the packet continues the previous one */
	bool continuous;
};

struct obs_source_audio_timing_under {
	/* in: the timestamp from obs_source_audio_timing_pre_lock(); out: the
	 * system time the packet is placed at, with timing_adjust, the sync
	 * offset and the resampler offset */
	uint64_t timestamp;
	/* in: source->timing_adjust as read before the lock, for the packet */
	uint64_t packet_timing_adjust;
	/* in: the packet's own timestamp (data->timestamp) */
	uint64_t data_timestamp;
	/* in: as for obs_source_audio_timing_pre_lock() */
	uint64_t os_time;
	/* in: source->next_audio_ts_min, as stored after the pre-lock step */
	uint64_t next_audio_ts_min;
	/* in: source->resample_offset and source->sync_offset */
	uint64_t resample_offset;
	int64_t sync_offset;
	/* in: source->timing_adjust as read under the lock; out: when
	 * reset_timing, the value to store */
	uint64_t timing_adjust;
	/* in: source->next_audio_sys_ts_min; out: the value to store */
	uint64_t next_audio_sys_ts_min;
	/* in: source->last_sync_offset; out: the value to store */
	int64_t last_sync_offset;
	/* in: from obs_source_audio_timing_pre_lock() */
	bool continuous;
	/* out: append the packet to the buffer (when it has an audio_ts) instead
	 * of placing it by its timestamp */
	bool push_back;
	/* out: reset_audio_timing() to the packet's own timestamp: store
	 * timing_set = true, then timing_adjust */
	bool reset_timing;
};

void obs_source_audio_timing_pre_lock(struct obs_source_audio_timing_pre *t);
void obs_source_audio_timing_under_lock(struct obs_source_audio_timing_under *t);
uint64_t obs_source_audio_timing_buffer_start(uint64_t os_time, int64_t sync_offset, uint64_t resample_offset);
