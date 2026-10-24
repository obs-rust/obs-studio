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

#include "libobs/build/libobs-ported-source-draw-plan.pch.h"

/* The source draw decisions of obs-source.c, moved from rotate_async_video(),
 * obs_source_render_async_video() and source_render(): the decisions, on
 * plain values. obs-source.c keeps every gs_* call in its order, reads the
 * colour spaces, the graphics linear sRGB flag, the white level and the
 * source's fields, passes them in and acts on what comes back. Declared in
 * obs-source-draw-plan.h without EXPORT. In hybrid libobs the Rust port
 * (rust/obs-source) replaces this file. */

float obs_source_draw_plan_rotation_x(long rotation, uint32_t width, uint32_t height)
{
	float x = 0;

	/* obs_source_set_async_rotation() keeps the rotation in 0-359 */
	switch (rotation) {
	case 270:
		x = (float)height;
		break;
	case 180:
		x = (float)width;
	}

	return x;
}

float obs_source_draw_plan_rotation_y(long rotation, uint32_t width, uint32_t height)
{
	float y = 0;

	switch (rotation) {
	case 90:
		y = (float)width;
		break;
	case 180:
		y = (float)height;
	}

	return y;
}

bool obs_source_draw_plan_async_linear_srgb(int source_space, int current_space, bool gs_linear_srgb)
{
	bool linear_srgb = gs_linear_srgb;
	switch ((enum gs_color_space)source_space) {
	case GS_CS_SRGB:
		linear_srgb = linear_srgb || ((enum gs_color_space)current_space != GS_CS_SRGB);
		break;
	case GS_CS_SRGB_16F:
		break;
	case GS_CS_709_EXTENDED:
	case GS_CS_709_SCRGB:
		switch ((enum gs_color_space)current_space) {
		case GS_CS_SRGB:
		case GS_CS_SRGB_16F:
			linear_srgb = true;
			break;
		case GS_CS_709_EXTENDED:
		case GS_CS_709_SCRGB:
			break;
		}
	}

	return linear_srgb;
}

bool obs_source_draw_plan_async_nonlinear_alpha(int source_space, bool linear_srgb, bool linear_alpha)
{
	return ((enum gs_color_space)source_space == GS_CS_SRGB) && linear_srgb && !linear_alpha;
}

enum obs_source_draw_technique obs_source_draw_plan_technique(int source_space, int current_space, bool nonlinear_alpha)
{
	enum obs_source_draw_technique technique = OBS_SOURCE_DRAW_TECH_DRAW;
	switch ((enum gs_color_space)source_space) {
	case GS_CS_SRGB:
		switch ((enum gs_color_space)current_space) {
		case GS_CS_SRGB:
		case GS_CS_SRGB_16F:
		case GS_CS_709_EXTENDED:
			if (nonlinear_alpha)
				technique = OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA;
			break;
		case GS_CS_709_SCRGB:
			technique = nonlinear_alpha ? OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA_MULTIPLY
						    : OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY;
		}
		break;
	case GS_CS_SRGB_16F:
		if ((enum gs_color_space)current_space == GS_CS_709_SCRGB)
			technique = OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY;
		break;
	case GS_CS_709_EXTENDED:
		switch ((enum gs_color_space)current_space) {
		case GS_CS_SRGB:
		case GS_CS_SRGB_16F:
			technique = OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP;
			break;
		case GS_CS_709_SCRGB:
			technique = OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY;
			break;
		case GS_CS_709_EXTENDED:
			break;
		}
		break;
	case GS_CS_709_SCRGB:
		switch ((enum gs_color_space)current_space) {
		case GS_CS_SRGB:
		case GS_CS_SRGB_16F:
			technique = OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY_TONEMAP;
			break;
		case GS_CS_709_EXTENDED:
			technique = OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY;
			break;
		case GS_CS_709_SCRGB:
			break;
		}
	}

	return technique;
}

bool obs_source_draw_plan_reads_white_level(int source_space, int current_space)
{
	switch ((enum gs_color_space)source_space) {
	case GS_CS_SRGB:
	case GS_CS_SRGB_16F:
	case GS_CS_709_EXTENDED:
		return (enum gs_color_space)current_space == GS_CS_709_SCRGB;
	case GS_CS_709_SCRGB:
		switch ((enum gs_color_space)current_space) {
		case GS_CS_SRGB:
		case GS_CS_SRGB_16F:
		case GS_CS_709_EXTENDED:
			return true;
		case GS_CS_709_SCRGB:
			break;
		}
	}

	return false;
}

float obs_source_draw_plan_multiplier(int source_space, int current_space, float sdr_white_level)
{
	float multiplier = 1.0;
	switch ((enum gs_color_space)source_space) {
	case GS_CS_SRGB:
	case GS_CS_SRGB_16F:
	case GS_CS_709_EXTENDED:
		if ((enum gs_color_space)current_space == GS_CS_709_SCRGB)
			multiplier = sdr_white_level / 80.0f;
		break;
	case GS_CS_709_SCRGB:
		switch ((enum gs_color_space)current_space) {
		case GS_CS_SRGB:
		case GS_CS_SRGB_16F:
		case GS_CS_709_EXTENDED:
			multiplier = 80.0f / sdr_white_level;
			break;
		case GS_CS_709_SCRGB:
			break;
		}
	}

	return multiplier;
}

bool obs_source_draw_plan_convert(int source_space, int current_space)
{
	switch ((enum gs_color_space)source_space) {
	case GS_CS_SRGB:
	case GS_CS_SRGB_16F:
		switch ((enum gs_color_space)current_space) {
		case GS_CS_709_EXTENDED:
		case GS_CS_709_SCRGB:
			return true;
		case GS_CS_SRGB:
		case GS_CS_SRGB_16F:
			break;
		}
		break;
	case GS_CS_709_EXTENDED:
		switch ((enum gs_color_space)current_space) {
		case GS_CS_SRGB:
		case GS_CS_SRGB_16F:
		case GS_CS_709_SCRGB:
			return true;
		case GS_CS_709_EXTENDED:
			break;
		}
		break;
	case GS_CS_709_SCRGB:
		switch ((enum gs_color_space)current_space) {
		case GS_CS_SRGB:
		case GS_CS_SRGB_16F:
		case GS_CS_709_EXTENDED:
			return true;
		case GS_CS_709_SCRGB:
			break;
		}
	}

	return false;
}
