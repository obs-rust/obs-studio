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

#include "libobs/build/libobs-ported-graphics.pch.h"

extern void *obs_graphics_stencil_context(const char *function);
#define thread_graphics gs_get_context()
#define gs_valid(function) (obs_graphics_stencil_context(function) != NULL)

void gs_enable_stencil_test(bool enable)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_enable_stencil_test"))
		return;

	graphics->exports.device_enable_stencil_test(graphics->device, enable);
}

void gs_enable_stencil_write(bool enable)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_enable_stencil_write"))
		return;

	graphics->exports.device_enable_stencil_write(graphics->device, enable);
}

void gs_stencil_function(enum gs_stencil_side side, enum gs_depth_test test)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_stencil_function"))
		return;

	graphics->exports.device_stencil_function(graphics->device, side, test);
}

void gs_stencil_op(enum gs_stencil_side side, enum gs_stencil_op_type fail, enum gs_stencil_op_type zfail,
		   enum gs_stencil_op_type zpass)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_stencil_op"))
		return;

	graphics->exports.device_stencil_op(graphics->device, side, fail, zfail, zpass);
}

#undef gs_valid
#undef thread_graphics
