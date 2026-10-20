/* Test-only oracle: the original libobs/obs-scene-blend.c.h, unmodified,
 * with its table renamed to oracle_obs_blend_mode_params so it can link next
 * to the Rust one. The accessors below give the parity test the table's row
 * count, the row layout and the enum values the rows hold. */
#include <stddef.h>

#define obs_blend_mode_params oracle_obs_blend_mode_params

#include "obs-scene-blend.c.h"

/* Rows in the table, from its definition. */
size_t oracle_scene_blend_rows(void);

size_t oracle_scene_blend_rows(void)
{
	return sizeof(oracle_obs_blend_mode_params) / sizeof(oracle_obs_blend_mode_params[0]);
}

const struct obs_blend_mode_param *oracle_scene_blend_table(void);

const struct obs_blend_mode_param *oracle_scene_blend_table(void)
{
	return oracle_obs_blend_mode_params;
}

/* sizeof, _Alignof and the five field offsets of struct
 * obs_blend_mode_param, in that order; (size_t)-1 past the end. */
size_t oracle_scene_blend_layout(size_t which);

size_t oracle_scene_blend_layout(size_t which)
{
	const size_t layout[] = {
		sizeof(struct obs_blend_mode_param),
		_Alignof(struct obs_blend_mode_param),
		offsetof(struct obs_blend_mode_param, src_color),
		offsetof(struct obs_blend_mode_param, src_alpha),
		offsetof(struct obs_blend_mode_param, dst_color),
		offsetof(struct obs_blend_mode_param, dst_alpha),
		offsetof(struct obs_blend_mode_param, op),
	};
	return which < sizeof(layout) / sizeof(layout[0]) ? layout[which] : (size_t)-1;
}

/* enum obs_blending_type, enum gs_blend_type and enum gs_blend_op_type, in
 * declaration order: value `which` of `kind` (0, 1 or 2), -1 past the end. */
int oracle_scene_blend_enum(int kind, size_t which);

int oracle_scene_blend_enum(int kind, size_t which)
{
	static const int modes[] = {
		OBS_BLEND_NORMAL,   OBS_BLEND_ADDITIVE, OBS_BLEND_SUBTRACT, OBS_BLEND_SCREEN,
		OBS_BLEND_MULTIPLY, OBS_BLEND_LIGHTEN,  OBS_BLEND_DARKEN,
	};
	static const int types[] = {
		GS_BLEND_ZERO,     GS_BLEND_ONE,         GS_BLEND_SRCCOLOR,    GS_BLEND_INVSRCCOLOR,
		GS_BLEND_SRCALPHA, GS_BLEND_INVSRCALPHA, GS_BLEND_DSTCOLOR,    GS_BLEND_INVDSTCOLOR,
		GS_BLEND_DSTALPHA, GS_BLEND_INVDSTALPHA, GS_BLEND_SRCALPHASAT,
	};
	static const int ops[] = {
		GS_BLEND_OP_ADD, GS_BLEND_OP_SUBTRACT, GS_BLEND_OP_REVERSE_SUBTRACT, GS_BLEND_OP_MIN, GS_BLEND_OP_MAX,
	};

	switch (kind) {
	case 0:
		return which < sizeof(modes) / sizeof(modes[0]) ? modes[which] : -1;
	case 1:
		return which < sizeof(types) / sizeof(types[0]) ? types[which] : -1;
	case 2:
		return which < sizeof(ops) / sizeof(ops[0]) ? ops[which] : -1;
	default:
		return -1;
	}
}
