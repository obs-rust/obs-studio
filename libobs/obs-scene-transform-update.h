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

/* The transform update decisions of scene_video_render() and
 * update_transforms_and_prune_sources() in obs-scene.c, on plain values
 * (obs-scene-transform-update.c):
 * - obs_scene_transform_update_scene_size(): scene_size_changed(), whether
 *   the scene has a new size since its last render, and the stored size;
 * - obs_scene_transform_update_item(): whether an item's transform is
 *   computed again on this walk.
 *
 * obs-scene.c keeps the video locks, the item list walk, the recursion into
 * a group's scene, the removed-source check and the removal, the group
 * rebuild flag, update_item_transform() (its defer check, size read, lock
 * and item_transform signal), resize_group() (its defer check) and every
 * size read: scene_getwidth() and scene_getheight() for the scene, and
 * obs_source_get_width() and obs_source_get_height() for an item, which it
 * reads only when the item's update_transform flag is clear (the || of the
 * original stopped at the flag). Nothing here takes a lock, calls into the
 * core or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* last_width and last_height: scene->last_width and scene->last_height.
 * width and height: the scene's size now.
 *
 * Returns false when the size is the stored one. Otherwise stores width and
 * height and returns true. */
bool obs_scene_transform_update_scene_size(uint32_t *last_width, uint32_t *last_height, uint32_t width,
					   uint32_t height);

/* update_transform: item->update_transform. last_width and last_height:
 * item->last_width and item->last_height (the source size at the item's
 * last transform). width and height: the source's size now, read only when
 * update_transform is false; ignored otherwise. scene_size_changed: the top
 * scene's obs_scene_transform_update_scene_size() result, given unchanged to
 * the walk of each group inside it; false for obs_scene_prune_sources().
 *
 * Returns true when update_transform is set, when the source size is not
 * the last one, or when scene_size_changed is set. */
bool obs_scene_transform_update_item(bool update_transform, uint32_t last_width, uint32_t last_height, uint32_t width,
				     uint32_t height, bool scene_size_changed);
