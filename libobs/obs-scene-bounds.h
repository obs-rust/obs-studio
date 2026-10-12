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

#include "obs.h"
#include "graphics/vec2.h"

/* Scales a source of cx x cy pixels, drawn at `scale`, so that it fits a
 * bounding box of `bounds` pixels the way `type` says (any OBS_BOUNDS_*
 * other than OBS_BOUNDS_NONE). Flips (negative scales) are kept.
 *
 * This is the scale half of the scene item transform (calculate_bounds_data
 * in obs-scene.c), kept free of scene items so it can be tested on its own. */
void obs_bounds_fit_scale(enum obs_bounds_type type, const struct vec2 *bounds, uint32_t cx, uint32_t cy,
			  struct vec2 *scale);
