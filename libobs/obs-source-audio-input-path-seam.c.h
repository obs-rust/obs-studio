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

/* Private adapter, not a new public API (obs-source-audio-input-path-seam.h). */

/* obs_source::audio_input_path::Audio is struct obs_source_audio, field for
 * field, and it clears the planes up to its array's length. */
_Static_assert(MAX_AV_PLANES == 8, "MAX_AV_PLANES");
_Static_assert(MAX_AUDIO_CHANNELS == 8, "MAX_AUDIO_CHANNELS");
_Static_assert(sizeof(enum speaker_layout) == sizeof(int), "enum speaker_layout is passed as int");
_Static_assert(sizeof(enum audio_format) == sizeof(int), "enum audio_format is passed as int");
_Static_assert(offsetof(struct obs_source_audio, data) == 0, "struct obs_source_audio");
_Static_assert(offsetof(struct obs_source_audio, frames) == 8 * sizeof(void *), "struct obs_source_audio");
_Static_assert(offsetof(struct obs_source_audio, speakers) == 8 * sizeof(void *) + 4, "struct obs_source_audio");
_Static_assert(offsetof(struct obs_source_audio, format) == 8 * sizeof(void *) + 8, "struct obs_source_audio");
_Static_assert(offsetof(struct obs_source_audio, samples_per_sec) == 8 * sizeof(void *) + 12,
	       "struct obs_source_audio");
_Static_assert(offsetof(struct obs_source_audio, timestamp) == 8 * sizeof(void *) + 16, "struct obs_source_audio");

bool obs_source_audio_input_path_seam_valid(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_output_audio");
}

bool obs_source_audio_input_path_seam_destroying(const obs_source_t *source)
{
	return destroying(source);
}

bool obs_source_audio_input_path_seam_input_valid(const struct obs_source_audio *audio_in)
{
	return obs_ptr_valid(audio_in, "obs_source_output_audio");
}

size_t obs_source_audio_input_path_seam_audio_planes(int format, int speakers)
{
	return get_audio_planes((enum audio_format)format, (enum speaker_layout)speakers);
}

struct obs_source_audio_input_format obs_source_audio_input_path_seam_output_format(void)
{
	const struct audio_output_info *info = audio_output_get_info(obs->audio.audio);
	const struct obs_source_audio_input_format format = {
		.format = (int)info->format,
		.samples_per_sec = info->samples_per_sec,
		.speakers = (int)info->speakers,
	};
	return format;
}

size_t obs_source_audio_input_path_seam_planes(void)
{
	return audio_output_get_planes(obs->audio.audio);
}

size_t obs_source_audio_input_path_seam_block_size(void)
{
	return audio_output_get_block_size(obs->audio.audio);
}

size_t obs_source_audio_input_path_seam_channels(void)
{
	return audio_output_get_channels(obs->audio.audio);
}

void obs_source_audio_input_path_seam_lock_audio_buf(obs_source_t *source)
{
	pthread_mutex_lock(&source->audio_buf_mutex);
}

void obs_source_audio_input_path_seam_unlock_audio_buf(obs_source_t *source)
{
	pthread_mutex_unlock(&source->audio_buf_mutex);
}

void obs_source_audio_input_path_seam_lock_filter(obs_source_t *source)
{
	pthread_mutex_lock(&source->filter_mutex);
}

void obs_source_audio_input_path_seam_unlock_filter(obs_source_t *source)
{
	pthread_mutex_unlock(&source->filter_mutex);
}

void obs_source_audio_input_path_seam_lock_audio(obs_source_t *source)
{
	pthread_mutex_lock(&source->audio_mutex);
}

void obs_source_audio_input_path_seam_unlock_audio(obs_source_t *source)
{
	pthread_mutex_unlock(&source->audio_mutex);
}

bool obs_source_audio_input_path_seam_has_resampler(const obs_source_t *source)
{
	return source->resampler != NULL;
}

uint64_t obs_source_audio_input_path_seam_get_next_resample_in_ts(const obs_source_t *source)
{
	return source->next_resample_in_ts;
}

void obs_source_audio_input_path_seam_set_next_resample_in_ts(obs_source_t *source, uint64_t value)
{
	source->next_resample_in_ts = value;
}

struct obs_source_audio_input_format obs_source_audio_input_path_seam_get_sample_info(const obs_source_t *source)
{
	const struct obs_source_audio_input_format format = {
		.format = (int)source->sample_info.format,
		.samples_per_sec = source->sample_info.samples_per_sec,
		.speakers = (int)source->sample_info.speakers,
	};
	return format;
}

int obs_source_audio_input_path_seam_get_sample_speakers(const obs_source_t *source)
{
	return (int)source->sample_info.speakers;
}

