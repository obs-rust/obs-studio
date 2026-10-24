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

/* The techniques of the default and deinterlace effects that a source draws
 * with, shared by the source draw plan (obs-source-draw-plan.h) and the
 * deinterlace draw decisions (obs-source-deinterlace-draw.h). The decisions
 * return the enum value; obs-source.c and obs-source-deinterlace.c map it to
 * the name with obs_source_draw_technique_name(). The values cross into the
 * Rust port (rust/obs-source draw_technique) as the C's numbers. */
enum obs_source_draw_technique {
	/* "Draw" */
	OBS_SOURCE_DRAW_TECH_DRAW,
	/* "DrawMultiply" */
	OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY,
	/* "DrawTonemap" */
	OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP,
	/* "DrawMultiplyTonemap" */
	OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY_TONEMAP,
	/* "DrawNonlinearAlpha" */
	OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA,
	/* "DrawNonlinearAlphaMultiply" */
	OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA_MULTIPLY,
};

/* The technique's name in default.effect and the deinterlace effects; "Draw"
 * for a value outside the enum. */
static inline const char *obs_source_draw_technique_name(enum obs_source_draw_technique technique)
{
	switch (technique) {
	case OBS_SOURCE_DRAW_TECH_DRAW:
		return "Draw";
	case OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY:
		return "DrawMultiply";
	case OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP:
		return "DrawTonemap";
	case OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY_TONEMAP:
		return "DrawMultiplyTonemap";
	case OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA:
		return "DrawNonlinearAlpha";
	case OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA_MULTIPLY:
		return "DrawNonlinearAlphaMultiply";
	}

	return "Draw";
}
