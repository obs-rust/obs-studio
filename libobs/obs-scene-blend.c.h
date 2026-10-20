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

#include "libobs/build/libobs.pch.h"

static const struct {
	enum gs_blend_type src_color;
	enum gs_blend_type src_alpha;
	enum gs_blend_type dst_color;
	enum gs_blend_type dst_alpha;
	enum gs_blend_op_type op;
} obs_blend_mode_params[] = {
	/* clang-format off */
	// OBS_BLEND_NORMAL
	{
		GS_BLEND_ONE,
		GS_BLEND_ONE,
		GS_BLEND_INVSRCALPHA,
		GS_BLEND_INVSRCALPHA,
		GS_BLEND_OP_ADD,
	},
	// OBS_BLEND_ADDITIVE
	{
		GS_BLEND_ONE,
		GS_BLEND_ONE,
		GS_BLEND_ONE,
		GS_BLEND_ONE,
		GS_BLEND_OP_ADD,
	},
	// OBS_BLEND_SUBTRACT
	{
		GS_BLEND_ONE,
		GS_BLEND_ONE,
		GS_BLEND_ONE,
		GS_BLEND_ONE,
		GS_BLEND_OP_REVERSE_SUBTRACT,
	},
	// OBS_BLEND_SCREEN
	{
		GS_BLEND_ONE,
		GS_BLEND_ONE,
		GS_BLEND_INVSRCCOLOR,
		GS_BLEND_INVSRCALPHA,
		GS_BLEND_OP_ADD
	},
	// OBS_BLEND_MULTIPLY
	{
		GS_BLEND_DSTCOLOR,
		GS_BLEND_DSTALPHA,
		GS_BLEND_INVSRCALPHA,
		GS_BLEND_INVSRCALPHA,
		GS_BLEND_OP_ADD
	},
	// OBS_BLEND_LIGHTEN
	{
		GS_BLEND_ONE,
		GS_BLEND_ONE,
		GS_BLEND_ONE,
		GS_BLEND_ONE,
		GS_BLEND_OP_MAX,
	},
	// OBS_BLEND_DARKEN
	{
		GS_BLEND_ONE,
		GS_BLEND_ONE,
		GS_BLEND_ONE,
		GS_BLEND_ONE,
		GS_BLEND_OP_MIN,
	},
	/* clang-format on */
};
