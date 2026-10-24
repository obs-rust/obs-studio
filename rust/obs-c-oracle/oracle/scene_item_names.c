/* Test-only oracle: the original libobs/obs-scene-item-names.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next
 * to the Rust shims. The loaders call the compare they are given; the
 * parity test gives them the C dstr.c's astrcmpi (oracle_astrcmpi, from
 * oracle/dstr.c), so the oracle stays all C. */
#include <stddef.h>

#define obs_scene_item_scale_filter_from_name oracle_obs_scene_item_scale_filter_from_name
#define obs_scene_item_blend_method_from_name oracle_obs_scene_item_blend_method_from_name
#define obs_scene_item_blend_type_from_name oracle_obs_scene_item_blend_type_from_name
#define obs_scene_item_scale_filter_name oracle_obs_scene_item_scale_filter_name
#define obs_scene_item_blend_method_name oracle_obs_scene_item_blend_method_name
#define obs_scene_item_blend_type_name oracle_obs_scene_item_blend_type_name
#define obs_scene_item_transform_keys oracle_obs_scene_item_transform_keys
#define obs_scene_item_bounds_keys oracle_obs_scene_item_bounds_keys
#define obs_scene_item_save_relative oracle_obs_scene_item_save_relative

#include "obs-scene-item-names.c.h"

#undef obs_scene_item_save_relative
#undef obs_scene_item_bounds_keys
#undef obs_scene_item_transform_keys
#undef obs_scene_item_blend_type_name
#undef obs_scene_item_blend_method_name
#undef obs_scene_item_scale_filter_name
#undef obs_scene_item_blend_type_from_name
#undef obs_scene_item_blend_method_from_name
#undef obs_scene_item_scale_filter_from_name

/* The values the C uses, in the order of
 * rust/obs-c-oracle/src/scene_item_names.rs. */
enum {
	ORACLE_SCENE_ITEM_NAMES_LAYOUT_COUNT = 19,
};

size_t oracle_scene_item_names_layout(size_t which);

size_t oracle_scene_item_names_layout(size_t which)
{
	const size_t values[ORACLE_SCENE_ITEM_NAMES_LAYOUT_COUNT] = {
		/* enum obs_scene_item_keys: 0-3 */
		sizeof(enum obs_scene_item_keys),
		OBS_SCENE_ITEM_KEYS_RELATIVE,
		OBS_SCENE_ITEM_KEYS_FROM_ABSOLUTE,
		OBS_SCENE_ITEM_KEYS_ABSOLUTE,
		/* enum obs_scale_type: 4-9 */
		OBS_SCENE_ITEM_SCALE_DISABLE,
		OBS_SCENE_ITEM_SCALE_POINT,
		OBS_SCENE_ITEM_SCALE_BICUBIC,
		OBS_SCENE_ITEM_SCALE_BILINEAR,
		OBS_SCENE_ITEM_SCALE_LANCZOS,
		OBS_SCENE_ITEM_SCALE_AREA,
		/* enum obs_blending_method: 10-11 */
		OBS_SCENE_ITEM_BLEND_METHOD_DEFAULT,
		OBS_SCENE_ITEM_BLEND_METHOD_SRGB_OFF,
		/* enum obs_blending_type: 12-18 */
		OBS_SCENE_ITEM_BLEND_NORMAL,
		OBS_SCENE_ITEM_BLEND_ADDITIVE,
		OBS_SCENE_ITEM_BLEND_SUBTRACT,
		OBS_SCENE_ITEM_BLEND_SCREEN,
		OBS_SCENE_ITEM_BLEND_MULTIPLY,
		OBS_SCENE_ITEM_BLEND_LIGHTEN,
		OBS_SCENE_ITEM_BLEND_DARKEN,
	};
	return which < ORACLE_SCENE_ITEM_NAMES_LAYOUT_COUNT ? values[which] : 0;
}
