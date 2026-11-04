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

#include "obs-internal.h"
#include "obs-source-audio-input.h"

/* Private adapter, not a new public API: the seam between the source audio
 * input path (rust/obs-source, obs_source::audio_input_path) and the fields of
 * struct obs_source (obs-source-audio-input-path-seam.c).
 *
 * - One lock and one unlock per mutex, each a single pthread_mutex_*() call.
 * - One get and one set per field, each a single read or write. The caller
 *   holds audio_buf_mutex around resample_offset.
 * - One function per call into libobs. The samples stay in C: the resampler,
 *   the storage, the copy, the balance, the downmix and the filters.
 * - filter_async_audio() stays in obs-source.c.
 *
 * The fields stay in C so that ThreadSanitizer keeps seeing their reads and
 * writes. Hidden in libobs: not EXPORT. */

bool obs_source_audio_input_path_seam_valid(const obs_source_t *source);
bool obs_source_audio_input_path_seam_destroying(const obs_source_t *source);
bool obs_source_audio_input_path_seam_input_valid(const struct obs_source_audio *audio_in);
size_t obs_source_audio_input_path_seam_audio_planes(int format, int speakers);

struct obs_source_audio_input_format obs_source_audio_input_path_seam_output_format(void);
size_t obs_source_audio_input_path_seam_planes(void);
size_t obs_source_audio_input_path_seam_block_size(void);
size_t obs_source_audio_input_path_seam_channels(void);

void obs_source_audio_input_path_seam_lock_audio_buf(obs_source_t *source);
void obs_source_audio_input_path_seam_unlock_audio_buf(obs_source_t *source);
void obs_source_audio_input_path_seam_lock_filter(obs_source_t *source);
void obs_source_audio_input_path_seam_unlock_filter(obs_source_t *source);
void obs_source_audio_input_path_seam_lock_audio(obs_source_t *source);
void obs_source_audio_input_path_seam_unlock_audio(obs_source_t *source);

bool obs_source_audio_input_path_seam_has_resampler(const obs_source_t *source);
uint64_t obs_source_audio_input_path_seam_get_next_resample_in_ts(const obs_source_t *source);
void obs_source_audio_input_path_seam_set_next_resample_in_ts(obs_source_t *source, uint64_t value);
struct obs_source_audio_input_format obs_source_audio_input_path_seam_get_sample_info(const obs_source_t *source);
int obs_source_audio_input_path_seam_get_sample_speakers(const obs_source_t *source);
void obs_source_audio_input_path_seam_set_sample_info(obs_source_t *source, struct obs_source_audio_input_format value);
bool obs_source_audio_input_path_seam_get_last_async_compensation(const obs_source_t *source);
void obs_source_audio_input_path_seam_set_last_async_compensation(obs_source_t *source, bool value);
bool obs_source_audio_input_path_seam_get_async_compensation(const obs_source_t *source);
bool obs_source_audio_input_path_seam_get_audio_failed(const obs_source_t *source);
void obs_source_audio_input_path_seam_set_audio_failed(obs_source_t *source, bool value);
uint64_t obs_source_audio_input_path_seam_get_resample_offset(const obs_source_t *source);
void obs_source_audio_input_path_seam_set_resample_offset(obs_source_t *source, uint64_t value);
size_t obs_source_audio_input_path_seam_get_audio_storage_size(const obs_source_t *source);
void obs_source_audio_input_path_seam_set_audio_storage_size(obs_source_t *source, size_t value);
void obs_source_audio_input_path_seam_set_data_frames(obs_source_t *source, uint32_t value);
void obs_source_audio_input_path_seam_set_data_timestamp(obs_source_t *source, uint64_t value);
uint32_t obs_source_audio_input_path_seam_get_flags(const obs_source_t *source);

void obs_source_audio_input_path_seam_destroy_resampler(obs_source_t *source);
bool obs_source_audio_input_path_seam_create_resampler(obs_source_t *source,
						       struct obs_source_audio_input_format output);
void obs_source_audio_input_path_seam_log_resampler_failed(void);
void obs_source_audio_input_path_seam_resample(obs_source_t *source, uint8_t *output[], uint32_t *out_frames,
					       uint64_t *offset, const uint8_t *const data[], uint32_t frames);

void obs_source_audio_input_path_seam_free_storage(obs_source_t *source, size_t plane);
void obs_source_audio_input_path_seam_alloc_storage(obs_source_t *source, size_t plane, size_t size);
void obs_source_audio_input_path_seam_copy_to_storage(obs_source_t *source, size_t plane, const uint8_t *data,
						      size_t size);

float obs_source_audio_input_path_seam_balance_value(const obs_source_t *source);
void obs_source_audio_input_path_seam_apply_balance(obs_source_t *source, uint32_t frames, float balance);
void obs_source_audio_input_path_seam_downmix(obs_source_t *source, size_t channels, uint32_t frames);

struct obs_audio_data *obs_source_audio_input_path_seam_filter_audio(obs_source_t *source);
void obs_source_audio_input_path_seam_output_audio_data(obs_source_t *source, const struct obs_audio_data *output);
