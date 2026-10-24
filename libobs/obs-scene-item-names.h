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

#include <stdbool.h>

/* The scene item names and coordinate keys of scene_load_item() and
 * scene_save_item() in obs-scene.c, on plain values
 * (obs-scene-item-names.c):
 * - obs_scene_item_scale_filter_from_name(),
 *   obs_scene_item_blend_method_from_name() and
 *   obs_scene_item_blend_type_from_name(): the value a saved name loads as,
 *   matched with the caller's astrcmpi(); NULL or any other name loads as
 *   Disable, Default or Normal;
 * - obs_scene_item_scale_filter_name(), obs_scene_item_blend_method_name()
 *   and obs_scene_item_blend_type_name(): the name a value is saved under,
 *   a static string; any other value saves as "disable", "default" or
 *   "normal";
 * - obs_scene_item_transform_keys() and obs_scene_item_bounds_keys(): which
 *   keys the position and scale, and the bounds, load from;
 * - obs_scene_item_save_relative(): whether the relative keys are saved.
 *
 * obs-scene.c keeps every obs_data_* call, the coordinate conversions
 * (obs-scene-coords.c), the source lookup, the transitions and the private
 * settings, and stores the results in the item. Nothing here takes a lock,
 * calls into libobs or keeps state: the loaders call only the compare they
 * are given, which obs-scene.c passes as libobs's own astrcmpi().
 *
 * Enum values cross as int, so this header needs no core header;
 * obs-scene.c checks the constants below against obs.h.
 *
 * Hidden in libobs: not EXPORT. */

/* enum obs_scale_type */
#define OBS_SCENE_ITEM_SCALE_DISABLE 0
#define OBS_SCENE_ITEM_SCALE_POINT 1
#define OBS_SCENE_ITEM_SCALE_BICUBIC 2
#define OBS_SCENE_ITEM_SCALE_BILINEAR 3
#define OBS_SCENE_ITEM_SCALE_LANCZOS 4
#define OBS_SCENE_ITEM_SCALE_AREA 5
/* enum obs_blending_method */
#define OBS_SCENE_ITEM_BLEND_METHOD_DEFAULT 0
#define OBS_SCENE_ITEM_BLEND_METHOD_SRGB_OFF 1
/* enum obs_blending_type */
#define OBS_SCENE_ITEM_BLEND_NORMAL 0
#define OBS_SCENE_ITEM_BLEND_ADDITIVE 1
#define OBS_SCENE_ITEM_BLEND_SUBTRACT 2
#define OBS_SCENE_ITEM_BLEND_SCREEN 3
#define OBS_SCENE_ITEM_BLEND_MULTIPLY 4
#define OBS_SCENE_ITEM_BLEND_LIGHTEN 5
#define OBS_SCENE_ITEM_BLEND_DARKEN 6

/* astrcmpi()'s signature: 0 when the two names match. */
typedef int (*obs_scene_item_name_cmp)(const char *name, const char *known);

/* name: obs_data_get_string() of "scale_filter", "blend_method" or
 * "blend_type" (never NULL from obs_data, but NULL is the default here,
 * without a compare). cmp: astrcmpi, called as cmp(name, known) for each
 * known name in turn until one gives 0; never NULL. The result is an enum
 * obs_scale_type, obs_blending_method or obs_blending_type value. */
int obs_scene_item_scale_filter_from_name(const char *name, obs_scene_item_name_cmp cmp);
int obs_scene_item_blend_method_from_name(const char *name, obs_scene_item_name_cmp cmp);
int obs_scene_item_blend_type_from_name(const char *name, obs_scene_item_name_cmp cmp);

/* scale_filter, blend_method, blend_type: the item's value, any int. */
const char *obs_scene_item_scale_filter_name(int scale_filter);
const char *obs_scene_item_blend_method_name(int blend_method);
const char *obs_scene_item_blend_type_name(int blend_type);

/* Which keys scene_load_item() reads a position, scale or bounds from. */
enum obs_scene_item_keys {
	/* "pos_rel", "scale_rel" and "scale_ref", or "bounds_rel", as they are */
	OBS_SCENE_ITEM_KEYS_RELATIVE,
	/* "pos" and "scale", or "bounds", converted from absolute */
	OBS_SCENE_ITEM_KEYS_FROM_ABSOLUTE,
	/* "pos" and "scale", or "bounds", as they are */
	OBS_SCENE_ITEM_KEYS_ABSOLUTE,
};

/* absolute_coordinates: item->absolute_coordinates. has_*:
 * obs_data_has_user_value() of that key. */
enum obs_scene_item_keys obs_scene_item_transform_keys(bool absolute_coordinates, bool has_pos_rel, bool has_scale_rel,
						       bool has_scale_ref);
enum obs_scene_item_keys obs_scene_item_bounds_keys(bool absolute_coordinates, bool has_bounds_rel);

/* scene_save_item() writes "pos_rel", "scale_rel" and "bounds_rel", and
 * "pos", "scale" and "bounds" converted to absolute, rather than "pos",
 * "scale" and "bounds" alone. */
bool obs_scene_item_save_relative(bool absolute_coordinates);
