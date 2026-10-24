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

#include <stdbool.h>
#include <stdint.h>

/* The source and filter render routing of obs-source.c, on plain values
 * (obs-source-render-route.c):
 * - source_render_video(): skip a source that is neither video nor a filter,
 *   update the async textures of an async input not drawn as a filter, skip
 *   a source with no data or one that is disabled, then pick the draw:
 *   filters, main render, filter target, deinterlace or async video;
 * - obs_source_main_render(): the default-effect rule, the sRGB-aware rule
 *   and the effect passed to the source (none for custom draw);
 * - obs_source_process_filter_begin_with_color_space(): no target or no
 *   parent, the direct-render bypass (can_bypass()), the skip at zero size,
 *   texrender reuse when its format matches, and default render versus
 *   video render of the target;
 * - obs_source_process_filter_tech_end(): bypass or texture, the technique
 *   name ("Draw" when none is given) and the linear sRGB flag;
 * - obs_source_skip_video_filter(): the NULL parent and target guard (#2177)
 *   and the draw of the target.
 *
 * obs-source.c keeps every gs_* call and texrender, the source refs, the
 * render functions themselves, get_base_width() and get_base_height() (it
 * passes the sizes), the debug markers and every field read, in their order
 * and under their conditions. Where the old code read a field or called a
 * function only on one side of a short circuit, a _reads_ function says
 * whether to read it, and a draw route that needs a field it has not read
 * yet asks for it with a READ_ step: obs-source.c reads that one value,
 * passes it to obs_source_render_route_draw_next() and goes on. None of
 * these functions takes a lock, calls into libobs or keeps state.
 *
 * Source types (enum obs_source_type), the direct render choice (enum
 * obs_allow_direct_render), colour spaces (enum gs_color_space) and colour
 * formats (enum gs_color_format) are passed as int; obs-source.c checks the
 * sizes. A value outside an enum takes the path it took before. Output flags
 * are the uint32_t OBS_SOURCE_* bits.
 *
 * Hidden in libobs: not EXPORT. */

/* What a filter render step finds, in the order the old code checked. */
enum obs_source_render_route_filter {
	/* a target and a parent: go on */
	OBS_SOURCE_RENDER_ROUTE_FILTER_READY,
	/* no target (checked first) */
	OBS_SOURCE_RENDER_ROUTE_FILTER_NO_TARGET,
	/* a target but no parent */
	OBS_SOURCE_RENDER_ROUTE_FILTER_NO_PARENT,
};

/* A draw, or the value to read before the draw is known. In
 * source_render_video() the source draws itself; in
 * obs_source_skip_video_filter() and
 * obs_source_process_filter_begin_with_color_space() the filter draws its
 * target. */
enum obs_source_render_route_draw {
	/* obs_source_default_render(target) */
	OBS_SOURCE_RENDER_ROUTE_DRAW_DEFAULT,
	/* obs_source_render_filters(source) */
	OBS_SOURCE_RENDER_ROUTE_DRAW_FILTERS,
	/* obs_source_main_render() of the source or the target */
	OBS_SOURCE_RENDER_ROUTE_DRAW_MAIN,
	/* obs_source_video_render() of the filter target */
	OBS_SOURCE_RENDER_ROUTE_DRAW_TARGET,
	/* deinterlace_render() of the source or the target */
	OBS_SOURCE_RENDER_ROUTE_DRAW_DEINTERLACE,
	/* obs_source_render_async_video() of the source or the target */
	OBS_SOURCE_RENDER_ROUTE_DRAW_ASYNC,
	/* read source->rendering_filter */
	OBS_SOURCE_RENDER_ROUTE_DRAW_READ_RENDERING_FILTER,
	/* read source->info.video_render != NULL */
	OBS_SOURCE_RENDER_ROUTE_DRAW_READ_VIDEO_RENDER,
	/* read source->filter_target != NULL */
	OBS_SOURCE_RENDER_ROUTE_DRAW_READ_FILTER_TARGET,
	/* call deinterlacing_enabled(source) */
	OBS_SOURCE_RENDER_ROUTE_DRAW_READ_DEINTERLACING,
	/* read target->info.video_render != NULL */
	OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_VIDEO_RENDER,
	/* call deinterlacing_enabled(target) */
	OBS_SOURCE_RENDER_ROUTE_DRAW_READ_TARGET_DEINTERLACING,
};

/* What obs_source_process_filter_tech_end() draws. */
enum obs_source_render_route_tech_end {
	/* render_filter_bypass(): the target, straight through the effect */
	OBS_SOURCE_RENDER_ROUTE_TECH_END_BYPASS,
	/* render_filter_tex(): the filter's texrender, if it has a texture */
	OBS_SOURCE_RENDER_ROUTE_TECH_END_TEXTURE,
};

/* --- filters ----------------------------------------------------------- */

/* The target and parent check of obs_source_process_filter_begin*(),
 * obs_source_process_filter_tech_end() and obs_source_skip_video_filter():
 * no target first, then no parent. A filter removed while it renders has
 * neither (#2177). */
enum obs_source_render_route_filter obs_source_render_route_filter(bool has_target, bool has_parent);

/* --- source_render_video() ------------------------------------------------ */

/* Whether the "no video" check reads the source's output flags: for any
 * type but OBS_SOURCE_TYPE_FILTER. */
bool obs_source_render_route_video_reads_flags(int type);

/* A source that is neither a filter nor OBS_SOURCE_VIDEO draws nothing (a
 * filter in a chain skips itself). flags is 0 when not read. */
