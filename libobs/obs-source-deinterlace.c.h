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

/* A frame is due once the source clock is this far past its timestamp. */
#define FRAME_TOLERANCE 2000000ULL
_Static_assert(FRAME_TOLERANCE == OBS_SOURCE_DEINTERLACE_PACING_FRAME_TOLERANCE, "FRAME_TOLERANCE");

static bool ready_deinterlace_frames(obs_source_t *source, uint64_t sys_time)
{
	struct obs_source_frame *next_frame = source->async_frames.array[0];
	struct obs_source_frame *prev_frame = NULL;
	struct obs_source_frame *frame = NULL;
	uint64_t sys_offset = sys_time - source->last_sys_timestamp;
	uint64_t frame_time = next_frame->timestamp;
	uint64_t frame_offset = 0;
	size_t idx = 1;

	if (source->async_unbuffered) {
		while (source->async_frames.num > 2) {
			da_erase(source->async_frames, 0);
			remove_async_frame(source, next_frame);
			next_frame = source->async_frames.array[0];
		}

		if (source->async_frames.num == 2) {
			bool has_prev_frame = true;
			if (source->async_unbuffered && source->deinterlace_offset) {
				const uint64_t timestamp = source->async_frames.array[0]->timestamp;
				const uint64_t after_timestamp = source->async_frames.array[1]->timestamp;
				if (obs_source_deinterlace_pacing_wait_for_next(timestamp, after_timestamp,
										source->deinterlace_offset, sys_time)) {
					// Don't skip ahead prematurely.
					has_prev_frame = false;
					source->deinterlace_frame_ts = obs_source_deinterlace_pacing_waiting_frame_ts(
						timestamp, after_timestamp);
				}
			}
			source->async_frames.array[0]->prev_frame = has_prev_frame;
		}
		source->last_frame_ts = next_frame->timestamp;
		return true;
	}

	/* account for timestamp invalidation */
	if (frame_out_of_bounds(source, frame_time)) {
		source->last_frame_ts = next_frame->timestamp;
		source->deinterlace_offset = 0;
		return true;
	} else {
		frame_offset = frame_time - source->last_frame_ts;
		source->last_frame_ts += sys_offset;
	}

	while (source->last_frame_ts > next_frame->timestamp) {

		/* this tries to reduce the needless frame duplication, also
		 * helps smooth out async rendering to frame boundaries.  In
		 * other words, tries to keep the framerate as smooth as
		 * possible */
		if ((source->last_frame_ts - next_frame->timestamp) < FRAME_TOLERANCE)
			break;

		if (prev_frame) {
			da_erase(source->async_frames, 0);
			remove_async_frame(source, prev_frame);
		}

		if (source->async_frames.num <= 2) {
			bool exit = true;

			/* Two frames or fewer left: stop and show the front one
			 * alone, unless none was taken yet. A prev_frame was just
			 * erased and has gone back to the cache, so it is no
			 * pair for the front frame and is not flagged (#2009). */
			if (!frame && source->async_frames.num == 2)
				exit = false;

			if (exit) {
				source->deinterlace_offset = 0;
				return true;
			}
		}

		if (frame)
			idx = 2;
		else
			idx = 1;

		prev_frame = frame;
		frame = next_frame;
		next_frame = source->async_frames.array[idx];

		/* more timestamp checking and compensating */
		if ((next_frame->timestamp - frame_time) > MAX_TS_VAR) {
			source->last_frame_ts = next_frame->timestamp - frame_offset;
			source->deinterlace_offset = 0;
		}

		frame_time = next_frame->timestamp;
		frame_offset = frame_time - source->last_frame_ts;
	}

	if (prev_frame)
		prev_frame->prev_frame = true;

	return frame != NULL;
}

static inline bool first_frame(obs_source_t *s)
{
	if (s->last_frame_ts)
		return false;

	if (s->async_frames.num >= 2)
		s->async_frames.array[0]->prev_frame = true;
	return true;
}

