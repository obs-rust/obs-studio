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

#include "libobs/build/libobs-ported-scene-transform-update.pch.h"

/* Ported to rust/obs-scene (transform_update): the hybrid libobs drops this
 * file. */

/* The transform update decisions of scene_video_render() and
 * update_transforms_and_prune_sources() in obs-scene.c, moved: the
 * decisions, on plain values. obs-scene.c keeps the locks, the walk and
 * every size read. Declared in obs-scene-transform-update.h without
 * EXPORT. */

/* scene_size_changed(), without the size reads. */
bool obs_scene_transform_update_scene_size(uint32_t *last_width, uint32_t *last_height, uint32_t width, uint32_t height)
{
	if (width == *last_width && height == *last_height)
		return false;

	*last_width = width;
	*last_height = height;
	return true;
}

/* The test of update_transforms_and_prune_sources(), with
 * source_size_changed()'s comparison. */
bool obs_scene_transform_update_item(bool update_transform, uint32_t last_width, uint32_t last_height, uint32_t width,
				     uint32_t height, bool scene_size_changed)
{
	return update_transform || last_width != width || last_height != height || scene_size_changed;
}
