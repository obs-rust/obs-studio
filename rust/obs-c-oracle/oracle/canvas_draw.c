/* Test-only oracle: the original libobs/obs-canvas-draw.c.h, unmodified,
 * with its eight functions renamed to oracle_* so they can link next to the
 * Rust shims. */
#include <stddef.h>
#include <stdint.h>

#define obs_canvas_draw_skip oracle_obs_canvas_draw_skip
#define obs_canvas_draw_technique oracle_obs_canvas_draw_technique
#define obs_canvas_draw_reads_white_level oracle_obs_canvas_draw_reads_white_level
#define obs_canvas_draw_multiplier oracle_obs_canvas_draw_multiplier
#define obs_display_clear_reads_space oracle_obs_display_clear_reads_space
#define obs_display_clear_linear oracle_obs_display_clear_linear
#define obs_display_clear_flags oracle_obs_display_clear_flags
#define obs_display_clear_draws_solid oracle_obs_display_clear_draws_solid

#include "obs-canvas-draw.c.h"

#undef obs_display_clear_draws_solid
#undef obs_display_clear_flags
#undef obs_display_clear_linear
#undef obs_display_clear_reads_space
#undef obs_canvas_draw_multiplier
#undef obs_canvas_draw_reads_white_level
#undef obs_canvas_draw_technique
#undef obs_canvas_draw_skip

/* The technique's name as obs.c maps it, for a value the port returns. */
const char *oracle_canvas_draw_technique_name(int technique);

const char *oracle_canvas_draw_technique_name(int technique)
{
	return obs_source_draw_technique_name((enum obs_source_draw_technique)technique);
}

/* The enum values, flags and sizes the port relies on, in the order of
 * rust/obs-c-oracle/src/canvas_draw.rs; INT64_MIN past the end. */
int64_t oracle_canvas_draw_value(size_t which);

int64_t oracle_canvas_draw_value(size_t which)
{
	const int64_t values[] = {
		/* enum gs_color_space */
		GS_CS_SRGB,
		GS_CS_SRGB_16F,
		GS_CS_709_EXTENDED,
		GS_CS_709_SCRGB,
		/* enum obs_source_draw_technique */
		OBS_SOURCE_DRAW_TECH_DRAW,
		OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY,
		OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP,
		/* the clear flags, copied and from graphics.h */
		OBS_DISPLAY_CLEAR_COLOR,
		OBS_DISPLAY_CLEAR_DEPTH,
		OBS_DISPLAY_CLEAR_STENCIL,
		GS_CLEAR_COLOR,
		GS_CLEAR_DEPTH,
		GS_CLEAR_STENCIL,
		/* sizes */
		sizeof(enum gs_color_space),
		sizeof(enum obs_source_draw_technique),
		sizeof(int),
		sizeof(bool),
		sizeof(float),
		sizeof(uint32_t),
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT64_MIN;
}
