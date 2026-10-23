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

#include "libobs/build/libobs.pch.h"

bool obs_encoder_roi_admit(uint32_t caps, const struct obs_encoder_roi *roi)
{
	if (!(caps & OBS_ENCODER_CAP_ROI))
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

void obs_encoder_roi_scale(const struct obs_encoder_roi *roi, float scale_x, float scale_y,
			   struct obs_encoder_roi *scaled)
{
	*scaled = (struct obs_encoder_roi){
		.top = (uint32_t)((float)roi->top * scale_y),
		.bottom = (uint32_t)((float)roi->bottom * scale_y),
		.left = (uint32_t)((float)roi->left * scale_x),
		.right = (uint32_t)((float)roi->right * scale_x),
		.priority = roi->priority,
	};
}
