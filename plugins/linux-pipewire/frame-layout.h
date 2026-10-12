/* frame-layout.h
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

#pragma once

#include <stdbool.h>

#include <obs.h>
#include <spa/buffer/buffer.h>

#include "formats.h"

/* Points frame->data and frame->linesize at the planes of a mapped PipeWire
 * buffer. frame->width and frame->height must be set. Kept free of stream
 * state so it can be tested on its own. Returns false when the buffer cannot
 * be used, and the frame must be dropped. */
bool obs_pw_frame_layout(struct obs_source_frame *frame, const struct spa_buffer *buffer,
			 const struct obs_pw_video_format *format);
