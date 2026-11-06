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
#include "libobs/build/libobs-ported-source-audio-output.pch.h"

/* The path that puts one audio packet of a source in its buffer: moved out of
 * obs-source.c as it is (#3628). source_muted() and
 * source_signal_audio_data() stay in obs-source.c.
 * Hybrid libobs has it in Rust (obs_source::audio_output, shims in
 * obs-source-seam). This file stays for the C reference and the oracle. */

void reset_audio_timing(obs_source_t *source, uint64_t timestamp, uint64_t os_time)
{
	source->timing_set = true;
	source->timing_adjust = os_time - timestamp;
}

void reset_audio_data(obs_source_t *source, uint64_t os_time)
{
	for (size_t i = 0; i < MAX_AUDIO_CHANNELS; i++) {
		if (source->audio_input_buf[i].size)
			deque_pop_front(&source->audio_input_buf[i], NULL, source->audio_input_buf[i].size);
	}

	source->last_audio_input_buf_size = 0;
	source->audio_ts = os_time;
	source->next_audio_sys_ts_min = os_time;
}

/* Empties the buffer for packets that continue at system time os_time.
 * source_output_audio_data() appends such packets to the buffer, which must
 * then start where they belong: at that time shifted by the sync offset, as
 * a packet placed on its own would be. */
void reset_audio_data_at_sys_time(obs_source_t *source, uint64_t os_time)
{
	reset_audio_data(source, os_time);
	source->audio_ts = obs_source_audio_timing_buffer_start(os_time, source->sync_offset, source->resample_offset);
}

static void handle_ts_jump(obs_source_t *source, uint64_t expected, uint64_t ts, uint64_t diff, uint64_t os_time)
{
	blog(LOG_DEBUG,
	     "Timestamp for source '%s' jumped by '%" PRIu64 "', "
	     "expected value %" PRIu64 ", input value %" PRIu64,
	     source->context.name, diff, expected, ts);

	pthread_mutex_lock(&source->audio_buf_mutex);
	reset_audio_timing(source, ts, os_time);
	reset_audio_data_at_sys_time(source, os_time);
	pthread_mutex_unlock(&source->audio_buf_mutex);
}

static inline size_t get_buf_placement(audio_t *audio, uint64_t timestamp, uint64_t audio_ts)
{
	uint32_t sample_rate = audio_output_get_sample_rate(audio);
	return obs_source_audio_place_offset(timestamp, audio_ts, sample_rate);
}

static void source_output_audio_place(obs_source_t *source, const struct audio_data *in)
{
	audio_t *audio = obs->audio.audio;
	size_t buf_placement;
	size_t channels = audio_output_get_channels(audio);
	size_t size = obs_source_audio_place_size(in->frames);

	if (!obs_source_audio_place_started(source->audio_ts) ||
	    obs_source_audio_place_older(in->timestamp, source->audio_ts))
		reset_audio_data(source, in->timestamp);

	buf_placement = get_buf_placement(audio, in->timestamp, source->audio_ts);

#if defined(DEBUG_AUDIO) && DEBUG_AUDIO == 1
	blog(LOG_DEBUG, "frames: %lu, size: %lu, placement: %lu, base_ts: %llu, ts: %llu", (unsigned long)in->frames,
	     (unsigned long)source->audio_input_buf[0].size, (unsigned long)buf_placement, source->audio_ts,
	     in->timestamp);
#endif

	/* do not allow the circular buffers to become too big */
	if (!obs_source_audio_place_fits(buf_placement, size))
		return;

	for (size_t i = 0; i < channels; i++) {
		deque_place(&source->audio_input_buf[i], buf_placement, in->data[i], size);
		deque_pop_back(&source->audio_input_buf[i], NULL,
			       obs_source_audio_place_pop_back(source->audio_input_buf[i].size, buf_placement, size));
	}

	source->last_audio_input_buf_size = 0;
}

static inline void source_output_audio_push_back(obs_source_t *source, const struct audio_data *in)
{
	audio_t *audio = obs->audio.audio;
	size_t channels = audio_output_get_channels(audio);
	size_t size = obs_source_audio_place_size(in->frames);

	/* do not allow the circular buffers to become too big */
	if (!obs_source_audio_place_fits(source->audio_input_buf[0].size, size))
		return;

	for (size_t i = 0; i < channels; i++)
		deque_push_back(&source->audio_input_buf[i], in->data[i], size);

	/* reset audio input buffer size to ensure that audio doesn't get
	 * perpetually cut */
	source->last_audio_input_buf_size = 0;
}

