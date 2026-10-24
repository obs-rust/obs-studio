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

/* The source draw decisions of obs-source.c, on plain values
 * (obs-source-draw-plan.c):
 * - rotate_async_video(): the translate that moves a rotated async frame
 *   back into its box (obs_source_draw_plan_rotation_x() and _y());
 * - obs_source_render_async_video(): the linear sRGB flag, the non-linear
 *   alpha blend, the technique and the multiplier for each pair of source and
 *   current colour spaces;
 * - source_render(): whether a source is drawn through color_space_texrender
 *   to convert its colour space, and the technique and multiplier of that
 *   draw.
 *
 * obs-source.c keeps every gs_* call in its order and under the same
 * conditions, convert_video_space(), obs_get_video_sdr_white_level(),
 * gs_get_format_from_space() and the texrenders: it passes their results in
 * and acts on what comes back. obs_get_video_sdr_white_level() is called only
 * when obs_source_draw_plan_reads_white_level() says so, as before. None of
 * these functions takes a lock, calls into libobs or keeps state.
 *
 * Colour spaces are enum gs_color_space values, passed as int; obs-source.c
 * checks the size. A value outside the enum takes the paths it took before.
 * The techniques are enum obs_source_draw_technique
 * (obs-source-draw-technique.h), which the deinterlace draw decisions share.
 *
 * Hidden in libobs: not EXPORT. */

/* The x of the translate before an async frame of width x height is rotated
 * by rotation degrees: height for 270, width for 180, 0 for any other
 * rotation (obs_source_set_async_rotation() keeps it in 0-359). The size
 * converts to float as (float)uint32_t does. */
float obs_source_draw_plan_rotation_x(long rotation, uint32_t width, uint32_t height);

/* The y of that translate: width for 90, height for 180, 0 for any other
 * rotation. */
float obs_source_draw_plan_rotation_y(long rotation, uint32_t width, uint32_t height);

/* Whether obs_source_render_async_video() draws with linear sRGB:
 * gs_linear_srgb (gs_get_linear_srgb(), always read), or also a GS_CS_SRGB
 * source into any current space but GS_CS_SRGB, or a GS_CS_709_EXTENDED or
 * GS_CS_709_SCRGB source into SDR (GS_CS_SRGB, GS_CS_SRGB_16F), which is
 * tonemapped. */
bool obs_source_draw_plan_async_linear_srgb(int source_space, int current_space, bool gs_linear_srgb);

/* Whether obs_source_render_async_video() blends with non-linear alpha
 * (GS_BLEND_ONE, GS_BLEND_INVSRCALPHA): a GS_CS_SRGB source drawn with linear
 * sRGB (obs_source_draw_plan_async_linear_srgb()) whose alpha is not linear
 * (async_linear_alpha). */
bool obs_source_draw_plan_async_nonlinear_alpha(int source_space, bool linear_srgb, bool linear_alpha);

/* The technique obs_source_render_async_video() draws with, and the one
 * source_render() converts with (nonlinear_alpha false):
 * - SDR (GS_CS_SRGB, GS_CS_SRGB_16F) into GS_CS_709_SCRGB: DrawMultiply, or
 *   DrawNonlinearAlphaMultiply with non-linear alpha;
 * - GS_CS_SRGB into GS_CS_SRGB, GS_CS_SRGB_16F or GS_CS_709_EXTENDED with
 *   non-linear alpha: DrawNonlinearAlpha;
 * - GS_CS_709_EXTENDED: DrawTonemap into SDR, DrawMultiply into
 *   GS_CS_709_SCRGB;
 * - GS_CS_709_SCRGB: DrawMultiplyTonemap into SDR, DrawMultiply into
 *   GS_CS_709_EXTENDED;
 * - Draw for any other pair, a space outside the enum included.
 * Only a GS_CS_SRGB source has non-linear alpha
 * (obs_source_draw_plan_async_nonlinear_alpha()); for any other source the
 * flag changes nothing. */
enum obs_source_draw_technique obs_source_draw_plan_technique(int source_space, int current_space,
							      bool nonlinear_alpha);

/* Whether obs_source_render_async_video() or source_render() reads
 * obs_get_video_sdr_white_level() for the pair: SDR or GS_CS_709_EXTENDED
 * into GS_CS_709_SCRGB, GS_CS_709_SCRGB into SDR or GS_CS_709_EXTENDED. */
bool obs_source_draw_plan_reads_white_level(int source_space, int current_space);

/* The multiplier for the pair: sdr_white_level / 80.0f into GS_CS_709_SCRGB
 * from SDR or GS_CS_709_EXTENDED, 80.0f / sdr_white_level from
 * GS_CS_709_SCRGB into SDR or GS_CS_709_EXTENDED, 1.0f for any other pair.
 * One float division, so a white level of 0 gives +inf from GS_CS_709_SCRGB
 * and a NaN stays a NaN. */
float obs_source_draw_plan_multiplier(int source_space, int current_space, float sdr_white_level);

/* Whether source_render() draws the source into color_space_texrender and
 * converts it: SDR into GS_CS_709_EXTENDED or GS_CS_709_SCRGB,
 * GS_CS_709_EXTENDED into SDR or GS_CS_709_SCRGB, GS_CS_709_SCRGB into SDR or
 * GS_CS_709_EXTENDED. Not for a pair in the same space class, nor a space
 * outside the enum. */
bool obs_source_draw_plan_convert(int source_space, int current_space);
