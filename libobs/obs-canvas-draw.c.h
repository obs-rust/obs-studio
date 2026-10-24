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

#include "libobs/build/libobs-ported-canvas-draw.pch.h"

/* The canvas texture draw and display clear decisions, moved from
 * obs_render_canvas_texture_internal() in obs.c and render_display_begin() in
 * obs-display.c: only colour spaces, flags and the white level, no texture,
 * effect or display. obs.c and obs-display.c make the calls they decide on.
 * Ported to rust/obs-core (canvas_draw): the hybrid libobs drops this file. */

/* obs_render_canvas_texture_internal()'s early return. */
bool obs_canvas_draw_skip(bool has_mix, bool texture_rendered)
{
	return !has_mix || !texture_rendered;
}

/* obs_render_canvas_texture_internal()'s technique switch. */
enum obs_source_draw_technique obs_canvas_draw_technique(int source_space, int current_space)
{
	enum obs_source_draw_technique technique = OBS_SOURCE_DRAW_TECH_DRAW;

	switch (current_space) {
	case GS_CS_SRGB:
	case GS_CS_SRGB_16F:
		if (source_space == GS_CS_709_EXTENDED)
			technique = OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP;
		break;
	case GS_CS_709_SCRGB:
		technique = OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY;
		break;
	case GS_CS_709_EXTENDED:
		break;
	}

	return technique;
}

/* The multiplier switch: only GS_CS_709_SCRGB reads the white level. */
bool obs_canvas_draw_reads_white_level(int current_space)
{
	switch (current_space) {
	case GS_CS_709_SCRGB:
		return true;
	}

	return false;
}

float obs_canvas_draw_multiplier(float sdr_white_level)
{
	return sdr_white_level / 80.f;
}

/* render_display_begin()'s clear colour: the host first, then the space. */
bool obs_display_clear_reads_space(bool always_srgb)
{
	return !always_srgb;
}

bool obs_display_clear_linear(int current_space)
{
	return current_space == GS_CS_SRGB;
}

uint32_t obs_display_clear_flags(bool use_clear_workaround)
{
	uint32_t clear_flags = OBS_DISPLAY_CLEAR_DEPTH | OBS_DISPLAY_CLEAR_STENCIL;
	if (!use_clear_workaround)
		clear_flags |= OBS_DISPLAY_CLEAR_COLOR;
	return clear_flags;
}

bool obs_display_clear_draws_solid(bool use_clear_workaround)
{
	return use_clear_workaround;
}
