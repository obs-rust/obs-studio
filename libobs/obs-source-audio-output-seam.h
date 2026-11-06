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

/* Private adapter, not a new public API: the seam between the source audio
 * output path (rust/obs-source, obs_source::audio_output) and the fields of
 * struct obs_source (obs-source-audio-output-seam.c).
 *
 * - One lock and one unlock, each a single pthread_mutex_*() call.
 * - One get and one set per field, each a single read or write. The caller
 *   holds audio_buf_mutex around all of them except next_audio_ts_min,
 *   async_unbuffered, async_decoupled and the resampler's.
 * - One function per deque operation, for one channel. The caller holds
 *   audio_buf_mutex. The packet's data stays in C.
 * - source_muted() and source_signal_audio_data() stay in obs-source.c.
 *
 * The fields stay in C so that ThreadSanitizer keeps seeing their reads and
 * writes. Hidden in libobs: not EXPORT. */

uint64_t obs_source_audio_output_seam_now(void);
uint32_t obs_source_audio_output_seam_sample_rate(void);
size_t obs_source_audio_output_seam_channels(void);

uint64_t obs_source_audio_output_seam_data_timestamp(const struct audio_data *data);
uint32_t obs_source_audio_output_seam_data_frames(const struct audio_data *data);

void obs_source_audio_output_seam_lock_audio_buf(const obs_source_t *source);
void obs_source_audio_output_seam_unlock_audio_buf(const obs_source_t *source);

bool obs_source_audio_output_seam_get_timing_set(const obs_source_t *source);
void obs_source_audio_output_seam_set_timing_set(obs_source_t *source, bool value);
uint64_t obs_source_audio_output_seam_get_timing_adjust(const obs_source_t *source);
void obs_source_audio_output_seam_set_timing_adjust(obs_source_t *source, uint64_t value);
uint64_t obs_source_audio_output_seam_get_next_audio_ts_min(const obs_source_t *source);
void obs_source_audio_output_seam_set_next_audio_ts_min(obs_source_t *source, uint64_t value);
uint64_t obs_source_audio_output_seam_get_next_audio_sys_ts_min(const obs_source_t *source);
void obs_source_audio_output_seam_set_next_audio_sys_ts_min(obs_source_t *source, uint64_t value);
uint64_t obs_source_audio_output_seam_get_audio_ts(const obs_source_t *source);
void obs_source_audio_output_seam_set_audio_ts(obs_source_t *source, uint64_t value);
void obs_source_audio_output_seam_clear_last_buf_size(obs_source_t *source);
int64_t obs_source_audio_output_seam_get_sync_offset(const obs_source_t *source);
int64_t obs_source_audio_output_seam_get_last_sync_offset(const obs_source_t *source);
void obs_source_audio_output_seam_set_last_sync_offset(obs_source_t *source, int64_t value);
uint64_t obs_source_audio_output_seam_get_resample_offset(const obs_source_t *source);
int obs_source_audio_output_seam_get_monitoring_type(const obs_source_t *source);
bool obs_source_audio_output_seam_get_async_unbuffered(const obs_source_t *source);
bool obs_source_audio_output_seam_get_async_decoupled(const obs_source_t *source);
/* source->last_output_frame_ts, under audio_buf_mutex (#3747) */
uint64_t obs_source_audio_output_seam_get_last_output_frame_ts(const obs_source_t *source);

bool obs_source_audio_output_seam_compensating(const obs_source_t *source);
void obs_source_audio_output_seam_set_compensation_error(obs_source_t *source, int64_t error);

size_t obs_source_audio_output_seam_buf_size(const obs_source_t *source, size_t channel);
void obs_source_audio_output_seam_pop_front(obs_source_t *source, size_t channel, size_t size);
void obs_source_audio_output_seam_pop_back(obs_source_t *source, size_t channel, size_t size);
void obs_source_audio_output_seam_place(obs_source_t *source, size_t channel, size_t placement,
					const struct audio_data *data, size_t size);
void obs_source_audio_output_seam_push_back(obs_source_t *source, size_t channel, const struct audio_data *data,
					    size_t size);

void obs_source_audio_output_seam_log_ts_jump(const obs_source_t *source, uint64_t diff, uint64_t expected,
					      uint64_t timestamp);
void obs_source_audio_output_seam_log_ts_exceeded(const obs_source_t *source, uint64_t diff, uint64_t expected,
						  uint64_t timestamp);

bool obs_source_audio_output_seam_muted(obs_source_t *source, uint64_t os_time);
void obs_source_audio_output_seam_signal_audio_data(obs_source_t *source, const struct audio_data *data, bool muted);
