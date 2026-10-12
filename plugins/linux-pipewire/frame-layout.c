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

/* The bytes of pixel data in one row of a plane, and its number of rows. */
struct plane_shape {
	uint64_t row_bytes;
	uint32_t rows;
};

static uint32_t get_plane_shapes(const struct obs_source_frame *frame, const struct obs_pw_video_format *format,
				 struct plane_shape planes[MAX_AV_PLANES])
{
	const uint64_t even_width = ((uint64_t)frame->width + 1) & ~(uint64_t)1;

	switch (frame->format) {
	case VIDEO_FORMAT_NV12:
		/* Luma, then interleaved chroma: half the rows, each as
		 * wide as a luma row. */
		planes[0] = (struct plane_shape){frame->width, frame->height};
		planes[1] = (struct plane_shape){even_width, (frame->height + 1) / 2};
		return 2;
	case VIDEO_FORMAT_YUY2:
		planes[0] = (struct plane_shape){even_width * 2, frame->height};
		return 1;
	default:
		planes[0] = (struct plane_shape){(uint64_t)frame->width * format->bpp, frame->height};
		return 1;
	}
}

/* The chunk's stride if the producer set one, else rows rounded up to 4
 * bytes, as OBS assumed before. */
static uint64_t chunk_stride(const struct spa_data *d, uint64_t fallback)
{
	return d->chunk->stride > 0 ? (uint64_t)d->chunk->stride : fallback;
}

/* Points plane p at `pos` bytes into the valid data of block d. The chunk is
 * written by the producer, so it is checked against the mapped block, and
 * the plane's rows against the chunk: libobs reads up to stride * rows. */
static bool place_plane(struct obs_source_frame *frame, uint32_t p, const struct spa_data *d, uint64_t pos,
			uint64_t stride, const struct plane_shape *shape)
{
	const struct spa_chunk *chunk = d->chunk;

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
		 * has the first plane's (NV12's planes have equal rows). */
		for (uint32_t p = 0; p < n_planes; p++) {
			const struct spa_data *d = &buffer->datas[p];

			if (!d->chunk ||
			    !place_plane(frame, p, d, 0, p == 0 ? stride : chunk_stride(d, stride), &planes[p]))
				return false;
		}
		return true;
	}

	if (buffer->n_datas == 1) {
		/* All planes in one block, one after the other, with the
		 * same stride (NV12's planes have equal rows). */
		uint64_t pos = 0;

		for (uint32_t p = 0; p < n_planes; p++) {
			if (!place_plane(frame, p, &buffer->datas[0], pos, stride, &planes[p]))
				return false;
			pos += stride * planes[p].rows;
		}
		return true;
	}

	return false;
}
