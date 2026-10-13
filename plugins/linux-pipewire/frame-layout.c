/* frame-layout.c
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#include "frame-layout.h"

#include <spa/utils/defs.h>

/* The bytes of pixel data in one row of a plane, its number of rows, the
 * divisor that gives its stride from the first plane's when the producer
 * gives none of its own, and the frame plane it becomes. */
struct plane_shape {
	uint64_t row_bytes;
	uint32_t rows;
	uint32_t stride_div;
	uint32_t frame_plane;
};

static uint32_t get_plane_shapes(const struct obs_source_frame *frame, const struct obs_pw_video_format *format,
				 struct plane_shape planes[MAX_AV_PLANES])
{
	const uint64_t even_width = ((uint64_t)frame->width + 1) & ~(uint64_t)1;
	const uint32_t half_height = (frame->height + 1) / 2;

	switch (frame->format) {
	case VIDEO_FORMAT_NV12:
		/* Luma, then interleaved chroma: half the rows, each as
		 * wide as a luma row. */
		planes[0] = (struct plane_shape){frame->width, frame->height, 1, 0};
		planes[1] = (struct plane_shape){even_width, half_height, 1, 1};
		return 2;
	case VIDEO_FORMAT_I420: {
		/* Luma, then U and V: half the rows, half as wide, with
		 * half the luma stride rounded up (V4L2's YU12 layout: at
		 * an odd width a chroma row has (width + 1) / 2 bytes).
		 * YV12 has V first. */
		const bool yv12 = format->spa_format == SPA_VIDEO_FORMAT_YV12;

		planes[0] = (struct plane_shape){frame->width, frame->height, 1, 0};
		planes[1] = (struct plane_shape){even_width / 2, half_height, 2, yv12 ? 2 : 1};
		planes[2] = (struct plane_shape){even_width / 2, half_height, 2, yv12 ? 1 : 2};
		return 3;
	}
	case VIDEO_FORMAT_YUY2:
		planes[0] = (struct plane_shape){even_width * 2, frame->height, 1, 0};
		return 1;
	default:
		planes[0] = (struct plane_shape){(uint64_t)frame->width * format->bpp, frame->height, 1, 0};
		return 1;
	}
}

/* The chunk's stride if the producer set one, else rows rounded up to 4
 * bytes, as OBS assumed before. */
static uint64_t chunk_stride(const struct spa_data *d, uint64_t fallback)
{
	return d->chunk->stride > 0 ? (uint64_t)d->chunk->stride : fallback;
}

/* The stride of a plane without one of its own: the first plane's, divided
 * and rounded up for planar chroma. A tightly packed luma row of an odd
 * width w has stride w, and its chroma rows (w + 1) / 2 bytes. */
static uint64_t derived_stride(uint64_t stride, const struct plane_shape *shape)
{
	return (stride + shape->stride_div - 1) / shape->stride_div;
}

/* Points the frame plane of `shape` at `pos` bytes into the valid data of
 * block d. The chunk is written by the producer, so it is checked against
 * the mapped block, and the plane's rows against the chunk: libobs reads up
 * to stride * rows. */
static bool place_plane(struct obs_source_frame *frame, const struct spa_data *d, uint64_t pos, uint64_t stride,
			const struct plane_shape *shape)
{
	const struct spa_chunk *chunk = d->chunk;
	const uint32_t p = shape->frame_plane;

	if (!d->data || stride < shape->row_bytes || stride > UINT32_MAX)
		return false;
	if ((uint64_t)chunk->offset + chunk->size > d->maxsize)
		return false;
	if (pos + stride * shape->rows > chunk->size)
		return false;

	frame->data[p] = (uint8_t *)d->data + chunk->offset + pos;
	frame->linesize[p] = (uint32_t)stride;
	return true;
}

bool obs_pw_frame_layout(struct obs_source_frame *frame, const struct spa_buffer *buffer,
			 const struct obs_pw_video_format *format)
{
	struct plane_shape planes[MAX_AV_PLANES];
	uint32_t n_planes = get_plane_shapes(frame, format, planes);
	uint64_t stride;

	if (buffer->n_datas == 0 || !buffer->datas[0].chunk)
		return false;

	stride = chunk_stride(&buffer->datas[0], SPA_ROUND_UP_N(planes[0].row_bytes, 4));

	if (buffer->n_datas >= n_planes) {
		/* One block per plane. A plane without a stride of its own
		 * has the first plane's, halved (rounded up) for planar
		 * chroma. */
		for (uint32_t p = 0; p < n_planes; p++) {
			const struct spa_data *d = &buffer->datas[p];
			const uint64_t plane_stride = derived_stride(stride, &planes[p]);

			if (!d->chunk ||
			    !place_plane(frame, d, 0, p == 0 ? stride : chunk_stride(d, plane_stride), &planes[p]))
				return false;
		}
		return true;
	}

	if (buffer->n_datas == 1) {
		/* All planes in one block, one after the other. The chunk
		 * stride is the first plane's; NV12's chroma rows have the
		 * same, planar chroma rows half of it, rounded up. */
		uint64_t pos = 0;

		for (uint32_t p = 0; p < n_planes; p++) {
			const uint64_t plane_stride = derived_stride(stride, &planes[p]);

			if (!place_plane(frame, &buffer->datas[0], pos, plane_stride, &planes[p]))
				return false;
			pos += plane_stride * planes[p].rows;
		}
		return true;
	}

	return false;
}
