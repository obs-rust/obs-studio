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

#pragma once

#include <stdbool.h>
#include <stdint.h>

struct matrix4;
struct obs_sceneitem_crop;
struct vec2;

/* The scene item transform composition of compute_item_transform() in
 * obs-scene.c, on plain values (obs-scene-item-transform.c), as the steps
 * the scene calls in order:
 * - obs_scene_item_transform_prepare(): the bounds crop reset, a nested
 *   scene's crop rescaled by its size change (update_nested_scene_crop),
 *   the cropped size (calc_cx and calc_cy) and the last size;
 * - obs_scene_item_transform_bounded(): whether the item has a bounding box,
 *   so its size and box scale come from the bounds and not the scale;
 * - obs_scene_item_transform_matrix(): draw_transform and box_transform,
 *   the same matrix4 calls in the same order (scale, minus the origin, the
 *   rotation about z, the position);
 * - obs_scene_item_transform_box_scale(): the box scale of an item without
 *   bounds, the scale times the cropped size.
 *
 * obs-scene.c keeps every get_scene_dimensions() read where it was
 * (item_canvas_scale(), pos_to_absolute() and size_to_absolute(), each
 * read between these steps, never hoisted), calculate_bounds_data() (a
 * read of the bounds, then the ported obs_bounds_place()), add_alignment()
 * and all six float-to-integer casts: the two (uint32_t)((float)cx *
 * scale.x) of an item without bounds, and the four (int) passed to
 * add_alignment(). A negative float cast to uint32_t, or a float past
 * INT_MAX cast to int, gives a different result on x86-64 and arm64, so
 * these values are cast where they were, and each architecture keeps its
 * result. update_item_transform() keeps the defer check, the source size
 * read, the parent's video lock and the item_transform signal. Nothing here
 * takes a lock, calls into the core or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* The cropped size of the source. */
struct obs_scene_item_transform_size {
	uint32_t cx;
	uint32_t cy;
};

/* crop and bounds_crop: item->crop and item->bounds_crop. last_width and
 * last_height: item->last_width and item->last_height. width and height:
 * the source's size. nested: update_tex && item->is_scene.
 *
 * Zeroes bounds_crop. When nested and neither last size is 0, scales each
 * crop by the new size over the last one ((float)width / (float)last_width,
 * obs_scene_scale_crop()). Returns the size left after the crops
 * (obs_scene_cropped_size(), with the bounds crop just zeroed), then sets
 * the last size to width and height. */
struct obs_scene_item_transform_size obs_scene_item_transform_prepare(struct obs_sceneitem_crop *crop,
								      struct obs_sceneitem_crop *bounds_crop,
								      uint32_t *last_width, uint32_t *last_height,
								      uint32_t width, uint32_t height, bool nested);

/* bounds_type: item->bounds_type, an enum obs_bounds_type. Not
 * OBS_BOUNDS_NONE. A value outside the enum is OBS_BOUNDS_NONE before it
 * gets here (valid_bounds_type(), #2281). */
bool obs_scene_item_transform_bounded(int bounds_type);

/* The identity, scaled by (scale->x, scale->y, 1), translated by
 * (-origin->x, -origin->y, 0), rotated by RAD(rot) about (0, 0, 1) and
 * translated by (position->x, position->y, 0), into dst. */
void obs_scene_item_transform_matrix(struct matrix4 *dst, const struct vec2 *scale, const struct vec2 *origin,
				     float rot, const struct vec2 *position);

/* scale->x *= (float)width and scale->y *= (float)height, in float. */
void obs_scene_item_transform_box_scale(struct vec2 *scale, uint32_t width, uint32_t height);
