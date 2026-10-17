/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>
    Copyright (C) 2025 by Taylor Giampaolo <warchamp7@obsproject.com>

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

#pragma once

#include <obs.h>

#include <cstdint>

/* Draws the source (the program output when source is null), scaled to fit
 * and centred, into texrender at outputWidth x outputHeight in the given
 * color space. Call with the graphics context entered. Used by the
 * "Screenshot (Source)" and "Screenshot (Output)" actions (ScreenshotObj). */
void RenderScreenshot(gs_texrender_t *texrender, obs_source_t *source, uint32_t sourceWidth, uint32_t sourceHeight,
		      uint32_t outputWidth, uint32_t outputHeight, enum gs_color_space space);
