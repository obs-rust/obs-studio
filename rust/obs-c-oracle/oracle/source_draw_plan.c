/* Test-only oracle: the original libobs/obs-source-draw-plan.c.h,
 * unmodified, with its eight functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <stddef.h>
#include <stdint.h>

#define obs_source_draw_plan_rotation_x oracle_obs_source_draw_plan_rotation_x
#define obs_source_draw_plan_rotation_y oracle_obs_source_draw_plan_rotation_y
#define obs_source_draw_plan_async_linear_srgb oracle_obs_source_draw_plan_async_linear_srgb
#define obs_source_draw_plan_async_nonlinear_alpha oracle_obs_source_draw_plan_async_nonlinear_alpha
#define obs_source_draw_plan_technique oracle_obs_source_draw_plan_technique
#define obs_source_draw_plan_reads_white_level oracle_obs_source_draw_plan_reads_white_level
#define obs_source_draw_plan_multiplier oracle_obs_source_draw_plan_multiplier
#define obs_source_draw_plan_convert oracle_obs_source_draw_plan_convert

#include "obs-source-draw-plan.c.h"

#undef obs_source_draw_plan_convert
#undef obs_source_draw_plan_multiplier
#undef obs_source_draw_plan_reads_white_level
#undef obs_source_draw_plan_technique
#undef obs_source_draw_plan_async_nonlinear_alpha
#undef obs_source_draw_plan_async_linear_srgb
#undef obs_source_draw_plan_rotation_y
#undef obs_source_draw_plan_rotation_x

/* The technique's name as obs-source.c maps it, for a value the port
 * returns. */
const char *oracle_source_draw_plan_technique_name(int technique);

const char *oracle_source_draw_plan_technique_name(int technique)
{
	return obs_source_draw_technique_name((enum obs_source_draw_technique)technique);
}

/* The enum values and sizes the port relies on, in the order of
 * rust/obs-c-oracle/src/source_draw_plan.rs; INT64_MIN past the end. */
int64_t oracle_source_draw_plan_value(size_t which);

int64_t oracle_source_draw_plan_value(size_t which)
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
		OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY_TONEMAP,
		OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA,
		OBS_SOURCE_DRAW_TECH_DRAW_NONLINEAR_ALPHA_MULTIPLY,
		/* sizes */
		sizeof(enum gs_color_space),
		sizeof(enum obs_source_draw_technique),
		sizeof(int),
		sizeof(long),
		sizeof(bool),
		sizeof(float),
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT64_MIN;
}
