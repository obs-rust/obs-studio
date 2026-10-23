/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>
                          Philippe Groarke <philippe.groarke@gmail.com>

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

#include "libobs/build/libobs-ported-scene-extent.pch.h"

/* Ported to rust/obs-scene (extent): the hybrid libobs drops this file. */

void obs_scene_extent_init(struct vec2 *minv, struct vec2 *maxv)
{
	vec2_set(minv, M_INFINITE, M_INFINITE);
	vec2_set(maxv, -M_INFINITE, -M_INFINITE);
}

void obs_scene_extent_add(struct vec2 *minv, struct vec2 *maxv, const struct matrix4 *box_transform)
{
#define get_min_max(x_val, y_val)                          \
	do {                                               \
		struct vec3 v;                             \
		vec3_set(&v, x_val, y_val, 0.0f);          \
		vec3_transform(&v, &v, box_transform);     \
		if (v.x < minv->x)                         \
			minv->x = v.x;                     \
		if (v.y < minv->y)                         \
			minv->y = v.y;                     \
		if (v.x > maxv->x)                         \
			maxv->x = v.x;                     \
		if (v.y > maxv->y)                         \
			maxv->y = v.y;                     \
	} while (false)

	get_min_max(0.0f, 0.0f);
	get_min_max(1.0f, 0.0f);
	get_min_max(0.0f, 1.0f);
	get_min_max(1.0f, 1.0f);
#undef get_min_max
}

void obs_scene_extent_size(struct vec2 *scale, uint32_t *cx, uint32_t *cy, const struct vec2 *minv,
			   const struct vec2 *maxv)
{
	vec2_sub(scale, maxv, minv);
	*cx = obs_scene_size_from_extent(scale->x);
	*cy = obs_scene_size_from_extent(scale->y);
}