void source_output_audio_data(obs_source_t *source, const struct audio_data *data)
{
	size_t sample_rate = audio_output_get_sample_rate(obs->audio.audio);
	struct audio_data in = *data;
	uint64_t os_time = os_gettime_ns();
	uint64_t timing_adjust;
	bool push_back;

	/* The timestamp placement is obs-source-audio-timing.c's: each step
	 * gets the fields it reads and returns what to store, and the stores
	 * happen here, in their order. The graphics thread also sets timing_set
	 * and timing_adjust, so they go under audio_buf_mutex. */
	pthread_mutex_lock(&source->audio_buf_mutex);

	struct obs_source_audio_timing_pre pre = {
		.timestamp = in.timestamp,
		.os_time = os_time,
		.duration = conv_frames_to_time(sample_rate, in.frames),
		.next_audio_ts_min = source->next_audio_ts_min,
		.timing_set = source->timing_set,
		.async_unbuffered = source->async_unbuffered,
		.async_decoupled = source->async_decoupled,
	};
	obs_source_audio_timing_pre_lock(&pre);

	if (pre.direct) {
		source->timing_adjust = 0;
		source->timing_set = true;
	}

	if (pre.action == OBS_SOURCE_AUDIO_TS_RESET) {
		/* reset_audio_timing(source, in.timestamp, os_time) */
		source->timing_set = true;
		source->timing_adjust = pre.timing_adjust;
	} else if (pre.action == OBS_SOURCE_AUDIO_TS_SMOOTH && pre.reanchor) {
		source->timing_adjust = pre.timing_adjust;
	}

	pthread_mutex_unlock(&source->audio_buf_mutex);

	switch (pre.action) {
	case OBS_SOURCE_AUDIO_TS_KEEP:
	case OBS_SOURCE_AUDIO_TS_RESET:
	case OBS_SOURCE_AUDIO_TS_SMOOTH:
		break;
	case OBS_SOURCE_AUDIO_TS_JUMP:
		handle_ts_jump(source, source->next_audio_ts_min, in.timestamp, pre.diff, os_time);
		break;
	case OBS_SOURCE_AUDIO_TS_EXCEEDED:
		blog(LOG_DEBUG,
		     "Audio timestamp for '%s' exceeded TS_SMOOTHING_THRESHOLD, diff=%" PRIu64 " ns, expected %" PRIu64
		     ", input %" PRIu64,
		     source->context.name, pre.diff, source->next_audio_ts_min, in.timestamp);
		break;
	}

	/* Feed the error to the resampler so that the samples keep pace with
	 * the timestamps instead of drifting until the threshold above is
	 * exceeded. After a resync the error is unknown, so hold. */
	if (source->resampler && source->last_async_compensation)
		audio_resampler_set_compensation_error(source->resampler, pre.drift_error);

	source->next_audio_ts_min = pre.next_audio_ts_min;

	pthread_mutex_lock(&source->audio_buf_mutex);

	timing_adjust = source->timing_adjust;

	struct obs_source_audio_timing_under under = {
		.timestamp = pre.timestamp,
		.packet_timing_adjust = timing_adjust,
		.data_timestamp = data->timestamp,
		.os_time = os_time,
		.next_audio_ts_min = source->next_audio_ts_min,
		.resample_offset = source->resample_offset,
		.sync_offset = source->sync_offset,
		.timing_adjust = source->timing_adjust,
		.next_audio_sys_ts_min = source->next_audio_sys_ts_min,
		.last_sync_offset = source->last_sync_offset,
		/* a continuous packet follows the video's timing only on the
		 * video's clock (#3747) */
		.continuous = pre.continuous &&
			      obs_source_audio_timing_follows_video(data->timestamp, source->last_output_frame_ts),
	};
	obs_source_audio_timing_under_lock(&under);

	if (under.reset_timing) {
		/* reset_audio_timing(source, data->timestamp, os_time) */
		source->timing_set = true;
		source->timing_adjust = under.timing_adjust;
	}
	source->next_audio_sys_ts_min = under.next_audio_sys_ts_min;
	source->last_sync_offset = under.last_sync_offset;
	in.timestamp = under.timestamp;
	push_back = under.push_back;

	if (obs_source_audio_place_outputs((int)source->monitoring_type)) {
		if (push_back && obs_source_audio_place_started(source->audio_ts))
			source_output_audio_push_back(source, &in);
		else
			source_output_audio_place(source, &in);
	}

	pthread_mutex_unlock(&source->audio_buf_mutex);

	source_signal_audio_data(source, data, source_muted(source, os_time));
}
