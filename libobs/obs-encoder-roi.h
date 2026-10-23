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

#pragma once

#include <stdbool.h>
#include <stdint.h>

/* The encoder region of interest (ROI) arithmetic of obs-encoder.c.h: which
 * ROIs obs_encoder_add_roi() stores, and how obs_encoder_enum_roi() scales
 * them to the encoder's scaled size. The encoder keeps the lock, the list,
 * roi_increment, the walk (last added first) and the callback; these take
 * and return plain values.
 *
 * obs_encoder_roi_admit: whether an encoder with `caps` stores `roi` (not
 * NULL): the encoder has OBS_ENCODER_CAP_ROI, the ROI is at least 16 x 16
 * (the smallest block), it is not upside down (bottom above top, or right
 * left of left) and its priority is in [-1, 1] (not NaN).
 *
 * obs_encoder_roi_scale_factors: the factors from the video's size
 * (`width` x `height`) to the scaled size. False, with the factors
 * unchanged, when the video has no size: then no ROI is passed on.
 *
 * obs_encoder_roi_scale: `roi` with its edges scaled, its priority kept.
 * Each edge is multiplied in float and truncated toward 0, then the
 * nearest uint32_t: 0 for NaN and below 0, UINT32_MAX from 2^32 up.
 *
 * Not EXPORT: obs-encoder.c.h calls them inside libobs. */

struct obs_encoder_roi;

bool obs_encoder_roi_admit(uint32_t caps, const struct obs_encoder_roi *roi);

bool obs_encoder_roi_scale_factors(uint32_t scaled_width, uint32_t scaled_height, uint32_t width, uint32_t height,
				   float *scale_x, float *scale_y);

void obs_encoder_roi_scale(const struct obs_encoder_roi *roi, float scale_x, float scale_y,
			   struct obs_encoder_roi *scaled);
