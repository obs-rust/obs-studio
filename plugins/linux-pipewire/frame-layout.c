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

bool obs_pw_frame_layout(struct obs_source_frame *frame, const struct spa_buffer *buffer,
			 const struct obs_pw_video_format *format)
{
	frame->linesize[0] = SPA_ROUND_UP_N(frame->width * format->bpp, 4);

	for (uint32_t i = 0; i < buffer->n_datas && i < MAX_AV_PLANES; i++) {
		frame->data[i] = buffer->datas[i].data;
		if (frame->data[i] == NULL)
			return false;
	}

	return true;
}
