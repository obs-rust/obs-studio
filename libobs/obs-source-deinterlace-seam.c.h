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

/* Private adapter, not a new public API (obs-source-deinterlace-seam.h).
 * Each body is the line of obs-source-deinterlace.c it is copied from. */

/* The async queue. */

size_t obs_source_deinterlace_seam_get_async_frames_num(const obs_source_t *source)
{
	return source->async_frames.num;
}

struct obs_source_frame *obs_source_deinterlace_seam_get_async_frame(const obs_source_t *source, size_t idx)
{
	return source->async_frames.array[idx];
}

void obs_source_deinterlace_seam_erase_async_frame_front(obs_source_t *source)
{
	da_erase(source->async_frames, 0);
}

int obs_source_deinterlace_seam_async_mutex_lock(obs_source_t *source)
{
	return pthread_mutex_lock(&source->async_mutex);
}

int obs_source_deinterlace_seam_async_mutex_unlock(obs_source_t *source)
{
	return pthread_mutex_unlock(&source->async_mutex);
}

/* The clock fields. */

bool obs_source_deinterlace_seam_get_async_unbuffered(const obs_source_t *source)
{
	return source->async_unbuffered;
}

uint64_t obs_source_deinterlace_seam_get_last_sys_timestamp(const obs_source_t *source)
{
	return source->last_sys_timestamp;
}

uint64_t obs_source_deinterlace_seam_get_last_frame_ts(const obs_source_t *source)
{
	return source->last_frame_ts;
}

void obs_source_deinterlace_seam_set_last_frame_ts(obs_source_t *source, uint64_t value)
{
	source->last_frame_ts = value;
}

uint64_t obs_source_deinterlace_seam_get_deinterlace_offset(const obs_source_t *source)
{
	return source->deinterlace_offset;
}

void obs_source_deinterlace_seam_set_deinterlace_offset(obs_source_t *source, uint64_t value)
{
	source->deinterlace_offset = value;
}

uint64_t obs_source_deinterlace_seam_get_deinterlace_frame_ts(const obs_source_t *source)
{
	return source->deinterlace_frame_ts;
}

void obs_source_deinterlace_seam_set_deinterlace_frame_ts(obs_source_t *source, uint64_t value)
{
	source->deinterlace_frame_ts = value;
}

uint32_t obs_source_deinterlace_seam_get_deinterlace_half_duration(const obs_source_t *source)
{
	return source->deinterlace_half_duration;
}

void obs_source_deinterlace_seam_set_deinterlace_half_duration(obs_source_t *source, uint32_t value)
{
	source->deinterlace_half_duration = value;
}

/* The frame slots. */

struct obs_source_frame *obs_source_deinterlace_seam_get_cur_async_frame(const obs_source_t *source)
{
	return source->cur_async_frame;
}

void obs_source_deinterlace_seam_set_cur_async_frame(obs_source_t *source, struct obs_source_frame *value)
{
	source->cur_async_frame = value;
}

struct obs_source_frame *obs_source_deinterlace_seam_get_prev_async_frame(const obs_source_t *source)
{
	return source->prev_async_frame;
}

void obs_source_deinterlace_seam_set_prev_async_frame(obs_source_t *source, struct obs_source_frame *value)
{
	source->prev_async_frame = value;
}

/* The texture fields. */

enum video_format obs_source_deinterlace_seam_get_async_format(const obs_source_t *source)
{
	return source->async_format;
}

uint8_t obs_source_deinterlace_seam_get_async_trc(const obs_source_t *source)
{
	return source->async_trc;
}

uint32_t obs_source_deinterlace_seam_get_async_width(const obs_source_t *source)
{
	return source->async_width;
}

uint32_t obs_source_deinterlace_seam_get_async_height(const obs_source_t *source)
{
	return source->async_height;
}

bool obs_source_deinterlace_seam_get_async_flip(const obs_source_t *source)
{
	return source->async_flip;
}

gs_texture_t **obs_source_deinterlace_seam_get_async_textures(obs_source_t *source)
{
	return source->async_textures;
}

gs_texture_t **obs_source_deinterlace_seam_get_async_prev_textures(obs_source_t *source)
{
	return source->async_prev_textures;
}

gs_texrender_t **obs_source_deinterlace_seam_get_async_texrender_slot(obs_source_t *source)
{
	return &source->async_texrender;
}

gs_texrender_t **obs_source_deinterlace_seam_get_async_prev_texrender_slot(obs_source_t *source)
{
	return &source->async_prev_texrender;
}

/* The deinterlace fields. */

bool obs_source_deinterlace_seam_get_deinterlace_rendered(const obs_source_t *source)
{
	return source->deinterlace_rendered;
}

void obs_source_deinterlace_seam_set_deinterlace_rendered(obs_source_t *source, bool value)
{
	source->deinterlace_rendered = value;
}

gs_effect_t *obs_source_deinterlace_seam_get_deinterlace_effect(const obs_source_t *source)
{
	return source->deinterlace_effect;
}

void obs_source_deinterlace_seam_set_deinterlace_effect(obs_source_t *source, gs_effect_t *value)
{
	source->deinterlace_effect = value;
}

void obs_source_deinterlace_seam_set_deinterlace_mode(obs_source_t *source, enum obs_deinterlace_mode value)
{
	source->deinterlace_mode = value;
}

void obs_source_deinterlace_seam_set_deinterlace_top_first(obs_source_t *source, bool value)
{
	source->deinterlace_top_first = value;
}

/* The header inline. */

enum gs_color_space obs_source_deinterlace_seam_convert_video_space(enum video_format format, enum video_trc trc)
{
	return convert_video_space(format, trc);
}

/* The core video. */

uint64_t obs_source_deinterlace_seam_get_video_frame_interval_ns(void)
{
	return obs->video.video_frame_interval_ns;
}

struct obs_video_stats *obs_source_deinterlace_seam_get_video_stats(void)
{
	return &obs->video.stats;
}

gs_effect_t **obs_source_deinterlace_seam_get_discard_effect_slot(void)
{
	return &obs->video.deinterlace_discard_effect;
}

gs_effect_t **obs_source_deinterlace_seam_get_discard_2x_effect_slot(void)
{
	return &obs->video.deinterlace_discard_2x_effect;
}

gs_effect_t **obs_source_deinterlace_seam_get_blend_effect_slot(void)
{
	return &obs->video.deinterlace_blend_effect;
}

gs_effect_t **obs_source_deinterlace_seam_get_blend_2x_effect_slot(void)
{
	return &obs->video.deinterlace_blend_2x_effect;
}

gs_effect_t **obs_source_deinterlace_seam_get_linear_effect_slot(void)
{
	return &obs->video.deinterlace_linear_effect;
}

gs_effect_t **obs_source_deinterlace_seam_get_linear_2x_effect_slot(void)
{
	return &obs->video.deinterlace_linear_2x_effect;
}

gs_effect_t **obs_source_deinterlace_seam_get_yadif_effect_slot(void)
{
	return &obs->video.deinterlace_yadif_effect;
}

gs_effect_t **obs_source_deinterlace_seam_get_yadif_2x_effect_slot(void)
{
	return &obs->video.deinterlace_yadif_2x_effect;
}
