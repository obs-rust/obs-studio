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

#include <math.h>
#include <stdint.h>

#include "graphics/matrix4.h"
#include "graphics/vec2.h"

/* The content extent of a scene or group, from resize_scene_base in
 * obs-scene.c.h. The scene still walks its items, still shifts the
 * children so the content starts at the origin (update_item_transform),
 * and still decides what a group does with the result.
 *
 * obs_scene_extent_init starts the fold: min is (M_INFINITE, M_INFINITE)
 * and max is (-M_INFINITE, -M_INFINITE). M_INFINITE is 3.4e38, not
 * infinity.
 *
 * obs_scene_extent_add folds the four corners of one item's box transform,
 * (0, 0), (1, 0), (0, 1) and (1, 1) through the matrix, into min and max.
 * The compares are < and >, so a NaN corner changes nothing and a tie
 * keeps the value already there.
 *
 * obs_scene_extent_size sets scale to max - min, and cx and cy to that
 * size rounded up (obs_scene_size_from_extent below).
 *
 * Not EXPORT: obs-scene.c.h calls them inside libobs. */

void obs_scene_extent_init(struct vec2 *minv, struct vec2 *maxv);

void obs_scene_extent_add(struct vec2 *minv, struct vec2 *maxv, const struct matrix4 *box_transform);

/* A group's size from the extent of its content, rounded up: UINT32_MAX
 * when it is past the range, 0 for none, a negative one or NaN. Moved here
 * from obs-scene.h (#1840), which includes this header. */
static inline uint32_t obs_scene_size_from_extent(float extent)
{
	float size = ceilf(extent);
	if (!(size > 0.0f))
		return 0;
	if (size >= 4294967296.0f)
		return UINT32_MAX;
	return (uint32_t)size;
}

void obs_scene_extent_size(struct vec2 *scale, uint32_t *cx, uint32_t *cy, const struct vec2 *minv,
			   const struct vec2 *maxv);
