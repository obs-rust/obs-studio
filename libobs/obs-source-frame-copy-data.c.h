#include "libobs/build/libobs.pch.h"

_Static_assert(OBS_SOURCE_FRAME_COPY_PLANES <= MAX_AV_PLANES, "a copy plan's planes are frame planes");

static inline struct obs_source_frame_copy_lines frame_copy_lines(const struct obs_source_frame *frame)
{
	struct obs_source_frame_copy_lines lines;

	for (size_t i = 0; i < OBS_SOURCE_FRAME_COPY_PLANES; i++)
		lines.linesize[i] = frame->linesize[i];

	return lines;
}

/* Runs one copy of the plan: copy_frame_data_plane() and
 * copy_frame_data_line(). */
static inline void copy_frame_data_plane(struct obs_source_frame *dst, const struct obs_source_frame *src, size_t plane,
					 const struct obs_source_frame_plane_copy *copy)
{
	if (!copy->whole) {
		for (uint32_t y = 0; y < copy->rows; y++) {
			uint32_t pos_src = y * copy->src_stride;
			uint32_t pos_dst = y * copy->dst_stride;

			memcpy(dst->data[plane] + pos_dst, src->data[plane] + pos_src, copy->bytes);
		}
	} else {
		memcpy(dst->data[plane], src->data[plane], copy->bytes);
	}
}

static void copy_frame_data(struct obs_source_frame *dst, const struct obs_source_frame *src)
{
	dst->flip = src->flip;
	dst->flags = src->flags;
	dst->trc = src->trc;
	dst->full_range = src->full_range;
	dst->max_luminance = src->max_luminance;
	dst->timestamp = src->timestamp;
	memcpy(dst->color_matrix, src->color_matrix, sizeof(float) * 16);

	const struct obs_source_frame_copy_plan plan = obs_source_frame_copy_plan(
		(int)src->format, dst->full_range, dst->height, frame_copy_lines(src), frame_copy_lines(dst));

	if (plan.color_range) {
		size_t const size = sizeof(float) * 3;
		memcpy(dst->color_range_min, src->color_range_min, size);
		memcpy(dst->color_range_max, src->color_range_max, size);
	}

	for (size_t i = 0; i < plan.count && i < OBS_SOURCE_FRAME_COPY_PLANES; i++)
		copy_frame_data_plane(dst, src, i, &plan.planes[i]);
}

void obs_source_frame_copy(struct obs_source_frame *dst, const struct obs_source_frame *src)
{
	copy_frame_data(dst, src);
}
