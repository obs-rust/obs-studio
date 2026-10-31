/* Test-only oracle: the original libobs/graphics/effect-param-types.c.h,
 * unmodified, with its seven functions renamed to oracle_* so they can link
 * next to the Rust shims (the route by a function-like macro, which leaves
 * the enum tag of the same name alone). astrcmp_n is the C dstr.c, already
 * renamed in oracle/dstr.c, so the oracle stays all C. */
#include <stddef.h>

#define obs_effect_param_type oracle_obs_effect_param_type
#define obs_effect_param_route(...) oracle_obs_effect_param_route(__VA_ARGS__)
#define obs_effect_param_rows_bad oracle_obs_effect_param_rows_bad
#define obs_effect_param_rows oracle_obs_effect_param_rows
#define obs_effect_param_has_cols oracle_obs_effect_param_has_cols
#define obs_effect_param_cols_bad oracle_obs_effect_param_cols_bad
#define obs_effect_param_cols oracle_obs_effect_param_cols
#define astrcmp_n oracle_astrcmp_n

#include "graphics/effect-param-types.c.h"

#undef astrcmp_n
#undef obs_effect_param_cols
#undef obs_effect_param_cols_bad
#undef obs_effect_param_has_cols
#undef obs_effect_param_rows
#undef obs_effect_param_rows_bad
#undef obs_effect_param_route
#undef obs_effect_param_type

/* The header's enums and the sizes and alignments of the types that cross,
 * in the order of rust/obs-c-oracle/src/effect_param_types.rs. */
enum {
	ORACLE_EFFECT_PARAM_LAYOUT_COUNT = 26,
};

size_t oracle_effect_param_types_layout(size_t which);

size_t oracle_effect_param_types_layout(size_t which)
{
	const size_t values[ORACLE_EFFECT_PARAM_LAYOUT_COUNT] = {
		GS_SHADER_PARAM_UNKNOWN,
		GS_SHADER_PARAM_BOOL,
		GS_SHADER_PARAM_FLOAT,
		GS_SHADER_PARAM_INT,
		GS_SHADER_PARAM_STRING,
		GS_SHADER_PARAM_VEC2,
		GS_SHADER_PARAM_VEC3,
		GS_SHADER_PARAM_VEC4,
		GS_SHADER_PARAM_INT2,
		GS_SHADER_PARAM_INT3,
		GS_SHADER_PARAM_INT4,
		GS_SHADER_PARAM_MATRIX4X4,
		GS_SHADER_PARAM_TEXTURE,
		OBS_EFFECT_PARAM_ROUTE_TEXTURE,
		OBS_EFFECT_PARAM_ROUTE_INT,
		OBS_EFFECT_PARAM_ROUTE_FLOAT,
		OBS_EFFECT_PARAM_ROUTE_INT_ARRAY,
		OBS_EFFECT_PARAM_ROUTE_FLOAT_ARRAY,
		OBS_EFFECT_PARAM_ROUTE_STRING,
		OBS_EFFECT_PARAM_ROUTE_BOOL,
		OBS_EFFECT_PARAM_ROUTE_INVALID,
		sizeof(bool),
		_Alignof(bool),
		sizeof(int),
		_Alignof(int),
		sizeof(enum obs_effect_param_route),
	};
	return which < ORACLE_EFFECT_PARAM_LAYOUT_COUNT ? values[which] : 0;
}
