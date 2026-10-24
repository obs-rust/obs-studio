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

#include "libobs/build/libobs-ported-source-render-route.pch.h"

/* The source and filter render routing of obs-source.c, moved from
 * source_render_video(), obs_source_main_render(), can_bypass(),
 * obs_source_process_filter_begin_with_color_space(),
 * obs_source_process_filter_tech_end() and obs_source_skip_video_filter():
 * the decisions, on plain values. obs-source.c keeps every gs_* call and
 * texrender, the refs, the render calls, the sizes, the debug markers and
 * every field read in its order, passes the values in and acts on what comes
 * back. Declared in obs-source-render-route.h without EXPORT. In hybrid
 * libobs the Rust port (rust/obs-source) replaces this file. */

enum obs_source_render_route_filter obs_source_render_route_filter(bool has_target, bool has_parent)
{
	if (!has_target)
		return OBS_SOURCE_RENDER_ROUTE_FILTER_NO_TARGET;
	if (!has_parent)
		return OBS_SOURCE_RENDER_ROUTE_FILTER_NO_PARENT;
	return OBS_SOURCE_RENDER_ROUTE_FILTER_READY;
}

bool obs_source_render_route_video_reads_flags(int type)
{
	return (enum obs_source_type)type != OBS_SOURCE_TYPE_FILTER;
}

bool obs_source_render_route_video_none(int type, uint32_t flags)
{
	return (enum obs_source_type)type != OBS_SOURCE_TYPE_FILTER && (flags & OBS_SOURCE_VIDEO) == 0;
}

bool obs_source_render_route_async_reads_flags(int type)
{
	return (enum obs_source_type)type == OBS_SOURCE_TYPE_INPUT;
}

bool obs_source_render_route_async_reads_rendering_filter(int type, uint32_t flags)
{
	return (enum obs_source_type)type == OBS_SOURCE_TYPE_INPUT && (flags & OBS_SOURCE_ASYNC) != 0;
}

bool obs_source_render_route_async_update(int type, uint32_t flags, bool rendering_filter)
{
	return (enum obs_source_type)type == OBS_SOURCE_TYPE_INPUT && (flags & OBS_SOURCE_ASYNC) != 0 &&
	       !rendering_filter;
}

bool obs_source_render_route_reads_enabled(bool has_data)
{
	return has_data;
}

bool obs_source_render_route_inactive(bool has_data, bool enabled)
{
	return !has_data || !enabled;
}

enum obs_source_render_route_draw obs_source_render_route_draw(bool has_filters)
{
	return has_filters ? OBS_SOURCE_RENDER_ROUTE_DRAW_READ_RENDERING_FILTER
			   : OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER;
}

enum obs_source_render_route_draw obs_source_render_route_draw_next(enum obs_source_render_route_draw step, bool value)
{
	switch (step) {
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_RENDERING_FILTER:
		return !value ? OBS_SOURCE_RENDER_ROUTE_DRAW_FILTERS : OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER:
		return value ? OBS_SOURCE_RENDER_ROUTE_DRAW_MAIN : OBS_SOURCE_RENDER_ROUTE_DRAW_READ_FILTER_TARGET;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_FILTER_TARGET:
		return value ? OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET : OBS_SOURCE_RENDER_ROUTE_DRAW_READ_DEINTERLACING;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_DEINTERLACING:
		return value ? OBS_SOURCE_RENDER_ROUTE_DRAW_DEINTERLACE : OBS_SOURCE_RENDER_ROUTE_DRAW_ASYNC;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_VIDEO_RENDER:
		return value ? OBS_SOURCE_RENDER_ROUTE_DRAW_MAIN
			     : OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_DEINTERLACING;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_DEINTERLACING:
		return value ? OBS_SOURCE_RENDER_ROUTE_DRAW_DEINTERLACE : OBS_SOURCE_RENDER_ROUTE_DRAW_ASYNC;
	case OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_FILTERS:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_MAIN:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_DEINTERLACE:
	case OBS_SOURCE_RENDER_ROUTE_DRAW_ASYNC:
		break;
	}

	return step;
}

bool obs_source_render_route_main_custom_draw(uint32_t flags)
{
	return (flags & OBS_SOURCE_CUSTOM_DRAW) != 0;
}

bool obs_source_render_route_main_srgb_aware(uint32_t flags)
{
	return (flags & OBS_SOURCE_SRGB) != 0;
}

bool obs_source_render_route_main_reads_filters(bool has_filter_parent)
{
	return !has_filter_parent;
}

bool obs_source_render_route_main_default_effect(uint32_t flags, bool has_filter_parent, bool has_filters)
{
	bool custom_draw = (flags & OBS_SOURCE_CUSTOM_DRAW) != 0;
	return !has_filter_parent && !has_filters && !custom_draw;
}

bool obs_source_render_route_bypass_reads_space(bool target_is_parent, int allow_direct, uint32_t filter_flags,
						uint32_t parent_flags)
{
	return target_is_parent && ((enum obs_allow_direct_render)allow_direct == OBS_ALLOW_DIRECT_RENDERING) &&
	       ((parent_flags & OBS_SOURCE_CUSTOM_DRAW) == 0) && ((parent_flags & OBS_SOURCE_ASYNC) == 0) &&
	       ((filter_flags & OBS_SOURCE_SRGB) == (parent_flags & OBS_SOURCE_SRGB));
}

bool obs_source_render_route_bypass(bool target_is_parent, int allow_direct, uint32_t filter_flags,
				    uint32_t parent_flags, int space, int current_space)
{
	return obs_source_render_route_bypass_reads_space(target_is_parent, allow_direct, filter_flags, parent_flags) &&
	       (enum gs_color_space)space == (enum gs_color_space)current_space;
}

bool obs_source_render_route_size_skip(int cx, int cy)
{
	return !cx || !cy;
}

bool obs_source_render_route_texrender_stale(int texrender_format, int format)
{
	return (enum gs_color_format)texrender_format != (enum gs_color_format)format;
}

enum obs_source_render_route_draw obs_source_render_route_filter_render(bool target_is_parent, uint32_t parent_flags)
{
	bool custom_draw = (parent_flags & OBS_SOURCE_CUSTOM_DRAW) != 0;
	bool async = (parent_flags & OBS_SOURCE_ASYNC) != 0;

	if (target_is_parent && !custom_draw && !async)
		return OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT;
	else
		return OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET;
}

bool obs_source_render_route_tech_end_linear_srgb(uint32_t filter_flags)
{
	return (filter_flags & OBS_SOURCE_SRGB) != 0;
}

const char *obs_source_render_route_tech_name(const char *tech_name)
{
	return tech_name ? tech_name : "Draw";
}

enum obs_source_render_route_tech_end obs_source_render_route_tech_end(bool bypass_active)
{
	return bypass_active ? OBS_SOURCE_RENDER_ROUTE_TECH_END_BYPASS : OBS_SOURCE_RENDER_ROUTE_TECH_END_TEXTURE;
}

enum obs_source_render_route_draw obs_source_render_route_skip(bool target_is_parent, uint32_t parent_flags)
{
	bool custom_draw = (parent_flags & OBS_SOURCE_CUSTOM_DRAW) != 0;
	bool async = (parent_flags & OBS_SOURCE_ASYNC) != 0;

	if (target_is_parent) {
		if (!custom_draw && !async)
			return OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT;
		else
			return OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_VIDEO_RENDER;

	} else {
		return OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET;
	}
}