static inline void deinterlace_get_closest_frames(obs_source_t *s, uint64_t sys_time)
{
	if (s->async_unbuffered && s->deinterlace_offset) {
		// Want to keep frame if it has not elapsed, unless we think time jumped.
		if (obs_source_deinterlace_pacing_hold(s->deinterlace_frame_ts, s->deinterlace_offset,
						       s->deinterlace_half_duration, sys_time))
			return;
	}

	if (!s->async_frames.num)
		return;

	if (first_frame(s) || ready_deinterlace_frames(s, sys_time)) {
		s->prev_async_frame = NULL;
		s->cur_async_frame = s->async_frames.array[0];

		da_erase(s->async_frames, 0);

		if ((s->async_frames.num > 0) && s->cur_async_frame->prev_frame) {
			s->prev_async_frame = s->cur_async_frame;
			s->cur_async_frame = s->async_frames.array[0];

			da_erase(s->async_frames, 0);

			s->deinterlace_half_duration = obs_source_deinterlace_pacing_half_duration(
				s->cur_async_frame->timestamp, s->prev_async_frame->timestamp,
				s->deinterlace_half_duration, obs->video.video_frame_interval_ns);
		} else {
			s->deinterlace_half_duration = obs_source_deinterlace_pacing_half_duration(
				s->cur_async_frame->timestamp, s->deinterlace_frame_ts, s->deinterlace_half_duration,
				obs->video.video_frame_interval_ns);
		}

		if (!s->last_frame_ts)
			s->last_frame_ts = s->cur_async_frame->timestamp;

		s->deinterlace_frame_ts = s->cur_async_frame->timestamp;

		if (!s->async_unbuffered) {
			s->deinterlace_offset =
				obs_source_deinterlace_pacing_buffered_offset(obs->video.video_time, s->last_frame_ts);
			return;
		}

		s->deinterlace_offset = obs_source_deinterlace_pacing_reanchor(
			s->deinterlace_offset, obs->video.video_time, s->deinterlace_frame_ts,
			s->deinterlace_half_duration, obs->video.video_frame_interval_ns);
	}
}

void deinterlace_process_last_frame(obs_source_t *s, uint64_t sys_time)
{
	if (s->prev_async_frame) {
		remove_async_frame(s, s->prev_async_frame);
		s->prev_async_frame = NULL;
	}
	if (s->cur_async_frame) {
		remove_async_frame(s, s->cur_async_frame);
		s->cur_async_frame = NULL;
	}

	deinterlace_get_closest_frames(s, sys_time);
}

void set_deinterlace_texture_size(obs_source_t *source)
{
	const enum gs_color_format format = convert_video_format(source->async_format, source->async_trc);

	if (source->async_gpu_conversion) {
		source->async_prev_texrender = gs_texrender_create(format, GS_ZS_NONE);

		for (int c = 0; c < source->async_channel_count; c++)
			source->async_prev_textures[c] =
				gs_texture_create(source->async_convert_width[c], source->async_convert_height[c],
						  source->async_texture_formats[c], 1, NULL, GS_DYNAMIC);
	} else {
		source->async_prev_textures[0] =
			gs_texture_create(source->async_width, source->async_height, format, 1, NULL, GS_DYNAMIC);
	}
}

void deinterlace_update_async_video(obs_source_t *source)
{
	if (source->deinterlace_rendered)
		return;

	source->deinterlace_rendered = true;

	pthread_mutex_lock(&source->async_mutex);

	const bool updated = source->cur_async_frame != NULL;
	struct obs_source_frame *frame = source->prev_async_frame;
	source->prev_async_frame = NULL;

	pthread_mutex_unlock(&source->async_mutex);

	if (frame) {
		os_atomic_inc_long(&frame->refs);

		if (set_async_texture_size(source, frame)) {
			update_async_textures(source, frame, source->async_prev_textures, source->async_prev_texrender);
		}

		obs_source_release_frame(source, frame);

	} else if (updated) { /* swap cur/prev if no previous texture */
		for (size_t c = 0; c < MAX_AV_PLANES; c++) {
			gs_texture_t *prev_tex = source->async_prev_textures[c];
			source->async_prev_textures[c] = source->async_textures[c];
			source->async_textures[c] = prev_tex;
		}

		if (source->async_texrender) {
			gs_texrender_t *prev = source->async_prev_texrender;
			source->async_prev_texrender = source->async_texrender;
			source->async_texrender = prev;
		}
	}
}

