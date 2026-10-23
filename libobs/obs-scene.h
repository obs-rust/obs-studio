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

#pragma once

#include "obs.h"
#include "graphics/matrix4.h"
#include "obs-scene-audio-actions.h"
#include "obs-scene-extent.h"

#include <limits.h>
#include <math.h>

/* how obs scene! */

struct obs_scene_item {
	volatile long ref;
	volatile bool removed;

	bool is_group;
	bool is_scene;
	bool update_transform;
	bool update_group_resize;

	int64_t id;

	struct obs_scene *parent;
	struct obs_source *source;
	volatile long active_refs;
	volatile long defer_update;
	volatile long defer_group_resize;
	bool user_visible;
	bool visible;
	bool selected;
	bool locked;

	gs_texrender_t *item_render;
	struct obs_sceneitem_crop crop;

	bool absolute_coordinates;
	struct vec2 pos;
	struct vec2 scale;
	struct vec2 scale_ref;
	float rot;
	uint32_t align;

	/* last width/height of the source, this is used to check whether
	 * the transform needs updating */
	uint32_t last_width;
	uint32_t last_height;

	struct vec2 output_scale;
	enum obs_scale_type scale_filter;

	enum obs_blending_method blend_method;
	enum obs_blending_type blend_type;

	struct matrix4 box_transform;
	struct vec2 box_scale;
	struct matrix4 draw_transform;

	enum obs_bounds_type bounds_type;
	uint32_t bounds_align;
	struct vec2 bounds;
	bool crop_to_bounds;
	struct obs_sceneitem_crop bounds_crop;

	obs_hotkey_pair_id toggle_visibility;

	obs_data_t *private_settings;

	pthread_mutex_t actions_mutex;
	obs_scene_item_action_array_t audio_actions;

	struct obs_source *show_transition;
	struct obs_source *hide_transition;
	uint32_t show_transition_duration;
	uint32_t hide_transition_duration;

	/* would do **prev_next, but not really great for reordering */
	struct obs_scene_item *prev;
	struct obs_scene_item *next;
};

struct obs_scene {
	struct obs_source *source;

	bool is_group;
	bool custom_size;
	uint32_t cx;
	uint32_t cy;

	bool absolute_coordinates;
	uint32_t last_width;
	uint32_t last_height;

	int64_t id_counter;

	pthread_mutex_t video_mutex;
	pthread_mutex_t audio_mutex;
	struct obs_scene_item *first_item;
};

/* Scene item crop and size arithmetic for obs-scene.c, here so that
 * test_scene_crop_steps can build it on its own with UBSan. */

/* A crop read from a scene collection, as an int: the nearest one when it
 * is past the range. */
static inline int obs_scene_crop_from_int64(int64_t crop)
{
	if (crop > INT_MAX)
		return INT_MAX;
	if (crop < INT_MIN)
		return INT_MIN;
	return (int)crop;
}

/* The size of a source of `size` pixels after the crops on both sides (the
 * item's crop and its bounds crop), or 2 when they crop more than all of it. */
static inline uint32_t obs_scene_cropped_size(uint32_t size, int crop1, int crop2, int bounds_crop1, int bounds_crop2)
{
	/* In 64 bits: four ints can be past the int range. Crops are 0 and up
	 * (obs_sceneitem_set_crop() clamps them); a negative sum crops all of
	 * it, as the sum read as unsigned did. */
	int64_t crop = (int64_t)crop1 + crop2 + bounds_crop1 + bounds_crop2;
	return (crop < 0 || crop > (int64_t)size) ? 2 : (size - (uint32_t)crop);
}

/* A nested scene's crop, scaled by the factor the scene's size changed by,
 * truncated: the nearest int when it is past the range, 0 for NaN. */
static inline int obs_scene_scale_crop(int crop, float factor)
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

/* How far the crops move the source when it is drawn. */
static inline float obs_scene_crop_offset(int crop, int bounds_crop)
{
	return (float)((int64_t)crop + bounds_crop);
}
