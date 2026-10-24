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

#include "libobs/build/libobs-ported-source-deinterlace-draw.pch.h"

/* The deinterlace draw decisions of obs-source-deinterlace.c, moved from
 * get_effect(), deinterlace_linear_required(), deinterlace_render() and
 * obs_source_set_deinterlace_mode(): the decisions, on plain values.
 * obs-source-deinterlace.c keeps every gs_* call in its order, loads the
 * effects, reads the colour spaces, the white level and the source's fields,
 * passes them in and acts on what comes back. Declared in
 * obs-source-deinterlace-draw.h without EXPORT. In hybrid libobs the Rust port
 * (rust/obs-source) replaces this file. */

enum obs_source_deinterlace_draw_effect obs_source_deinterlace_draw_effect(int mode)
{
	switch ((enum obs_deinterlace_mode)mode) {
	case OBS_DEINTERLACE_MODE_DISABLE:
		return OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_NONE;
	case OBS_DEINTERLACE_MODE_DISCARD:
		return OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_DISCARD;
	case OBS_DEINTERLACE_MODE_RETRO:
		return OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_DISCARD_2X;
	case OBS_DEINTERLACE_MODE_BLEND:
		return OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_BLEND;
	case OBS_DEINTERLACE_MODE_BLEND_2X:
		return OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_BLEND_2X;
	case OBS_DEINTERLACE_MODE_LINEAR:
		return OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_LINEAR;
	case OBS_DEINTERLACE_MODE_LINEAR_2X:
		return OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_LINEAR_2X;
	case OBS_DEINTERLACE_MODE_YADIF:
		return OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_YADIF;
	case OBS_DEINTERLACE_MODE_YADIF_2X:
		return OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_YADIF_2X;
	}

	return OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_NONE;
}

/* DISABLE and the modes obs_source_deinterlace_draw_effect() has an effect
 * for. */
static bool deinterlace_draw_mode_known(enum obs_deinterlace_mode mode)
{
	switch (mode) {
	case OBS_DEINTERLACE_MODE_DISABLE:
	case OBS_DEINTERLACE_MODE_DISCARD:
	case OBS_DEINTERLACE_MODE_RETRO:
	case OBS_DEINTERLACE_MODE_BLEND:
	case OBS_DEINTERLACE_MODE_BLEND_2X:
	case OBS_DEINTERLACE_MODE_LINEAR:
	case OBS_DEINTERLACE_MODE_LINEAR_2X:
	case OBS_DEINTERLACE_MODE_YADIF:
	case OBS_DEINTERLACE_MODE_YADIF_2X:
		return true;
	}

	return false;
}

bool obs_source_deinterlace_draw_linear_required(int mode)
{
	switch ((enum obs_deinterlace_mode)mode) {
	case OBS_DEINTERLACE_MODE_DISABLE:
	case OBS_DEINTERLACE_MODE_DISCARD:
	case OBS_DEINTERLACE_MODE_RETRO:
		return false;
	case OBS_DEINTERLACE_MODE_BLEND:
	case OBS_DEINTERLACE_MODE_BLEND_2X:
	case OBS_DEINTERLACE_MODE_LINEAR:
	case OBS_DEINTERLACE_MODE_LINEAR_2X:
	case OBS_DEINTERLACE_MODE_YADIF:
	case OBS_DEINTERLACE_MODE_YADIF_2X:
		return true;
	}

	return false;
}

bool obs_source_deinterlace_draw_skip(bool has_cur_tex, bool has_prev_tex, uint32_t width, uint32_t height)
{
	return !has_cur_tex || !has_prev_tex || !width || !height;
}

bool obs_source_deinterlace_draw_reads_linear_srgb(int source_space)
{
	/* (source_space != GS_CS_SRGB) || gs_get_linear_srgb() || ...: the
	 * graphics flag is read only for an SDR source. */
	return (enum gs_color_space)source_space == GS_CS_SRGB;
}

bool obs_source_deinterlace_draw_linear_srgb(int source_space, bool gs_linear_srgb, int mode)
{
	return ((enum gs_color_space)source_space != GS_CS_SRGB) || gs_linear_srgb ||
	       obs_source_deinterlace_draw_linear_required(mode);
}

enum obs_source_draw_technique obs_source_deinterlace_draw_technique(int source_space, int current_space)
{
	enum obs_source_draw_technique technique = OBS_SOURCE_DRAW_TECH_DRAW;
	switch ((enum gs_color_space)source_space) {
	case GS_CS_SRGB:
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

bool obs_source_deinterlace_draw_reads_white_level(int source_space, int current_space)
{
	switch (obs_source_deinterlace_draw_technique(source_space, current_space)) {
	case OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY:
	case OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY_TONEMAP:
	case OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA_MULTIPLY:
		return true;
	case OBS_SOURCE_DRAW_TECH_DRAW:
	case OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP:
	case OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA:
		break;
	}

	return false;
}

float obs_source_deinterlace_draw_multiplier(int source_space, int current_space, float sdr_white_level)
{
	float multiplier = 1.0;
	switch ((enum gs_color_space)source_space) {
	case GS_CS_SRGB:
	case GS_CS_SRGB_16F:
		if ((enum gs_color_space)current_space == GS_CS_709_SCRGB) {
			multiplier = sdr_white_level / 80.0f;
		}
		break;
	case GS_CS_709_EXTENDED:
		switch ((enum gs_color_space)current_space) {
		case GS_CS_SRGB:
		case GS_CS_SRGB_16F:
			break;
		case GS_CS_709_SCRGB:
			multiplier = sdr_white_level / 80.0f;
			break;
		case GS_CS_709_EXTENDED:
			break;
		}
		break;
	case GS_CS_709_SCRGB:
		switch ((enum gs_color_space)current_space) {
		case GS_CS_SRGB:
		case GS_CS_SRGB_16F:
			multiplier = 80.0f / sdr_white_level;
			break;
		case GS_CS_709_EXTENDED:
			multiplier = 80.0f / sdr_white_level;
			break;
		case GS_CS_709_SCRGB:
			break;
		}
	}

	return multiplier;
}

bool obs_source_deinterlace_draw_frame2(uint64_t video_time, uint64_t frame_ts, uint64_t offset, uint32_t half_duration)
{
	const uint64_t frame2_ts = frame_ts + offset + half_duration - OBS_SOURCE_DEINTERLACE_PACING_TWOX_TOLERANCE;
	return video_time >= frame2_ts;
}

enum obs_source_deinterlace_draw_mode_change obs_source_deinterlace_draw_mode_change(int current_mode, int mode)
{
	/* A mode outside the enum has no effect to draw with, so the source
	 * would draw nothing: it is DISABLE, and the getter and a saved scene
	 * collection say so (#2090). */
	if (!deinterlace_draw_mode_known((enum obs_deinterlace_mode)mode))
		mode = OBS_DEINTERLACE_MODE_DISABLE;
	if (current_mode == mode)
		return OBS_SOURCE_DEINTERLACE_DRAW_MODE_SAME;

	if (current_mode == OBS_DEINTERLACE_MODE_DISABLE)
		return OBS_SOURCE_DEINTERLACE_DRAW_MODE_ENABLE;
	else if (mode == OBS_DEINTERLACE_MODE_DISABLE)
		return OBS_SOURCE_DEINTERLACE_DRAW_MODE_DISABLE;
	else
		return OBS_SOURCE_DEINTERLACE_DRAW_MODE_SWITCH;
}