_Static_assert(sizeof(enum obs_deinterlace_mode) == sizeof(int), "enum obs_deinterlace_mode is passed as int");
_Static_assert(sizeof(enum gs_color_space) == sizeof(int), "enum gs_color_space is passed as int");

static inline gs_effect_t *get_effect(enum obs_deinterlace_mode mode)
{
	switch (obs_source_deinterlace_draw_effect(mode)) {
	case OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_NONE:
		return NULL;
	case OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_DISCARD:
		return obs_load_effect(&obs->video.deinterlace_discard_effect, "deinterlace_discard.effect");
	case OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_DISCARD_2X:
		return obs_load_effect(&obs->video.deinterlace_discard_2x_effect, "deinterlace_discard_2x.effect");
	case OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_BLEND:
		return obs_load_effect(&obs->video.deinterlace_blend_effect, "deinterlace_blend.effect");
	case OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_BLEND_2X:
		return obs_load_effect(&obs->video.deinterlace_blend_2x_effect, "deinterlace_blend_2x.effect");
	case OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_LINEAR:
		return obs_load_effect(&obs->video.deinterlace_linear_effect, "deinterlace_linear.effect");
	case OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_LINEAR_2X:
		return obs_load_effect(&obs->video.deinterlace_linear_2x_effect, "deinterlace_linear_2x.effect");
	case OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_YADIF:
		return obs_load_effect(&obs->video.deinterlace_yadif_effect, "deinterlace_yadif.effect");
	case OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_YADIF_2X:
		return obs_load_effect(&obs->video.deinterlace_yadif_2x_effect, "deinterlace_yadif_2x.effect");
	}

	return NULL;
}

void deinterlace_render(obs_source_t *s)
{
	gs_effect_t *effect = s->deinterlace_effect;

	gs_eparam_t *image = gs_effect_get_param_by_name(effect, "image");
	gs_eparam_t *prev = gs_effect_get_param_by_name(effect, "previous_image");
	gs_eparam_t *multiplier_param = gs_effect_get_param_by_name(effect, "multiplier");
	gs_eparam_t *field = gs_effect_get_param_by_name(effect, "field_order");
	gs_eparam_t *frame2 = gs_effect_get_param_by_name(effect, "frame2");
	gs_eparam_t *dimensions = gs_effect_get_param_by_name(effect, "dimensions");
	struct vec2 size = {(float)s->async_width, (float)s->async_height};

	gs_texture_t *cur_tex = s->async_texrender ? gs_texrender_get_texture(s->async_texrender)
						   : s->async_textures[0];
	gs_texture_t *prev_tex = s->async_prev_texrender ? gs_texrender_get_texture(s->async_prev_texrender)
							 : s->async_prev_textures[0];

	if (obs_source_deinterlace_draw_skip(cur_tex != NULL, prev_tex != NULL, s->async_width, s->async_height))
		return;

	const enum gs_color_space source_space = convert_video_space(s->async_format, s->async_trc);

	const bool gs_linear_srgb = obs_source_deinterlace_draw_reads_linear_srgb(source_space) && gs_get_linear_srgb();
	const bool linear_srgb =
		obs_source_deinterlace_draw_linear_srgb(source_space, gs_linear_srgb, s->deinterlace_mode);

	const enum gs_color_space current_space = gs_get_color_space();
	const char *tech_name =
		obs_source_draw_technique_name(obs_source_deinterlace_draw_technique(source_space, current_space));
	const float multiplier = obs_source_deinterlace_draw_reads_white_level(source_space, current_space)
					 ? obs_source_deinterlace_draw_multiplier(source_space, current_space,
										  obs_get_video_sdr_white_level())
					 : 1.0f;

	const bool previous = gs_framebuffer_srgb_enabled();
	gs_enable_framebuffer_srgb(linear_srgb);

	if (linear_srgb) {
		gs_effect_set_texture_srgb(image, cur_tex);
		gs_effect_set_texture_srgb(prev, prev_tex);
	} else {
		gs_effect_set_texture(image, cur_tex);
		gs_effect_set_texture(prev, prev_tex);
	}

	gs_effect_set_float(multiplier_param, multiplier);
	gs_effect_set_int(field, s->deinterlace_top_first);
	gs_effect_set_vec2(dimensions, &size);

	gs_effect_set_bool(frame2,
			   obs_source_deinterlace_draw_frame2(obs->video.video_time, s->deinterlace_frame_ts,
							      s->deinterlace_offset, s->deinterlace_half_duration));

	while (gs_effect_loop(effect, tech_name))
		gs_draw_sprite(NULL, s->async_flip ? GS_FLIP_V : 0, s->async_width, s->async_height);

	gs_enable_framebuffer_srgb(previous);
}

