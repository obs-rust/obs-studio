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

#include "libobs/build/libobs.pch.h"

/* Private adapter, not a new public API (obs-source-audio-output-seam.h). */

/* obs_source::audio_output empties this many buffers */
_Static_assert(MAX_AUDIO_CHANNELS == 8, "MAX_AUDIO_CHANNELS");
_Static_assert(sizeof(enum obs_monitoring_type) == sizeof(int), "enum obs_monitoring_type is passed as int");

uint64_t obs_source_audio_output_seam_now(void)
{
	return os_gettime_ns();
}

uint32_t obs_source_audio_output_seam_sample_rate(void)
{
	return audio_output_get_sample_rate(obs->audio.audio);
}

size_t obs_source_audio_output_seam_channels(void)
{
	return audio_output_get_channels(obs->audio.audio);
}

uint64_t obs_source_audio_output_seam_data_timestamp(const struct audio_data *data)
{
	return data->timestamp;
}

uint32_t obs_source_audio_output_seam_data_frames(const struct audio_data *data)
{
	return data->frames;
}

/* Locking synchronizes the reads without changing the logical source. */
void obs_source_audio_output_seam_lock_audio_buf(const obs_source_t *source)
{
	pthread_mutex_lock((pthread_mutex_t *)(uintptr_t)&source->audio_buf_mutex);
}

void obs_source_audio_output_seam_unlock_audio_buf(const obs_source_t *source)
{
	pthread_mutex_unlock((pthread_mutex_t *)(uintptr_t)&source->audio_buf_mutex);
}

bool obs_source_audio_output_seam_get_timing_set(const obs_source_t *source)
{
	return source->timing_set;
}

void obs_source_audio_output_seam_set_timing_set(obs_source_t *source, bool value)
{
	source->timing_set = value;
}

uint64_t obs_source_audio_output_seam_get_timing_adjust(const obs_source_t *source)
{
	return source->timing_adjust;
}

void obs_source_audio_output_seam_set_timing_adjust(obs_source_t *source, uint64_t value)
{
	source->timing_adjust = value;
}

uint64_t obs_source_audio_output_seam_get_next_audio_ts_min(const obs_source_t *source)
{
	return source->next_audio_ts_min;
}

void obs_source_audio_output_seam_set_next_audio_ts_min(obs_source_t *source, uint64_t value)
{
	source->next_audio_ts_min = value;
}

uint64_t obs_source_audio_output_seam_get_next_audio_sys_ts_min(const obs_source_t *source)
{
	return source->next_audio_sys_ts_min;
}

void obs_source_audio_output_seam_set_next_audio_sys_ts_min(obs_source_t *source, uint64_t value)
{
	source->next_audio_sys_ts_min = value;
}

uint64_t obs_source_audio_output_seam_get_audio_ts(const obs_source_t *source)
{
	return source->audio_ts;
}

void obs_source_audio_output_seam_set_audio_ts(obs_source_t *source, uint64_t value)
{
	source->audio_ts = value;
}

void obs_source_audio_output_seam_clear_last_buf_size(obs_source_t *source)
{
	source->last_audio_input_buf_size = 0;
}

int64_t obs_source_audio_output_seam_get_sync_offset(const obs_source_t *source)
{
	return source->sync_offset;
}

int64_t obs_source_audio_output_seam_get_last_sync_offset(const obs_source_t *source)
{
	return source->last_sync_offset;
}

void obs_source_audio_output_seam_set_last_sync_offset(obs_source_t *source, int64_t value)
{
	source->last_sync_offset = value;
}

uint64_t obs_source_audio_output_seam_get_resample_offset(const obs_source_t *source)
{
	return source->resample_offset;
}

int obs_source_audio_output_seam_get_monitoring_type(const obs_source_t *source)
{
	return (int)source->monitoring_type;
}

bool obs_source_audio_output_seam_get_async_unbuffered(const obs_source_t *source)
{
	return source->async_unbuffered;
}

bool obs_source_audio_output_seam_get_async_decoupled(const obs_source_t *source)
{
	return source->async_decoupled;
}

uint64_t obs_source_audio_output_seam_get_last_output_frame_ts(const obs_source_t *source)
{
	return source->last_output_frame_ts;
}

bool obs_source_audio_output_seam_compensating(const obs_source_t *source)
{
	return source->resampler && source->last_async_compensation;
}

void obs_source_audio_output_seam_set_compensation_error(obs_source_t *source, int64_t error)
{
	audio_resampler_set_compensation_error(source->resampler, error);
}

size_t obs_source_audio_output_seam_buf_size(const obs_source_t *source, size_t channel)
{
	return source->audio_input_buf[channel].size;
}

void obs_source_audio_output_seam_pop_front(obs_source_t *source, size_t channel, size_t size)
{
	deque_pop_front(&source->audio_input_buf[channel], NULL, size);
}

void obs_source_audio_output_seam_pop_back(obs_source_t *source, size_t channel, size_t size)
{
	deque_pop_back(&source->audio_input_buf[channel], NULL, size);
}

void obs_source_audio_output_seam_place(obs_source_t *source, size_t channel, size_t placement,
					const struct audio_data *data, size_t size)
{
	deque_place(&source->audio_input_buf[channel], placement, data->data[channel], size);
}

void obs_source_audio_output_seam_push_back(obs_source_t *source, size_t channel, const struct audio_data *data,
					    size_t size)
{
	deque_push_back(&source->audio_input_buf[channel], data->data[channel], size);
}

void obs_source_audio_output_seam_log_ts_jump(const obs_source_t *source, uint64_t diff, uint64_t expected,
					      uint64_t timestamp)
{
	blog(LOG_DEBUG,
	     "Timestamp for source '%s' jumped by '%" PRIu64 "', "
	     "expected value %" PRIu64 ", input value %" PRIu64,
	     source->context.name, diff, expected, timestamp);
}

void obs_source_audio_output_seam_log_ts_exceeded(const obs_source_t *source, uint64_t diff, uint64_t expected,
						  uint64_t timestamp)
{
	blog(LOG_DEBUG,
	     "Audio timestamp for '%s' exceeded TS_SMOOTHING_THRESHOLD, diff=%" PRIu64 " ns, expected %" PRIu64
	     ", input %" PRIu64,
	     source->context.name, diff, expected, timestamp);
}

bool obs_source_audio_output_seam_muted(obs_source_t *source, uint64_t os_time)
{
	return source_muted(source, os_time);
}

void obs_source_audio_output_seam_signal_audio_data(obs_source_t *source, const struct audio_data *data, bool muted)
{
	source_signal_audio_data(source, data, muted);
}
