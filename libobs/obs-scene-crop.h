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

#include <stdint.h>

/* The scene item crop arithmetic of obs-scene.c.h: the size a crop leaves
 * (calc_cx and calc_cy), a nested scene's crop rescaled when that scene
 * changes size (update_nested_scene_crop), where the crop moves the source
 * when it is drawn (render_item), and the clamp of obs_sceneitem_set_crop().
 * The scene still reads and writes the item, computes the rescale factors,
 * and decides when any of this runs; these take and return plain values.
 *
 * obs_scene_cropped_size: the size of a source of `size` pixels after the
 * crops on both sides (the item's crop and its bounds crop), or 2 when they
 * crop more than all of it. The sum is taken in 64 bits, so four ints past
 * the int range do not overflow; a negative sum also gives 2.
 *
 * obs_scene_scale_crop: `crop` scaled by `factor` in float, truncated
 * toward 0: INT_MAX from 2^31 up, INT_MIN from -2^31 down, 0 for NaN.
 *
 * obs_scene_crop_offset: crop + bounds_crop, summed in 64 bits, as a float.
 *
 * obs_scene_clamp_crop: a negative crop is 0; any other is kept.
 *
 * Moved here from obs-scene.h (#1840), which includes this header, so that
 * a test or the oracle can compile them without the core. Not EXPORT:
 * obs-scene.c.h calls them inside libobs. */

uint32_t obs_scene_cropped_size(uint32_t size, int crop1, int crop2, int bounds_crop1, int bounds_crop2);

int obs_scene_scale_crop(int crop, float factor);

float obs_scene_crop_offset(int crop, int bounds_crop);

int obs_scene_clamp_crop(int crop);
