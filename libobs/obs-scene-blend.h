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

#include "graphics/graphics.h"

/* One row of obs_blend_mode_params: the factors render_item_texture() in
 * obs-scene.c passes to gs_blend_function_separate() and the operation it
 * passes to gs_blend_op(). */
struct obs_blend_mode_param {
	enum gs_blend_type src_color;
	enum gs_blend_type src_alpha;
	enum gs_blend_type dst_color;
	enum gs_blend_type dst_alpha;
	enum gs_blend_op_type op;
};

/* Indexed by enum obs_blending_type, one row per mode (obs-scene-blend.c).
 * Not exported: obs-scene.c reads it inside libobs. */
extern const struct obs_blend_mode_param obs_blend_mode_params[];
