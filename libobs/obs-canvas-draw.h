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

/* The canvas texture draw and display clear decisions, on plain values
 * (obs-canvas-draw.c):
 * - obs_render_canvas_texture_internal() (obs.c): whether a canvas draws at
 *   all, and the technique and multiplier for each pair of source and
 *   current colour spaces;
 * - render_display_begin() (obs-display.c): how the background colour
 *   converts to the clear colour, the clear flags, and whether the solid
 *   colour workaround draws.
 *
 * obs.c and obs-display.c keep every gs_* call in its order and under the
 * same conditions, the effect and param lookups, the blend state,
 * obs_get_video_sdr_white_level(), obs_host_display_clear_color_always_srgb(),
 * the swapchain, the resize and the viewport: they pass their results in and
 * act on what comes back. obs_get_video_sdr_white_level() is called only when
 * obs_canvas_draw_reads_white_level() says so, and gs_get_color_space() for
 * the clear colour only when obs_display_clear_reads_space() says so, as
 * before. None of these functions takes a lock, calls into libobs or keeps
 * state.
 *
 * Colour spaces are enum gs_color_space values, passed as int. A value
 * outside the enum takes the paths it took before. The techniques are enum
 * obs_source_draw_technique (obs-source-draw-technique.h), which the source
 * draw decisions share; these return Draw, DrawMultiply and DrawTonemap only.
 *
 * Hidden in libobs: not EXPORT. */

/* GS_CLEAR_COLOR, GS_CLEAR_DEPTH and GS_CLEAR_STENCIL (graphics/graphics.h),
 * copied so this header needs no graphics headers; obs-display.c checks them. */
#define OBS_DISPLAY_CLEAR_COLOR (1 << 0)
#define OBS_DISPLAY_CLEAR_DEPTH (1 << 1)
#define OBS_DISPLAY_CLEAR_STENCIL (1 << 2)

/* Whether obs_render_canvas_texture_internal() returns before drawing: the
 * canvas has no mix (has_mix false), or its mix has not rendered its texture.
 * obs.c reads texture_rendered only when there is a mix, and passes false
 * otherwise. */
bool obs_canvas_draw_skip(bool has_mix, bool texture_rendered);

/* The technique the canvas texture draws with:
 * - DrawTonemap for a GS_CS_709_EXTENDED source into SDR (GS_CS_SRGB,
 *   GS_CS_SRGB_16F);
 * - DrawMultiply into GS_CS_709_SCRGB, from any source;
 * - Draw for any other pair, a space outside the enum included. */
enum obs_source_draw_technique obs_canvas_draw_technique(int source_space, int current_space);

/* Whether obs_render_canvas_texture_internal() reads
 * obs_get_video_sdr_white_level(): into GS_CS_709_SCRGB only. */
bool obs_canvas_draw_reads_white_level(int current_space);

/* The multiplier into GS_CS_709_SCRGB: sdr_white_level / 80.0f. One float
 * division, so a white level of 0 gives 0 and a NaN stays a NaN. Any other
 * current space keeps 1.0f, which obs.c sets without asking. */
float obs_canvas_draw_multiplier(float sdr_white_level);

/* Whether render_display_begin() asks gs_get_color_space() for the clear
 * colour: only when the host does not always clear in sRGB
 * (obs_host_display_clear_color_always_srgb(), asked first). */
bool obs_display_clear_reads_space(bool always_srgb);

/* Whether the background colour converts to a linear clear colour
 * (vec4_from_rgba()) rather than sRGB (vec4_from_rgba_srgb()): only into
 * GS_CS_SRGB. */
bool obs_display_clear_linear(int current_space);

/* The flags render_display_begin() clears with: depth and stencil always,
 * colour too without the clear workaround. */
uint32_t obs_display_clear_flags(bool use_clear_workaround);

/* Whether render_display_begin() draws the clear colour with the solid
 * effect after the viewport: only with the clear workaround. */
bool obs_display_clear_draws_solid(bool use_clear_workaround);
