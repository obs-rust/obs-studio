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

#include "libobs/build/libobs-ported-scene-crop.pch.h"

/* Ported to rust/obs-scene (crop): the hybrid libobs drops this file. */

uint32_t obs_scene_cropped_size(uint32_t size, int crop1, int crop2, int bounds_crop1, int bounds_crop2)
{
	/* In 64 bits: four ints can be past the int range. Crops are 0 and up
	 * (obs_sceneitem_set_crop() clamps them); a negative sum crops all of
	 * it, as the sum read as unsigned did. */
	int64_t crop = (int64_t)crop1 + crop2 + bounds_crop1 + bounds_crop2;
	return (crop < 0 || crop > (int64_t)size) ? 2 : (size - (uint32_t)crop);
}

int obs_scene_scale_crop(int crop, float factor)
{
	float scaled = (float)crop * factor;
	if (isnan(scaled))
		return 0;
	if (scaled >= 2147483648.0f)
		return INT_MAX;
	if (scaled <= -2147483648.0f)
		return INT_MIN;
	return (int)scaled;
}

float obs_scene_crop_offset(int crop, int bounds_crop)
{
	return (float)((int64_t)crop + bounds_crop);
}

int obs_scene_clamp_crop(int crop)
{
	return crop < 0 ? 0 : crop;
}