static void enable_deinterlacing(obs_source_t *source, enum obs_deinterlace_mode mode)
{
	obs_enter_graphics();

	if (source->async_format != VIDEO_FORMAT_NONE && source->async_width != 0 && source->async_height != 0)
		set_deinterlace_texture_size(source);

	source->deinterlace_mode = mode;
	source->deinterlace_effect = get_effect(mode);

	pthread_mutex_lock(&source->async_mutex);
	if (source->prev_async_frame) {
		remove_async_frame(source, source->prev_async_frame);
		source->prev_async_frame = NULL;
	}
	pthread_mutex_unlock(&source->async_mutex);

	obs_leave_graphics();
}

static void disable_deinterlacing(obs_source_t *source)
{
	obs_enter_graphics();
	gs_texture_destroy(source->async_prev_textures[0]);
	gs_texture_destroy(source->async_prev_textures[1]);
	gs_texture_destroy(source->async_prev_textures[2]);
	gs_texrender_destroy(source->async_prev_texrender);
	source->deinterlace_mode = OBS_DEINTERLACE_MODE_DISABLE;
	source->async_prev_textures[0] = NULL;
	source->async_prev_textures[1] = NULL;
	source->async_prev_textures[2] = NULL;
	source->async_prev_texrender = NULL;
	obs_leave_graphics();
}

void obs_source_set_deinterlace_mode(obs_source_t *source, enum obs_deinterlace_mode mode)
{
	if (!obs_source_valid(source, "obs_source_set_deinterlace_mode"))
		return;
	/* A mode outside the enum is DISABLE (#2090): the decision says
	 * disable, or nothing when the source is disabled already. Enable and
	 * switch come only with a mode in the enum. */
	switch (obs_source_deinterlace_draw_mode_change(source->deinterlace_mode, mode)) {
	case OBS_SOURCE_DEINTERLACE_DRAW_MODE_SAME:
		return;
	case OBS_SOURCE_DEINTERLACE_DRAW_MODE_ENABLE:
		enable_deinterlacing(source, mode);
		break;
	case OBS_SOURCE_DEINTERLACE_DRAW_MODE_DISABLE:
		disable_deinterlacing(source);
		break;
	case OBS_SOURCE_DEINTERLACE_DRAW_MODE_SWITCH:
		obs_enter_graphics();
		source->deinterlace_mode = mode;
		source->deinterlace_effect = get_effect(mode);
		obs_leave_graphics();
		break;
	}
}

enum obs_deinterlace_mode obs_source_get_deinterlace_mode(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_set_deinterlace_mode") ? source->deinterlace_mode
									   : OBS_DEINTERLACE_MODE_DISABLE;
}

void obs_source_set_deinterlace_field_order(obs_source_t *source, enum obs_deinterlace_field_order field_order)
{
	if (!obs_source_valid(source, "obs_source_set_deinterlace_field_order"))
		return;

	source->deinterlace_top_first = field_order == OBS_DEINTERLACE_FIELD_ORDER_TOP;
}

enum obs_deinterlace_field_order obs_source_get_deinterlace_field_order(const obs_source_t *source)
{
	if (!obs_source_valid(source, "obs_source_set_deinterlace_field_order"))
		return OBS_DEINTERLACE_FIELD_ORDER_TOP;

	return source->deinterlace_top_first ? OBS_DEINTERLACE_FIELD_ORDER_TOP : OBS_DEINTERLACE_FIELD_ORDER_BOTTOM;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef FRAME_TOLERANCE
