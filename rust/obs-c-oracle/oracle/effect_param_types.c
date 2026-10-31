/* Test-only oracle: the original libobs/graphics/effect-param-types.c.h,
 * unmodified, with its three functions renamed to oracle_* so they can link
 * next to the Rust shims. astrcmp_n is the C dstr.c, already renamed in
 * oracle/dstr.c, so the oracle stays all C. */
#include <stddef.h>

#define obs_effect_param_type oracle_obs_effect_param_type
#define obs_effect_param_route oracle_obs_effect_param_route
#define obs_effect_param_array_shape oracle_obs_effect_param_array_shape
#define astrcmp_n oracle_astrcmp_n

#include "graphics/effect-param-types.c.h"

#undef astrcmp_n
#undef obs_effect_param_array_shape
#undef obs_effect_param_route
#undef obs_effect_param_type

/* The header's enums and the layout of struct obs_effect_param_shape, in the
 * order of rust/obs-c-oracle/src/effect_param_types.rs. */
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
		sizeof(struct obs_effect_param_shape),
		_Alignof(struct obs_effect_param_shape),
		offsetof(struct obs_effect_param_shape, count),
		offsetof(struct obs_effect_param_shape, bad_rows),
		offsetof(struct obs_effect_param_shape, bad_cols),
	};
	return which < ORACLE_EFFECT_PARAM_LAYOUT_COUNT ? values[which] : 0;
}
