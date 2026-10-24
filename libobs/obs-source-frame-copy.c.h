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

#include "libobs/build/libobs-ported-source-frame-copy.pch.h"

/* The async frame copy plan and cache decisions, moved from obs-source.c:
 * the plane copies of copy_frame_data_line(), copy_frame_data_plane() and
 * copy_frame_data(), async_texture_changed() and the queue limit of
 * cache_video(), one entry of clean_cache(), the full-range rule of the
 * obs_source_output_video(), _preload_video() and _set_video_frame()
 * families, and preload_frame_changed(), on plain values. obs-source.c runs
 * the copies and keeps the frames, the cache and the locks. Declared in
 * obs-source-frame-copy.h without EXPORT. */

_Static_assert(sizeof(enum video_format) == sizeof(int), "enum video_format is passed as int");
_Static_assert(sizeof(enum video_range_type) == sizeof(int), "enum video_range_type is passed as int");

/* copy_frame_data_plane(): one copy when the line sizes are equal, else
 * lines rows of the smaller line size. */
static inline void frame_copy_plane(struct obs_source_frame_copy_plan *plan, uint32_t plane, uint32_t lines,
				    const struct obs_source_frame_copy_lines *src,
				    const struct obs_source_frame_copy_lines *dst)
{
	struct obs_source_frame_plane_copy *copy = &plan->planes[plan->count++];
	const uint32_t src_linesize = src->linesize[plane];
	const uint32_t dst_linesize = dst->linesize[plane];

	copy->rows = lines;
	copy->src_stride = src_linesize;
	copy->dst_stride = dst_linesize;

	if (dst_linesize != src_linesize) {
		copy->whole = false;
		copy->bytes = dst_linesize < src_linesize ? dst_linesize : src_linesize;
	} else {
		copy->whole = true;
		copy->bytes = (size_t)dst_linesize * (size_t)lines;
	}
}

struct obs_source_frame_copy_plan obs_source_frame_copy_plan(int format, bool full_range, uint32_t height,
							     struct obs_source_frame_copy_lines src,
							     struct obs_source_frame_copy_lines dst)
{
	struct obs_source_frame_copy_plan plan;
	memset(&plan, 0, sizeof(plan));

	plan.color_range = !full_range;

	switch ((enum video_format)format) {
	case VIDEO_FORMAT_I420:
	case VIDEO_FORMAT_I010: {
		const uint32_t half_height = (height + 1) / 2;
		frame_copy_plane(&plan, 0, height, &src, &dst);
		frame_copy_plane(&plan, 1, half_height, &src, &dst);
		frame_copy_plane(&plan, 2, half_height, &src, &dst);
		break;
	}

	case VIDEO_FORMAT_NV12:
	case VIDEO_FORMAT_P010: {
		const uint32_t half_height = (height + 1) / 2;
		frame_copy_plane(&plan, 0, height, &src, &dst);
		frame_copy_plane(&plan, 1, half_height, &src, &dst);
		break;
	}

	case VIDEO_FORMAT_I444:
	case VIDEO_FORMAT_I422:
	case VIDEO_FORMAT_I210:
	case VIDEO_FORMAT_I412:
		frame_copy_plane(&plan, 0, height, &src, &dst);
		frame_copy_plane(&plan, 1, height, &src, &dst);
		frame_copy_plane(&plan, 2, height, &src, &dst);
		break;

	case VIDEO_FORMAT_YVYU:
	case VIDEO_FORMAT_YUY2:
	case VIDEO_FORMAT_UYVY:
	case VIDEO_FORMAT_NONE:
	case VIDEO_FORMAT_RGBA:
	case VIDEO_FORMAT_BGRA:
	case VIDEO_FORMAT_BGRX:
	case VIDEO_FORMAT_Y800:
	case VIDEO_FORMAT_BGR3:
	case VIDEO_FORMAT_AYUV:
	case VIDEO_FORMAT_V210:
	case VIDEO_FORMAT_R10L:
		frame_copy_plane(&plan, 0, height, &src, &dst);
		break;

	case VIDEO_FORMAT_I40A: {
		const uint32_t half_height = (height + 1) / 2;
		frame_copy_plane(&plan, 0, height, &src, &dst);
		frame_copy_plane(&plan, 1, half_height, &src, &dst);
		frame_copy_plane(&plan, 2, half_height, &src, &dst);
		frame_copy_plane(&plan, 3, height, &src, &dst);
		break;
	}

	case VIDEO_FORMAT_I42A:
	case VIDEO_FORMAT_YUVA:
	case VIDEO_FORMAT_YA2L:
		frame_copy_plane(&plan, 0, height, &src, &dst);
		frame_copy_plane(&plan, 1, height, &src, &dst);
		frame_copy_plane(&plan, 2, height, &src, &dst);
		frame_copy_plane(&plan, 3, height, &src, &dst);
		break;

	case VIDEO_FORMAT_P216:
	case VIDEO_FORMAT_P416:
		/* Unimplemented */
		break;
	}

	return plan;
}

/* async_texture_changed(): the size or the convert type differs. */
static inline bool frame_copy_texture_changed(struct obs_source_frame_copy_key cache,
					      struct obs_source_frame_copy_key frame)
{
	return cache.width != frame.width || cache.height != frame.height || cache.convert_type != frame.convert_type;
}

enum obs_source_frame_cache_step obs_source_frame_copy_cache_step(size_t queued, struct obs_source_frame_copy_key cache,
								  struct obs_source_frame_copy_key frame)
{
	if (queued >= OBS_SOURCE_FRAME_COPY_MAX_ASYNC_FRAMES)
		return OBS_SOURCE_FRAME_CACHE_OVERFLOW;

	if (frame_copy_texture_changed(cache, frame))
		return OBS_SOURCE_FRAME_CACHE_RESET;

	return OBS_SOURCE_FRAME_CACHE_KEEP;
}

struct obs_source_frame_cache_age obs_source_frame_copy_cache_clean(bool used, long unused_count)
{
	struct obs_source_frame_cache_age age;
	memset(&age, 0, sizeof(age));

	age.unused_count = unused_count;
	if (!used) {
		age.counted = true;
		if (++age.unused_count == OBS_SOURCE_FRAME_COPY_MAX_UNUSED)
			age.drop = true;
	}

	return age;
}

bool obs_source_frame_copy_full_range(int format, bool full_range)
{
	return format_is_yuv((enum video_format)format) ? full_range : true;
}

bool obs_source_frame_copy_full_range2(int format, int range)
{
	return resolve_video_range((enum video_format)format, (enum video_range_type)range) == VIDEO_RANGE_FULL;
}

bool obs_source_frame_copy_preload_changed(bool has_preload, struct obs_source_frame_copy_shape preload,
					   struct obs_source_frame_copy_shape in)
{
	if (!has_preload)
		return true;

	return in.width != preload.width || in.height != preload.height || in.format != preload.format;
}
