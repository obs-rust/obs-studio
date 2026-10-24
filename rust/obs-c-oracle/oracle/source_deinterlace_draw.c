/* Test-only oracle: the original libobs/obs-source-deinterlace-draw.c.h,
 * unmodified, with its ten functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <stddef.h>
#include <stdint.h>

#define obs_source_deinterlace_draw_effect oracle_obs_source_deinterlace_draw_effect
#define obs_source_deinterlace_draw_linear_required oracle_obs_source_deinterlace_draw_linear_required
#define obs_source_deinterlace_draw_skip oracle_obs_source_deinterlace_draw_skip
#define obs_source_deinterlace_draw_reads_linear_srgb oracle_obs_source_deinterlace_draw_reads_linear_srgb
#define obs_source_deinterlace_draw_linear_srgb oracle_obs_source_deinterlace_draw_linear_srgb
#define obs_source_deinterlace_draw_technique oracle_obs_source_deinterlace_draw_technique
#define obs_source_deinterlace_draw_reads_white_level oracle_obs_source_deinterlace_draw_reads_white_level
#define obs_source_deinterlace_draw_multiplier oracle_obs_source_deinterlace_draw_multiplier
#define obs_source_deinterlace_draw_frame2 oracle_obs_source_deinterlace_draw_frame2
#define obs_source_deinterlace_draw_mode_change oracle_obs_source_deinterlace_draw_mode_change

#include "obs-source-deinterlace-draw.c.h"

#undef obs_source_deinterlace_draw_mode_change
#undef obs_source_deinterlace_draw_frame2
#undef obs_source_deinterlace_draw_multiplier
#undef obs_source_deinterlace_draw_reads_white_level
#undef obs_source_deinterlace_draw_technique
#undef obs_source_deinterlace_draw_linear_srgb
#undef obs_source_deinterlace_draw_reads_linear_srgb
#undef obs_source_deinterlace_draw_skip
#undef obs_source_deinterlace_draw_linear_required
#undef obs_source_deinterlace_draw_effect

/* The enum values and sizes the port relies on, in the order of
 * rust/obs-c-oracle/src/source_deinterlace_draw.rs; INT64_MIN past the end. */
int64_t oracle_source_deinterlace_draw_value(size_t which);

int64_t oracle_source_deinterlace_draw_value(size_t which)
{
	const int64_t values[] = {
		/* enum obs_deinterlace_mode */
		OBS_DEINTERLACE_MODE_DISABLE,
		OBS_DEINTERLACE_MODE_DISCARD,
		OBS_DEINTERLACE_MODE_RETRO,
		OBS_DEINTERLACE_MODE_BLEND,
		OBS_DEINTERLACE_MODE_BLEND_2X,
		OBS_DEINTERLACE_MODE_LINEAR,
		OBS_DEINTERLACE_MODE_LINEAR_2X,
		OBS_DEINTERLACE_MODE_YADIF,
		OBS_DEINTERLACE_MODE_YADIF_2X,
		/* enum gs_color_space */
		GS_CS_SRGB,
		GS_CS_SRGB_16F,
		GS_CS_709_EXTENDED,
		GS_CS_709_SCRGB,
		/* enum obs_source_deinterlace_draw_effect */
		OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_NONE,
		OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_DISCARD,
		OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_DISCARD_2X,
		OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_BLEND,
		OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_BLEND_2X,
		OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_LINEAR,
		OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_LINEAR_2X,
		OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_YADIF,
		OBS_SOURCE_DEINTERLACE_DRAW_EFFECT_YADIF_2X,
		/* enum obs_source_draw_technique */
		OBS_SOURCE_DRAW_TECH_DRAW,
		OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY,
		OBS_SOURCE_DRAW_TECH_DRAW_TONEMAP,
		OBS_SOURCE_DRAW_TECH_DRAW_MULTIPLY_TONEMAP,
		/* enum obs_source_deinterlace_draw_mode_change */
		OBS_SOURCE_DEINTERLACE_DRAW_MODE_SAME,
		OBS_SOURCE_DEINTERLACE_DRAW_MODE_ENABLE,
		OBS_SOURCE_DEINTERLACE_DRAW_MODE_DISABLE,
		OBS_SOURCE_DEINTERLACE_DRAW_MODE_SWITCH,
		/* TWOX_TOLERANCE */
		(int64_t)OBS_SOURCE_DEINTERLACE_PACING_TWOX_TOLERANCE,
		/* sizes; the renames above also renamed the two enum tags that
		 * share their function's name */
		sizeof(enum obs_deinterlace_mode),
		sizeof(enum gs_color_space),
		sizeof(enum oracle_obs_source_deinterlace_draw_effect),
		sizeof(enum obs_source_draw_technique),
		sizeof(enum oracle_obs_source_deinterlace_draw_mode_change),
		sizeof(int),
		sizeof(bool),
		sizeof(float),
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT64_MIN;
}
