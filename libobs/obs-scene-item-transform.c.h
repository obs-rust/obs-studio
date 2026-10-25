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

#include "libobs/build/libobs-ported-scene-item-transform.pch.h"

/* Ported to rust/obs-scene (item_transform): the hybrid libobs drops this file.
 * The scene item transform composition of compute_item_transform() in
 * obs-scene.c, moved here as the steps the scene calls in order, on plain
 * values. obs-scene.c keeps every get_scene_dimensions() read between them,
 * calculate_bounds_data(), add_alignment() and the six float-to-integer
 * casts. Declared in obs-scene-item-transform.h without EXPORT. */

static inline void update_nested_scene_crop(struct obs_sceneitem_crop *crop, uint32_t last_width, uint32_t last_height,
					    uint32_t width, uint32_t height)
{
	if (!last_height || !last_width)
		return;

	/* Use last size and new size to calculate factor to adjust crop by. */
	float scale_x = (float)width / (float)last_width;
	float scale_y = (float)height / (float)last_height;

	crop->left = obs_scene_scale_crop(crop->left, scale_x);
	crop->right = obs_scene_scale_crop(crop->right, scale_x);
	crop->top = obs_scene_scale_crop(crop->top, scale_y);
	crop->bottom = obs_scene_scale_crop(crop->bottom, scale_y);
}

struct obs_scene_item_transform_size obs_scene_item_transform_prepare(struct obs_sceneitem_crop *crop,
								      struct obs_sceneitem_crop *bounds_crop,
								      uint32_t *last_width, uint32_t *last_height,
								      uint32_t width, uint32_t height, bool nested)
{
	struct obs_scene_item_transform_size size;

	/* Reset bounds crop */
	memset(bounds_crop, 0, sizeof(*bounds_crop));

	/* Adjust crop on nested scenes (if any) */
	if (nested)
		update_nested_scene_crop(crop, *last_width, *last_height, width, height);

	size.cx = obs_scene_cropped_size(width, crop->left, crop->right, bounds_crop->left, bounds_crop->right);
	size.cy = obs_scene_cropped_size(height, crop->top, crop->bottom, bounds_crop->top, bounds_crop->bottom);
	*last_width = width;
	*last_height = height;
	return size;
}

bool obs_scene_item_transform_bounded(int bounds_type)
{
	return bounds_type != OBS_BOUNDS_NONE;
}

void obs_scene_item_transform_matrix(struct matrix4 *dst, const struct vec2 *scale, const struct vec2 *origin,
				     float rot, const struct vec2 *position)
{
	matrix4_identity(dst);
	matrix4_scale3f(dst, dst, scale->x, scale->y, 1.0f);
	matrix4_translate3f(dst, dst, -origin->x, -origin->y, 0.0f);
	matrix4_rotate_aa4f(dst, dst, 0.0f, 0.0f, 1.0f, RAD(rot));
	matrix4_translate3f(dst, dst, position->x, position->y, 0.0f);
}

void obs_scene_item_transform_box_scale(struct vec2 *scale, uint32_t width, uint32_t height)
{
	scale->x *= (float)width;
	scale->y *= (float)height;
}
