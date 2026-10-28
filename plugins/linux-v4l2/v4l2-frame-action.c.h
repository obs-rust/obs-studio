/*
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
*/

#include "plugins/linux-v4l2/build/linux-v4l2-nix.pch.h"

enum v4l2_frame_action v4l2_frame_action(struct obs_source_frame *out, int pixfmt, const struct v4l2_buffer *buf,
					 uint8_t *start, const size_t *plane_offsets, struct v4l2_decoder *decoder)
{
	if (buf->flags & V4L2_BUF_FLAG_ERROR)
		return V4L2_FRAME_SKIP;

	if (pixfmt == V4L2_PIX_FMT_MJPEG || pixfmt == V4L2_PIX_FMT_H264) {
		/* libavcodec rejects an empty packet that has data (EINVAL) */
		if (!buf->bytesused)
			return V4L2_FRAME_SKIP;

		int r = v4l2_decode_frame(out, start, buf->bytesused, decoder);
		/* no new picture: out still points at the previous one, which
		 * the decoder has released */
		if (r == AVERROR(EAGAIN))
			return V4L2_FRAME_SKIP;
		if (r < 0)
			return V4L2_FRAME_DROP;
	} else {
		for (uint i = 0; i < MAX_AV_PLANES; ++i)
			out->data[i] = start + plane_offsets[i];
	}

	return V4L2_FRAME_OUTPUT;
}
