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

#include <stdbool.h>
#include <stdint.h>

#include "obs-source-draw-technique.h"

/* The deinterlace draw decisions of obs-source-deinterlace.c, on plain values
 * (obs-source-deinterlace-draw.c):
 * - obs_source_deinterlace_draw_effect(): which effect a mode draws with
 *   (get_effect() loads it);
 * - obs_source_deinterlace_draw_linear_required(): whether a mode needs
 *   linear sRGB input;
 * - in deinterlace_render(): the early return
 *   (obs_source_deinterlace_draw_skip()), the linear sRGB flag, the
 *   technique and multiplier for the source and current colour spaces, and
 *   whether the second field is due (obs_source_deinterlace_draw_frame2());
 * - obs_source_deinterlace_draw_mode_change(): what
 *   obs_source_set_deinterlace_mode() does, a mode outside the enum being
 *   DISABLE (#2090).
 *
 * obs-source-deinterlace.c keeps every gs_* call in its order,
 * obs_load_effect() and the effect slots, convert_video_space(),
 * obs_get_video_sdr_white_level(), enable_deinterlacing(),
 * disable_deinterlacing() and obs_enter_graphics(): it passes their results
 * in. gs_get_linear_srgb() and obs_get_video_sdr_white_level() are called
 * only when the _reads_ functions say so, as before. None of these functions
 * takes a lock, calls into libobs or keeps state.
 *
 * Modes are enum obs_deinterlace_mode values and colour spaces enum
 * gs_color_space values, passed as int; obs-source-deinterlace.c checks the
 * sizes. A value outside either enum takes the paths it took before.
 *
 * Hidden in libobs: not EXPORT. */

/* The effect a mode draws with: obs->video.deinterlace_<name>_effect, loaded
 * from deinterlace_<name>.effect. */
enum obs_source_deinterlace_draw_effect {
	/* DISABLE, or a mode outside the enum: no effect */
	OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_NONE = -1,
	OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_DISCARD,
	OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_DISCARD_2X,
	OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_BLEND,
	OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_BLEND_2X,
	OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_LINEAR,
	OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_LINEAR_2X,
	OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_YADIF,
	OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_YADIF_2X,
};

/* What obs_source_set_deinterlace_mode() does. */
enum obs_source_deinterlace_draw_mode_change {
	/* nothing: the mode is the current one */
	OBS_SOURCE_DEINTERLACE_DRAW_MODE_SAME,
	/* enable_deinterlacing(source, mode): from DISABLE */
	OBS_SOURCE_DEINTERLACE_DRAW_MODE_ENABLE,
	/* disable_deinterlacing(source): to DISABLE, or a mode outside the enum */
	OBS_SOURCE_DEINTERLACE_DRAW_MODE_DISABLE,
	/* store the mode and its effect: from one mode to another */
	OBS_SOURCE_DEINTERLACE_DRAW_MODE_SWITCH,
};

/* get_effect(): the effect of each mode from DISCARD to YADIF_2X, NONE for
 * DISABLE and for a mode outside the enum. */
enum obs_source_deinterlace_draw_effect obs_source_deinterlace_draw_effect(int mode);

/* Whether a mode needs linear sRGB input: BLEND to YADIF_2X; not DISABLE,
 * DISCARD, RETRO or a mode outside the enum. */
bool obs_source_deinterlace_draw_linear_required(int mode);

/* The early return of deinterlace_render(): true without the current or the
 * previous texture, or with a width or height of 0. */
bool obs_source_deinterlace_draw_skip(bool has_cur_tex, bool has_prev_tex, uint32_t width, uint32_t height);

/* Whether deinterlace_render() reads gs_get_linear_srgb() for a source in
 * source_space: only for GS_CS_SRGB. */
bool obs_source_deinterlace_draw_reads_linear_srgb(int source_space);

/* Whether deinterlace_render() draws with linear sRGB: a source not in
 * GS_CS_SRGB, gs_linear_srgb (gs_get_linear_srgb(), false when it was not
 * read), or a mode that needs it. */
bool obs_source_deinterlace_draw_linear_srgb(int source_space, bool gs_linear_srgb, int mode);

/* The technique for a source in source_space drawn into current_space:
 * DrawMultiply for SDR (GS_CS_SRGB, GS_CS_SRGB_16F) or GS_CS_709_EXTENDED into
 * GS_CS_709_SCRGB, DrawTonemap for GS_CS_709_EXTENDED into SDR,
 * DrawMultiplyTonemap for GS_CS_709_SCRGB into SDR, DrawMultiply for
 * GS_CS_709_SCRGB into GS_CS_709_EXTENDED, Draw for any other pair. */
enum obs_source_draw_technique obs_source_deinterlace_draw_technique(int source_space, int current_space);

/* Whether deinterlace_render() reads obs_get_video_sdr_white_level() for the
 * pair: when the technique multiplies. */
bool obs_source_deinterlace_draw_reads_white_level(int source_space, int current_space);

/* The multiplier for the pair: sdr_white_level / 80.0f into
 * GS_CS_709_SCRGB from SDR or GS_CS_709_EXTENDED, 80.0f / sdr_white_level
 * from GS_CS_709_SCRGB into SDR or GS_CS_709_EXTENDED, 1.0f for any other
 * pair. One float division, so a white level of 0 gives +inf from
 * GS_CS_709_SCRGB and a NaN stays a NaN. */
float obs_source_deinterlace_draw_multiplier(int source_space, int current_space, float sdr_white_level);

/* Whether the second field is due: video_time at or after frame_ts + offset
 * + half_duration - TWOX_TOLERANCE, in uint64_t arithmetic that wraps (#395,
 * #1995, #2048). */
bool obs_source_deinterlace_draw_frame2(uint64_t video_time, uint64_t frame_ts, uint64_t offset,
					uint32_t half_duration);

/* What obs_source_set_deinterlace_mode() does from the source's current
 * mode: a mode outside the enum is DISABLE (#2090); then nothing for the
 * current mode, enable from DISABLE, disable to DISABLE, else switch. */
enum obs_source_deinterlace_draw_mode_change obs_source_deinterlace_draw_mode_change(int current_mode, int mode);