bool obs_source_render_route_video_none(int type, uint32_t flags);

/* Whether the async check reads the output flags (again): for an
 * OBS_SOURCE_TYPE_INPUT. */
bool obs_source_render_route_async_reads_flags(int type);

/* Whether the async check reads source->rendering_filter: an
 * OBS_SOURCE_TYPE_INPUT with OBS_SOURCE_ASYNC. */
bool obs_source_render_route_async_reads_rendering_filter(int type, uint32_t flags);

/* Whether to update the async video textures (and first the deinterlace
 * ones, when deinterlacing is on): an async input not being drawn by its own
 * filter chain. flags is 0 and rendering_filter false when not read. */
bool obs_source_render_route_async_update(int type, uint32_t flags, bool rendering_filter);

/* Whether the active check reads source->enabled: only with data. */
bool obs_source_render_route_reads_enabled(bool has_data);

/* A source with no data, or a disabled one, draws nothing (a filter in a
 * chain skips itself). enabled is false when not read. */
bool obs_source_render_route_inactive(bool has_data, bool enabled);

/* The first step of the draw: READ_RENDERING_FILTER with filters, else
 * READ_VIDEO_RENDER. */
enum obs_source_render_route_draw obs_source_render_route_draw(bool has_filters);

/* The next step after reading the value a READ_ step asks for:
 * - READ_RENDERING_FILTER: FILTERS unless already rendering the chain, else
 *   READ_VIDEO_RENDER;
 * - READ_VIDEO_RENDER: MAIN with a video_render callback, else
 *   READ_FILTER_TARGET;
 * - READ_FILTER_TARGET: TARGET with a filter target, else READ_DEINTERLACING;
 * - READ_DEINTERLACING: DEINTERLACE when on, else ASYNC;
 * - READ_TARGET_VIDEO_RENDER: MAIN with a video_render callback, else
 *   READ_TARGET_DEINTERLACING;
 * - READ_TARGET_DEINTERLACING: DEINTERLACE when on, else ASYNC.
 * A draw, or a value outside the enum, comes back unchanged. */
enum obs_source_render_route_draw obs_source_render_route_draw_next(enum obs_source_render_route_draw step, bool value);

/* --- obs_source_main_render() --------------------------------------------- */

/* OBS_SOURCE_CUSTOM_DRAW: the source gets no effect (NULL instead of
 * gs_get_effect(), which is not called). */
bool obs_source_render_route_main_custom_draw(uint32_t flags);

/* OBS_SOURCE_SRGB: the source keeps the linear sRGB flag; otherwise
 * obs_source_main_render() saves it, clears it and restores it. */
bool obs_source_render_route_main_srgb_aware(uint32_t flags);

/* Whether the default-effect rule reads source->filters.num: only for a
 * source that is not a filter. */
bool obs_source_render_route_main_reads_filters(bool has_filter_parent);

/* The default-effect rule: not a filter, no filters and not custom draw,
 * so obs_source_default_render() draws it with the "Draw" technique.
 * has_filters is false when not read. */
bool obs_source_render_route_main_default_effect(uint32_t flags, bool has_filter_parent, bool has_filters);

/* --- obs_source_process_filter_begin_with_color_space() --------------------- */

/* Whether can_bypass() reads gs_get_color_space(): the target is the parent,
 * direct rendering is allowed, the parent is neither custom draw nor async
 * and the filter and the parent agree on OBS_SOURCE_SRGB. */
bool obs_source_render_route_bypass_reads_space(bool target_is_parent, int allow_direct, uint32_t filter_flags,
						uint32_t parent_flags);

/* can_bypass(): the filter renders the parent directly through its effect,
 * with no texrender, when obs_source_render_route_bypass_reads_space() holds
 * and the requested space is the current one. current_space is
 * gs_get_color_space(), any value when not read. */
bool obs_source_render_route_bypass(bool target_is_parent, int allow_direct, uint32_t filter_flags,
				    uint32_t parent_flags, int space, int current_space);

/* A target of zero width or height: the filter skips itself and draws
 * nothing more. */
bool obs_source_render_route_size_skip(int cx, int cy);

/* The filter's texrender has another format than the one asked for: destroy
 * it (a new one is created). */
bool obs_source_render_route_texrender_stale(int texrender_format, int format);

/* How the filter draws its target into the texrender: DEFAULT when the
 * target is the parent and the parent is neither custom draw nor async,
 * TARGET otherwise. */
enum obs_source_render_route_draw obs_source_render_route_filter_render(bool target_is_parent, uint32_t parent_flags);

/* --- obs_source_process_filter_tech_end() --------------------------------- */

/* The linear sRGB flag the filter's draw runs with: its OBS_SOURCE_SRGB. */
bool obs_source_render_route_tech_end_linear_srgb(uint32_t filter_flags);

/* The technique to draw with: tech_name, or "Draw" when it is NULL. The
 * pointer is passed through and never read. */
const char *obs_source_render_route_tech_name(const char *tech_name);

/* Bypass, as obs_source_process_filter_begin*() decided, or the texture. */
enum obs_source_render_route_tech_end obs_source_render_route_tech_end(bool bypass_active);

/* --- obs_source_skip_video_filter() --------------------------------------- */

/* The first step of a skipped filter's draw: TARGET when the target is
 * another filter, DEFAULT for a parent that is neither custom draw nor
 * async, else READ_TARGET_VIDEO_RENDER (then main render, deinterlace or
 * async video of the parent). */
enum obs_source_render_route_draw obs_source_render_route_skip(bool target_is_parent, uint32_t parent_flags);
