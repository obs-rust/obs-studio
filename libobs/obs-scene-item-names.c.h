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

#include "libobs/build/libobs-ported-scene-item-names.pch.h"

/* Ported to rust/obs-scene (item_names): the hybrid libobs drops this file.
 * The scene item names and coordinate keys of scene_load_item() and
 * scene_save_item() in obs-scene.c: the tables and the key choice, on plain
 * values. obs-scene.c keeps every obs_data_* call and stores the results in
 * the item. Declared in obs-scene-item-names.h without EXPORT. */

int obs_scene_item_scale_filter_from_name(const char *name, obs_scene_item_name_cmp cmp)
{
	int scale_filter = OBS_SCENE_ITEM_SCALE_DISABLE;

	if (name) {
		if (cmp(name, "point") == 0)
			scale_filter = OBS_SCENE_ITEM_SCALE_POINT;
		else if (cmp(name, "bilinear") == 0)
			scale_filter = OBS_SCENE_ITEM_SCALE_BILINEAR;
		else if (cmp(name, "bicubic") == 0)
			scale_filter = OBS_SCENE_ITEM_SCALE_BICUBIC;
		else if (cmp(name, "lanczos") == 0)
			scale_filter = OBS_SCENE_ITEM_SCALE_LANCZOS;
		else if (cmp(name, "area") == 0)
			scale_filter = OBS_SCENE_ITEM_SCALE_AREA;
	}

	return scale_filter;
}

int obs_scene_item_blend_method_from_name(const char *name, obs_scene_item_name_cmp cmp)
{
	int blend_method = OBS_SCENE_ITEM_BLEND_METHOD_DEFAULT;

	if (name) {
		if (cmp(name, "srgb_off") == 0)
			blend_method = OBS_SCENE_ITEM_BLEND_METHOD_SRGB_OFF;
	}

	return blend_method;
}

int obs_scene_item_blend_type_from_name(const char *name, obs_scene_item_name_cmp cmp)
{
	int blend_type = OBS_SCENE_ITEM_BLEND_NORMAL;

	if (name) {
		if (cmp(name, "additive") == 0)
			blend_type = OBS_SCENE_ITEM_BLEND_ADDITIVE;
		else if (cmp(name, "subtract") == 0)
			blend_type = OBS_SCENE_ITEM_BLEND_SUBTRACT;
		else if (cmp(name, "screen") == 0)
			blend_type = OBS_SCENE_ITEM_BLEND_SCREEN;
		else if (cmp(name, "multiply") == 0)
			blend_type = OBS_SCENE_ITEM_BLEND_MULTIPLY;
		else if (cmp(name, "lighten") == 0)
			blend_type = OBS_SCENE_ITEM_BLEND_LIGHTEN;
		else if (cmp(name, "darken") == 0)
			blend_type = OBS_SCENE_ITEM_BLEND_DARKEN;
	}

	return blend_type;
}

const char *obs_scene_item_scale_filter_name(int scale_filter)
{
	if (scale_filter == OBS_SCENE_ITEM_SCALE_POINT)
		return "point";
	else if (scale_filter == OBS_SCENE_ITEM_SCALE_BILINEAR)
		return "bilinear";
	else if (scale_filter == OBS_SCENE_ITEM_SCALE_BICUBIC)
		return "bicubic";
	else if (scale_filter == OBS_SCENE_ITEM_SCALE_LANCZOS)
		return "lanczos";
	else if (scale_filter == OBS_SCENE_ITEM_SCALE_AREA)
		return "area";
	else
		return "disable";
}

const char *obs_scene_item_blend_method_name(int blend_method)
{
	if (blend_method == OBS_SCENE_ITEM_BLEND_METHOD_SRGB_OFF)
		return "srgb_off";
	else
		return "default";
}

const char *obs_scene_item_blend_type_name(int blend_type)
{
	if (blend_type == OBS_SCENE_ITEM_BLEND_NORMAL)
		return "normal";
	else if (blend_type == OBS_SCENE_ITEM_BLEND_ADDITIVE)
		return "additive";
	else if (blend_type == OBS_SCENE_ITEM_BLEND_SUBTRACT)
		return "subtract";
	else if (blend_type == OBS_SCENE_ITEM_BLEND_SCREEN)
		return "screen";
	else if (blend_type == OBS_SCENE_ITEM_BLEND_MULTIPLY)
		return "multiply";
	else if (blend_type == OBS_SCENE_ITEM_BLEND_LIGHTEN)
		return "lighten";
	else if (blend_type == OBS_SCENE_ITEM_BLEND_DARKEN)
		return "darken";
	else
		return "normal";
}

enum obs_scene_item_keys obs_scene_item_transform_keys(bool absolute_coordinates, bool has_pos_rel, bool has_scale_rel,
						       bool has_scale_ref)
{
	if (!absolute_coordinates && has_pos_rel && has_scale_rel && has_scale_ref)
		return OBS_SCENE_ITEM_KEYS_RELATIVE;
	else if (!absolute_coordinates)
		return OBS_SCENE_ITEM_KEYS_FROM_ABSOLUTE;
	else
		return OBS_SCENE_ITEM_KEYS_ABSOLUTE;
}

enum obs_scene_item_keys obs_scene_item_bounds_keys(bool absolute_coordinates, bool has_bounds_rel)
{
	if (!absolute_coordinates && has_bounds_rel)
		return OBS_SCENE_ITEM_KEYS_RELATIVE;
	else if (!absolute_coordinates)
		return OBS_SCENE_ITEM_KEYS_FROM_ABSOLUTE;
	else
		return OBS_SCENE_ITEM_KEYS_ABSOLUTE;
}

bool obs_scene_item_save_relative(bool absolute_coordinates)
{
	return !absolute_coordinates;
}
