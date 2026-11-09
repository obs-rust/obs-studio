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

#include "libobs/build/libobs-graphics.pch.h"

extern bool obs_graphics_viewport_valid(const char *function);
#define gs_valid obs_graphics_viewport_valid

uint32_t obs_graphics_device_max_texture_size(graphics_t *graphics);
uint32_t obs_graphics_device_max_texture_size(graphics_t *graphics)
{
	if (graphics->exports.device_get_max_texture_size)
		return graphics->exports.device_get_max_texture_size(graphics->device);

	/* Upper bounds fixed by the API: Direct3D 11 at feature level 11 and
	 * up (8192 below), and Metal on every Mac. */
	switch (graphics->exports.device_get_type()) {
	case GS_DEVICE_DIRECT3D_11:
	case GS_DEVICE_METAL:
		return 16384;
	default:
		return 0;
	}
}

void gs_reset_viewport(void)
{
	uint32_t cx, cy;

	if (!gs_valid("gs_reset_viewport"))
		return;

	gs_get_size(&cx, &cy);
	gs_set_viewport(0, 0, (int)cx, (int)cy);
}

void gs_set_2d_mode(void)
{
	uint32_t cx, cy;

	if (!gs_valid("gs_set_2d_mode"))
		return;

	gs_get_size(&cx, &cy);
	gs_ortho(0.0f, (float)cx, 0.0f, (float)cy, -1.0, -1024.0f);
}

#undef gs_valid
