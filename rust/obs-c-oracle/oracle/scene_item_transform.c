/* Test-only oracle: the original libobs/obs-scene-item-transform.c.h,
 * unmodified, with its four functions renamed to oracle_* so they can link
 * next to the Rust shims. The matrix4, quat and axisang calls are the
 * graphics oracle (graphics_math_names.h) and obs_scene_cropped_size and
 * obs_scene_scale_crop the crop oracle, so every call stays C.
 * matrix4_identity, the *3f wrappers and vec3_set stay the header inlines. */
#include <stddef.h>

#include "graphics_math_names.h"

#define obs_scene_cropped_size oracle_obs_scene_cropped_size
#define obs_scene_scale_crop oracle_obs_scene_scale_crop
#define obs_scene_item_transform_prepare oracle_obs_scene_item_transform_prepare
#define obs_scene_item_transform_bounded oracle_obs_scene_item_transform_bounded
#define obs_scene_item_transform_matrix oracle_obs_scene_item_transform_matrix
#define obs_scene_item_transform_box_scale oracle_obs_scene_item_transform_box_scale

#include "obs-scene-item-transform.c.h"

#undef obs_scene_item_transform_box_scale
#undef obs_scene_item_transform_matrix
#undef obs_scene_item_transform_bounded
#undef obs_scene_item_transform_prepare
#undef obs_scene_scale_crop
#undef obs_scene_cropped_size

/* The layouts and values the C uses, in the order of
 * rust/obs-c-oracle/src/scene_item_transform.rs. */
enum {
	ORACLE_SCENE_ITEM_TRANSFORM_LAYOUT_COUNT = 6,
};

size_t oracle_scene_item_transform_layout(size_t which);

size_t oracle_scene_item_transform_layout(size_t which)
{
	const size_t values[ORACLE_SCENE_ITEM_TRANSFORM_LAYOUT_COUNT] = {
		sizeof(struct obs_scene_item_transform_size),
		_Alignof(struct obs_scene_item_transform_size),
		offsetof(struct obs_scene_item_transform_size, cx),
		offsetof(struct obs_scene_item_transform_size, cy),
		OBS_BOUNDS_NONE,
		sizeof(enum obs_bounds_type),
	};
	return which < ORACLE_SCENE_ITEM_TRANSFORM_LAYOUT_COUNT ? values[which] : 0;
}
