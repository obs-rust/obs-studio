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
#include <stddef.h>
#include <stdint.h>

/* The async frame copy plan and cache decisions of obs-source.c, on plain
 * values (obs-source-frame-copy.c):
 * - obs_source_frame_copy_plan(): what copy_frame_data() copies: the colour
 *   range only when the frame is not full range, and for each plane of the
 *   format the rows ((height + 1) / 2 for 4:2:0 chroma), as one copy when
 *   the two line sizes are equal, else row by row with the smaller line
 *   size. P216, P416 and values outside the enum copy no plane;
 * - obs_source_frame_copy_cache_step(): cache_video()'s first two
 *   decisions: with MAX_ASYNC_FRAMES or more frames queued, clear the cache
 *   and refuse the frame; when the frame's size or convert type differs from
 *   the cache's (async_texture_changed()), clear the cache and take the
 *   frame's size; else keep the cache;
 * - obs_source_frame_copy_cache_clean(): one entry of clean_cache(): an
 *   unused entry counts one more call, and is dropped at
 *   MAX_UNUSED_FRAME_DURATION;
 * - obs_source_frame_copy_full_range() and _full_range2(): the range of a
 *   frame given to obs_source_output_video(), obs_source_preload_video()
 *   and obs_source_set_video_frame() (a frame that is not YUV is full
 *   range), and to their *2() variants (the range resolve_video_range()
 *   gives is VIDEO_RANGE_FULL);
 * - obs_source_frame_copy_preload_changed(): preload_frame_changed():
 *   there is no preload frame, or its size or format differs.
 *
 * obs-source.c keeps every memcpy() (it runs the plan), the metadata copy,
 * obs_source_frame_create() and _destroy(), the async_cache and
 * async_frames darrays and the walk that picks the first unused entry,
 * async_mutex, the refcount atomics, and get_convert_type() (it passes the
 * two convert types). Nothing here takes a lock, calls into libobs, touches
 * a frame or keeps state.
 *
 * The enums cross as int, so this header needs no media-io header.
 * obs-source.c checks the sizes and the constants.
 *
 * Hidden in libobs: not EXPORT. */

/* The planes a plan can copy: I40A, I42A, YUVA and YA2L have 4. */
#define OBS_SOURCE_FRAME_COPY_PLANES 4

/* MAX_ASYNC_FRAMES of obs-source.c: the queued frames at which
 * cache_video() clears the cache and refuses the frame. */
#define OBS_SOURCE_FRAME_COPY_MAX_ASYNC_FRAMES 30

/* MAX_UNUSED_FRAME_DURATION of obs-source.c: the clean_cache() calls after
 * which an unused cache entry is dropped. */
#define OBS_SOURCE_FRAME_COPY_MAX_UNUSED 5

/* The first OBS_SOURCE_FRAME_COPY_PLANES line sizes of a frame. */
struct obs_source_frame_copy_lines {
	uint32_t linesize[OBS_SOURCE_FRAME_COPY_PLANES];
};

/* The copy of one plane, from the start of the source plane into the start
 * of the destination plane. */
struct obs_source_frame_plane_copy {
	/* The two line sizes are equal: one memcpy() of bytes. */
	bool whole;
	/* whole: the line size times rows, in size_t. Otherwise the bytes of
	 * each row, the smaller line size. */
	size_t bytes;
	/* The rows of the plane. Not whole: row y is copied from
	 * y * src_stride into y * dst_stride, each product in uint32_t (it
	 * wraps). Set either way. */
	uint32_t rows;
	/* The source and destination line sizes. */
	uint32_t src_stride;
	uint32_t dst_stride;
};

/* The copies of copy_frame_data(): copy i is plane i, in this order, after
 * the colour range when color_range is set. */
struct obs_source_frame_copy_plan {
	/* Copy color_range_min and color_range_max: the frame is not full
	 * range. */
	bool color_range;
	/* The planes to copy, at most OBS_SOURCE_FRAME_COPY_PLANES. */
	size_t count;
	struct obs_source_frame_plane_copy planes[OBS_SOURCE_FRAME_COPY_PLANES];
};

/* What async_texture_changed() compares: a size and a convert type (an
 * enum convert_type value, from get_convert_type()). */
struct obs_source_frame_copy_key {
	uint32_t width;
	uint32_t height;
	int convert_type;
};

/* What cache_video() does before it picks a frame. */
enum obs_source_frame_cache_step {
	/* Keep the cache: its size and convert type match the frame's. */
	OBS_SOURCE_FRAME_CACHE_KEEP,
	/* free_async_cache(), then take the frame's width and height. */
	OBS_SOURCE_FRAME_CACHE_RESET,
	/* free_async_cache(), last_frame_ts = 0, and no frame: the queue is
	 * full. */
	OBS_SOURCE_FRAME_CACHE_OVERFLOW,
};

/* One entry of clean_cache(). */
struct obs_source_frame_cache_age {
	/* The entry is unused: store unused_count into it. */
	bool counted;
	/* The entry's count after this call. */
	long unused_count;
	/* Destroy the entry's frame and erase the entry. */
	bool drop;
};

/* What preload_frame_changed() compares of a frame: an enum video_format
 * value, the width and the height. */
struct obs_source_frame_copy_shape {
	int format;
	uint32_t width;
	uint32_t height;
};

/* copy_frame_data(): the copies from a frame of format (an enum
 * video_format value) and range full_range into one height rows high, with
 * the source's line sizes src and the destination's dst. */
struct obs_source_frame_copy_plan obs_source_frame_copy_plan(int format, bool full_range, uint32_t height,
							     struct obs_source_frame_copy_lines src,
							     struct obs_source_frame_copy_lines dst);

/* cache_video(): queued is async_frames.num, cache the cache's size
 * (async_cache_width and _height) and the convert type of its format, range
 * and transfer, frame the frame's. */
enum obs_source_frame_cache_step obs_source_frame_copy_cache_step(size_t queued, struct obs_source_frame_copy_key cache,
								  struct obs_source_frame_copy_key frame);

/* clean_cache(): an entry's used flag and unused_count. */
struct obs_source_frame_cache_age obs_source_frame_copy_cache_clean(bool used, long unused_count);

/* obs_source_output_video(), obs_source_preload_video() and
 * obs_source_set_video_frame(): the full_range a frame of format is given
 * (format is an enum video_format value). */
bool obs_source_frame_copy_full_range(int format, bool full_range);

/* obs_source_output_video2(), obs_source_preload_video2() and
 * obs_source_set_video_frame2(): the full_range a frame of format with
 * range (an enum video_range_type value) is given. */
bool obs_source_frame_copy_full_range2(int format, int range);

/* preload_frame_changed(): has_preload is async_preload_frame != NULL,
 * preload its shape (not looked at without one), in the new frame's. */
bool obs_source_frame_copy_preload_changed(bool has_preload, struct obs_source_frame_copy_shape preload,
					   struct obs_source_frame_copy_shape in);
