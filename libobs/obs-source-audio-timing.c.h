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

#include "libobs/build/libobs-ported-source-audio-timing.pch.h"

/* The audio timestamp placement of source_output_audio_data(), moved from
 * obs-source.c: the decisions, on plain values. obs-source.c reads the
 * source's fields into the structs, calls the steps and writes the results
 * back; it keeps the lock, handle_ts_jump(), the resampler's compensation,
 * the deque, the signal and the log lines. Declared in
 * obs-source-audio-timing.h without EXPORT. In hybrid libobs the Rust port
 * (rust/obs-source) replaces this file. */

static inline uint64_t audio_timing_diff(uint64_t ts1, uint64_t ts2)
{
	return (ts1 < ts2) ? (ts2 - ts1) : (ts1 - ts2);
}

void obs_source_audio_timing_pre_lock(struct obs_source_audio_timing_pre *t)
{
	uint64_t diff;
	bool using_direct_ts = false;
	bool timing_set = t->timing_set;

	t->action = OBS_SOURCE_AUDIO_TS_KEEP;
	t->timing_adjust = 0;
	t->diff = 0;
	t->drift_error = 0;
	t->direct = false;
	t->reanchor = false;
	t->continuous = false;

	/* detects 'directly' set timestamps as long as they're within
	 * a certain threshold */
	if (audio_timing_diff(t->timestamp, t->os_time) < OBS_SOURCE_AUDIO_MAX_TS_VAR) {
		/* timing_adjust = 0, timing_set = true */
		t->direct = true;
		timing_set = true;
		using_direct_ts = true;
	}

	if (!timing_set) {
		/* reset_audio_timing() */
		t->action = OBS_SOURCE_AUDIO_TS_RESET;
		t->timing_adjust = t->os_time - t->timestamp;

	} else if (t->next_audio_ts_min != 0) {
		diff = audio_timing_diff(t->next_audio_ts_min, t->timestamp);

		/* smooth audio if within threshold */
		if (diff > OBS_SOURCE_AUDIO_MAX_TS_VAR && !using_direct_ts) {
			/* handle_ts_jump(), which resets the timing to this
			 * timestamp */
			t->action = OBS_SOURCE_AUDIO_TS_JUMP;
			t->diff = diff;
			t->timing_adjust = t->os_time - t->timestamp;
		} else if (diff < OBS_SOURCE_AUDIO_TS_SMOOTHING_THRESHOLD) {
			t->action = OBS_SOURCE_AUDIO_TS_SMOOTH;
			if (t->async_unbuffered && t->async_decoupled) {
				t->reanchor = true;
				t->timing_adjust = t->os_time - t->timestamp;
			}
			/* how far the samples run ahead of the timestamps */
			t->drift_error = (int64_t)(t->next_audio_ts_min - t->timestamp);
			t->timestamp = t->next_audio_ts_min;
			t->continuous = true;
		} else {
			/* logged by the caller */
			t->action = OBS_SOURCE_AUDIO_TS_EXCEEDED;
			t->diff = diff;
		}
	}

	t->next_audio_ts_min = t->timestamp + t->duration;
}

void obs_source_audio_timing_under_lock(struct obs_source_audio_timing_under *t)
{
	uint64_t timestamp = t->timestamp + t->packet_timing_adjust;
	uint64_t diff;
	bool push_back = false;

	t->reset_timing = false;

	if (t->next_audio_sys_ts_min == timestamp) {
		push_back = true;

	} else if (t->next_audio_sys_ts_min) {
		diff = audio_timing_diff(t->next_audio_sys_ts_min, timestamp);

		if (diff < OBS_SOURCE_AUDIO_TS_SMOOTHING_THRESHOLD) {
			push_back = true;

		} else if (diff > OBS_SOURCE_AUDIO_MAX_TS_VAR && !t->continuous) {
			/* This typically only happens if used with async video when
			 * audio/video start transitioning in to a timestamp jump.
			 * Audio will typically have a timestamp jump, and then video
			 * will have a timestamp jump.  If that case is encountered,
			 * just clear the audio data in that small window and force a
			 * resync.  This handles all cases rather than just looping.
			 *
			 * If the audio itself did not jump, the timing of the
			 * source's video moved (a Video Delay (Async) filter), and
			 * the audio follows it as it does for smaller moves:
			 * resetting would only be undone by the next frame. */
			t->reset_timing = true;
			t->timing_adjust = t->os_time - t->data_timestamp;
			timestamp = t->data_timestamp + t->timing_adjust;
		}
	}

	timestamp += t->sync_offset;
	timestamp -= t->resample_offset;

	t->next_audio_sys_ts_min = t->next_audio_ts_min + t->timing_adjust;

	/* A packet appended to the buffer ignores its timestamp, and so the
	 * sync offset in it: after the offset changes, place the next packet
	 * by its timestamp, whatever the previous offset was. Before the first
	 * packet (no audio_ts yet) it is placed anyway. */
	if (t->last_sync_offset != t->sync_offset) {
		push_back = false;
		t->last_sync_offset = t->sync_offset;
	}

	t->timestamp = timestamp;
	t->push_back = push_back;
}

/* Empties the buffer for packets that continue at system time os_time:
 * reset_audio_data_at_sys_time() starts it where they belong, at that time
 * shifted by the sync offset, as a packet placed on its own would be. */
uint64_t obs_source_audio_timing_buffer_start(uint64_t os_time, int64_t sync_offset, uint64_t resample_offset)
{
	if (os_time)
		return os_time + sync_offset - resample_offset;
	return os_time;
}
