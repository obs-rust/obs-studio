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

#include "libobs/build/libobs-ported-encoder-roi.pch.h"

bool obs_encoder_roi_admit(uint32_t caps, const struct obs_encoder_roi *roi)
{
	if (!(caps & OBS_ENCODER_CAP_ROI))
		return false;
	/* Upside-down or empty: the size check below would wrap */
	if (roi->bottom <= roi->top || roi->right <= roi->left)
		return false;
	/* NaN is not in [-1, 1], but fails neither comparison below */
	if (isnan(roi->priority))
		return false;
	/* Area smaller than the smallest possible block (16x16) */
	if (roi->bottom - roi->top < 16 || roi->right - roi->left < 16)
		return false;
	/* Other invalid ROIs */
	if (roi->priority < -1.0f || roi->priority > 1.0f)
		return false;

	return true;
}

bool obs_encoder_roi_scale_factors(uint32_t scaled_width, uint32_t scaled_height, uint32_t width, uint32_t height,
				   float *scale_x, float *scale_y)
{
	if (!width || !height)
		return false;

	*scale_x = (float)scaled_width / (float)width;
	*scale_y = (float)scaled_height / (float)height;
	return true;
}

/* An edge scaled in float, then the nearest uint32_t: 0 for NaN and below
 * 0, UINT32_MAX from 2^32 up, otherwise truncated toward 0 as the cast does
 * (a plain cast is undefined out of range). */
static uint32_t obs_encoder_roi_scale_edge(uint32_t edge, float factor)
{
	float scaled = (float)edge * factor;
	if (isnan(scaled) || scaled < 0.0f)
		return 0;
	if (scaled >= 4294967296.0f)
		return UINT32_MAX;
	return (uint32_t)scaled;
}

void obs_encoder_roi_scale(const struct obs_encoder_roi *roi, float scale_x, float scale_y,
			   struct obs_encoder_roi *scaled)
{
	*scaled = (struct obs_encoder_roi){
		.top = obs_encoder_roi_scale_edge(roi->top, scale_y),
		.bottom = obs_encoder_roi_scale_edge(roi->bottom, scale_y),
		.left = obs_encoder_roi_scale_edge(roi->left, scale_x),
		.right = obs_encoder_roi_scale_edge(roi->right, scale_x),
		.priority = roi->priority,
	};
}
