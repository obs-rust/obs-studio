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

/* The audio input path of a source: moved out of obs-source.c as it is (#3632).
 * filter_async_audio() stays in obs-source.c. */

/* The audio input steps are obs-source-audio-input.c's: they get the
 * fields they read, and the stores happen here, in their order. */
static inline struct obs_source_audio_input_format audio_input_format(const struct resample_info *info)
{
	const struct obs_source_audio_input_format format = {
		.format = (int)info->format,
		.samples_per_sec = info->samples_per_sec,
		.speakers = (int)info->speakers,
	};
	return format;
}

static inline void reset_resampler(obs_source_t *source, const struct obs_source_audio *audio)
{
	const struct audio_output_info *obs_info;
	struct resample_info output_info;

	obs_info = audio_output_get_info(obs->audio.audio);

	output_info.format = obs_info->format;
	output_info.samples_per_sec = obs_info->samples_per_sec;
	output_info.speakers = obs_info->speakers;

	source->sample_info.format = audio->format;
	source->sample_info.samples_per_sec = audio->samples_per_sec;
	source->sample_info.speakers = audio->speakers;
	source->last_async_compensation = os_atomic_load_bool(&source->async_compensation);

	audio_resampler_destroy(source->resampler);
	source->resampler = NULL;
	pthread_mutex_lock(&source->audio_buf_mutex);
	source->resample_offset = 0;
	pthread_mutex_unlock(&source->audio_buf_mutex);

	if (!obs_source_audio_input_resampler_needed(audio_input_format(&source->sample_info),
						     audio_input_format(&output_info),
						     source->last_async_compensation)) {
		source->audio_failed = false;
		return;
	}

	source->resampler = audio_resampler_create(&output_info, &source->sample_info);

	source->audio_failed = source->resampler == NULL;
	if (source->resampler == NULL)
		blog(LOG_ERROR, "creation of resampler failed");
}

static void copy_audio_data(obs_source_t *source, const uint8_t *const data[], uint32_t frames, uint64_t ts)
{
	size_t planes = audio_output_get_planes(obs->audio.audio);
	size_t blocksize = audio_output_get_block_size(obs->audio.audio);
	size_t size = (size_t)frames * blocksize;
	bool resize = source->audio_storage_size < size;

	source->audio_data.frames = frames;
	source->audio_data.timestamp = ts;

	for (size_t i = 0; i < planes; i++) {
		/* ensure audio storage capacity */
		if (resize) {
			bfree(source->audio_data.data[i]);
			source->audio_data.data[i] = bmalloc(size);
		}

		memcpy(source->audio_data.data[i], data[i], size);
	}

	if (resize)
		source->audio_storage_size = size;
}

/* resamples/remixes new audio to the designated main audio output format */
static void process_audio(obs_source_t *source, const struct obs_source_audio *audio)
{
	uint32_t frames = audio->frames;
	bool mono_output;

	/* A packet that does not continue the previous one restarts the
	 * resampler (#546). */
	bool discontinuous = obs_source_audio_input_discontinuous(source->resampler != NULL,
								  source->next_resample_in_ts, audio->timestamp);

	const struct obs_source_audio_input_format packet = {
		.format = (int)audio->format,
		.samples_per_sec = audio->samples_per_sec,
		.speakers = (int)audio->speakers,
	};
	if (obs_source_audio_input_reset_needed(audio_input_format(&source->sample_info), packet,
						source->last_async_compensation,
						os_atomic_load_bool(&source->async_compensation), discontinuous))
		reset_resampler(source, audio);

	if (audio->samples_per_sec)
		source->next_resample_in_ts =
			audio->timestamp + util_mul_div64(audio->frames, 1000000000ULL, audio->samples_per_sec);

	if (source->audio_failed)
		return;

	if (source->resampler) {
		uint8_t *output[MAX_AV_PLANES];

		memset(output, 0, sizeof(output));

		/* The audio path reads the offset under audio_buf_mutex. The resampler
		 * works on a copy, so it does not run with the mutex held. */
		pthread_mutex_lock(&source->audio_buf_mutex);
		uint64_t resample_offset = source->resample_offset;
		pthread_mutex_unlock(&source->audio_buf_mutex);

		audio_resampler_resample(source->resampler, output, &frames, &resample_offset, audio->data,
					 audio->frames);

		pthread_mutex_lock(&source->audio_buf_mutex);
		source->resample_offset = resample_offset;
		pthread_mutex_unlock(&source->audio_buf_mutex);

		copy_audio_data(source, (const uint8_t *const *)output, frames, audio->timestamp);
	} else {
		copy_audio_data(source, audio->data, audio->frames, audio->timestamp);
	}

	mono_output = audio_output_get_channels(obs->audio.audio) == 1;

	/* one value for the whole packet, the setter can run at any time */
	const float balance = obs_source_get_balance_value(source);

	if (obs_source_audio_input_balance_needed(mono_output, (int)source->sample_info.speakers, balance)) {
		obs_source_audio_input_balance((float **)source->audio_data.data, frames, balance,
					       OBS_BALANCE_TYPE_SINE_LAW);
	}

	if (obs_source_audio_input_mono_needed(mono_output, source->flags))
		obs_source_audio_input_downmix((float **)source->audio_data.data,
					       audio_output_get_channels(obs->audio.audio), frames);
}

void obs_source_output_audio(obs_source_t *source, const struct obs_source_audio *audio_in)
{
	struct obs_audio_data *output;

	if (!obs_source_valid(source, "obs_source_output_audio"))
		return;
	if (destroying(source))
		return;
	if (!obs_ptr_valid(audio_in, "obs_source_output_audio"))
		return;

	/* sets unused data pointers to NULL automatically because apparently
	 * some filter plugins aren't checking the actual channel count, and
	 * instead are checking to see whether the pointer is non-zero. */
	struct obs_source_audio audio = *audio_in;
	size_t channels = get_audio_planes(audio.format, audio.speakers);
	for (size_t i = channels; i < MAX_AUDIO_CHANNELS; i++)
		audio.data[i] = NULL;

	process_audio(source, &audio);

	pthread_mutex_lock(&source->filter_mutex);
	output = filter_async_audio(source, &source->audio_data);

	if (output) {
		struct audio_data data;

		for (int i = 0; i < MAX_AV_PLANES; i++)
			data.data[i] = output->data[i];

		data.frames = output->frames;
		data.timestamp = output->timestamp;

		pthread_mutex_lock(&source->audio_mutex);
		source_output_audio_data(source, &data);
		pthread_mutex_unlock(&source->audio_mutex);
	}

	pthread_mutex_unlock(&source->filter_mutex);
}
