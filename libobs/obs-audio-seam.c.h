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

/* Private adapter, not a new public API (obs-audio-seam.h). */

/* ------------------------------------------------------------------------- */
/* The core audio */

audio_t *obs_audio_seam_get_output(void)
{
	return NULL;
}

size_t obs_audio_seam_render_order_num(void)
{
	return 0;
}

obs_source_t *obs_audio_seam_render_order_get(size_t idx)
{
	UNUSED_PARAMETER(idx);
	return NULL;
}

size_t obs_audio_seam_render_order_find(obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return 0;
}

void obs_audio_seam_render_order_push(obs_source_t *source)
{
	UNUSED_PARAMETER(source);
}

void obs_audio_seam_render_order_clear(void) {}

size_t obs_audio_seam_root_nodes_num(void)
{
	return 0;
}

obs_source_t *obs_audio_seam_root_nodes_get(size_t idx)
{
	UNUSED_PARAMETER(idx);
	return NULL;
}

void obs_audio_seam_root_nodes_push(obs_source_t *source)
{
	UNUSED_PARAMETER(source);
}

void obs_audio_seam_root_nodes_clear(void) {}

void obs_audio_seam_buffered_timestamps_push_back(const struct ts_info *ts)
{
	UNUSED_PARAMETER(ts);
}

void obs_audio_seam_buffered_timestamps_peek_front(struct ts_info *ts)
{
	UNUSED_PARAMETER(ts);
}

void obs_audio_seam_buffered_timestamps_pop_front(void) {}

struct obs_audio_buffering obs_audio_seam_get_buffering(void)
{
	struct obs_audio_buffering buffering = {0};
	return buffering;
}

bool obs_audio_seam_get_fixed_buffer(void)
{
	return false;
}

uint64_t obs_audio_seam_get_buffering_wait_ticks(void)
{
	return 0;
}

void obs_audio_seam_set_buffering_wait_ticks(uint64_t value)
{
	UNUSED_PARAMETER(value);
}

obs_source_t *obs_audio_seam_get_monitoring_duplicating_source(void)
{
	return NULL;
}

int obs_audio_seam_lock_tasks(void)
{
	return 0;
}

int obs_audio_seam_unlock_tasks(void)
{
	return 0;
}

size_t obs_audio_seam_tasks_size(void)
{
	return 0;
}

void obs_audio_seam_tasks_pop_front(struct obs_task_info *info)
{
	UNUSED_PARAMETER(info);
}

/* ------------------------------------------------------------------------- */
/* The core data and video */

obs_source_t *obs_audio_seam_get_first_audio_source(void)
{
	return NULL;
}

int obs_audio_seam_lock_audio_sources(void)
{
	return 0;
}

int obs_audio_seam_unlock_audio_sources(void)
{
	return 0;
}

int obs_audio_seam_lock_mixes(void)
{
	return 0;
}

int obs_audio_seam_unlock_mixes(void)
{
	return 0;
}

size_t obs_audio_seam_mixes_num(void)
{
	return 0;
}

struct obs_view *obs_audio_seam_mix_get_view(size_t idx)
{
	UNUSED_PARAMETER(idx);
	return NULL;
}

bool obs_audio_seam_mix_get_mix_audio(size_t idx)
{
	UNUSED_PARAMETER(idx);
	return false;
}

int obs_audio_seam_view_lock_channels(struct obs_view *view)
{
	UNUSED_PARAMETER(view);
	return 0;
}

int obs_audio_seam_view_unlock_channels(struct obs_view *view)
{
	UNUSED_PARAMETER(view);
	return 0;
}

obs_source_t *obs_audio_seam_view_get_channel(const struct obs_view *view, uint32_t idx)
{
	UNUSED_PARAMETER(view);
	UNUSED_PARAMETER(idx);
	return NULL;
}

/* ------------------------------------------------------------------------- */
/* A source */

obs_source_t *obs_audio_seam_source_get_next(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return NULL;
}

int obs_audio_seam_source_lock_buf(obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return 0;
}

int obs_audio_seam_source_unlock_buf(obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return 0;
}

bool obs_audio_seam_source_get_is_duplicated(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return false;
}

void obs_audio_seam_source_set_is_duplicated(obs_source_t *source, bool value)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(value);
}

bool obs_audio_seam_source_get_pending(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return false;
}

void obs_audio_seam_source_set_pending(obs_source_t *source, bool value)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(value);
}

uint64_t obs_audio_seam_source_get_audio_ts(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return 0;
}

size_t obs_audio_seam_source_get_input_size(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return 0;
}

struct obs_audio_input obs_audio_seam_source_get_input(obs_source_t *source)
{
	struct obs_audio_input in = {0};
	UNUSED_PARAMETER(source);
	return in;
}

enum obs_source_type obs_audio_seam_source_get_type(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return OBS_SOURCE_TYPE_INPUT;
}

uint32_t obs_audio_seam_source_get_output_flags(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return 0;
}

bool obs_audio_seam_source_has_audio_render(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return false;
}

obs_audio_seam_output_row_t *obs_audio_seam_source_get_output_buf(obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return NULL;
}

bool obs_audio_seam_source_get_muted(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return false;
}

float obs_audio_seam_source_get_volume(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return 0.0f;
}

uint32_t obs_audio_seam_source_get_audio_mixers(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return 0;
}

bool obs_audio_seam_source_get_monitoring_enabled(obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return false;
}