void obs_source_audio_input_path_seam_set_sample_info(obs_source_t *source, struct obs_source_audio_input_format value)
{
	source->sample_info.format = (enum audio_format)value.format;
	source->sample_info.samples_per_sec = value.samples_per_sec;
	source->sample_info.speakers = (enum speaker_layout)value.speakers;
}

bool obs_source_audio_input_path_seam_get_last_async_compensation(const obs_source_t *source)
{
	return source->last_async_compensation;
}

void obs_source_audio_input_path_seam_set_last_async_compensation(obs_source_t *source, bool value)
{
	source->last_async_compensation = value;
}

bool obs_source_audio_input_path_seam_get_async_compensation(const obs_source_t *source)
{
	return os_atomic_load_bool(&source->async_compensation);
}

bool obs_source_audio_input_path_seam_get_audio_failed(const obs_source_t *source)
{
	return source->audio_failed;
}

void obs_source_audio_input_path_seam_set_audio_failed(obs_source_t *source, bool value)
{
	source->audio_failed = value;
}

uint64_t obs_source_audio_input_path_seam_get_resample_offset(const obs_source_t *source)
{
	return source->resample_offset;
}

void obs_source_audio_input_path_seam_set_resample_offset(obs_source_t *source, uint64_t value)
{
	source->resample_offset = value;
}

size_t obs_source_audio_input_path_seam_get_audio_storage_size(const obs_source_t *source)
{
	return source->audio_storage_size;
}

void obs_source_audio_input_path_seam_set_audio_storage_size(obs_source_t *source, size_t value)
{
	source->audio_storage_size = value;
}

void obs_source_audio_input_path_seam_set_data_frames(obs_source_t *source, uint32_t value)
{
	source->audio_data.frames = value;
}

void obs_source_audio_input_path_seam_set_data_timestamp(obs_source_t *source, uint64_t value)
{
	source->audio_data.timestamp = value;
}

uint32_t obs_source_audio_input_path_seam_get_flags(const obs_source_t *source)
{
	return source->flags;
}

void obs_source_audio_input_path_seam_destroy_resampler(obs_source_t *source)
{
	audio_resampler_destroy(source->resampler);
	source->resampler = NULL;
}

bool obs_source_audio_input_path_seam_create_resampler(obs_source_t *source,
						       struct obs_source_audio_input_format output)
{
	struct resample_info output_info;

	output_info.format = (enum audio_format)output.format;
	output_info.samples_per_sec = output.samples_per_sec;
	output_info.speakers = (enum speaker_layout)output.speakers;

	source->resampler = audio_resampler_create(&output_info, &source->sample_info);
	return source->resampler != NULL;
}

void obs_source_audio_input_path_seam_log_resampler_failed(void)
{
	blog(LOG_ERROR, "creation of resampler failed");
}

void obs_source_audio_input_path_seam_resample(obs_source_t *source, uint8_t *output[], uint32_t *out_frames,
					       uint64_t *offset, const uint8_t *const data[], uint32_t frames)
{
	audio_resampler_resample(source->resampler, output, out_frames, offset, data, frames);
}

void obs_source_audio_input_path_seam_free_storage(obs_source_t *source, size_t plane)
{
	bfree(source->audio_data.data[plane]);
}

void obs_source_audio_input_path_seam_alloc_storage(obs_source_t *source, size_t plane, size_t size)
{
	source->audio_data.data[plane] = bmalloc(size);
}

void obs_source_audio_input_path_seam_copy_to_storage(obs_source_t *source, size_t plane, const uint8_t *data,
						      size_t size)
{
	memcpy(source->audio_data.data[plane], data, size);
}

float obs_source_audio_input_path_seam_balance_value(const obs_source_t *source)
{
	return obs_source_get_balance_value(source);
}

void obs_source_audio_input_path_seam_apply_balance(obs_source_t *source, uint32_t frames, float balance)
{
	obs_source_audio_input_balance((float **)source->audio_data.data, frames, balance, OBS_BALANCE_TYPE_SINE_LAW);
}

void obs_source_audio_input_path_seam_downmix(obs_source_t *source, size_t channels, uint32_t frames)
{
	obs_source_audio_input_downmix((float **)source->audio_data.data, channels, frames);
}

struct obs_audio_data *obs_source_audio_input_path_seam_filter_audio(obs_source_t *source)
{
	return filter_async_audio(source, &source->audio_data);
}

void obs_source_audio_input_path_seam_output_audio_data(obs_source_t *source, const struct obs_audio_data *output)
{
	struct audio_data data;

	for (int i = 0; i < MAX_AV_PLANES; i++)
		data.data[i] = output->data[i];

	data.frames = output->frames;
	data.timestamp = output->timestamp;

	source_output_audio_data(source, &data);
}
