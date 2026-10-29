/* Test-only oracle: the original libobs/obs-source-duplicate.c.h,
 * unmodified, with its fifteen functions renamed to oracle_* so they can
 * link next to the Rust shims. */
#include <stddef.h>

#define obs_source_duplicate_type_step oracle_obs_source_duplicate_type_step
#define obs_source_duplicate_scene_step oracle_obs_source_duplicate_scene_step
#define obs_source_duplicate_group_step oracle_obs_source_duplicate_group_step
#define obs_source_duplicate_scene_type oracle_obs_source_duplicate_scene_type
#define obs_source_duplicate_flags_step oracle_obs_source_duplicate_flags_step
#define obs_source_duplicate_created_step oracle_obs_source_duplicate_created_step
#define obs_source_duplicate_copies_filters oracle_obs_source_duplicate_copies_filters
#define obs_source_duplicate_filter_number oracle_obs_source_duplicate_filter_number
#define obs_scene_duplicate_unique oracle_obs_scene_duplicate_unique
#define obs_scene_duplicate_private oracle_obs_scene_duplicate_private
#define obs_scene_duplicate_keeps_unique oracle_obs_scene_duplicate_keeps_unique
#define obs_scene_duplicate_item_route oracle_obs_scene_duplicate_item_route
#define obs_scene_duplicate_reuse oracle_obs_scene_duplicate_reuse
#define obs_scene_duplicate_keeps_name oracle_obs_scene_duplicate_keeps_name
#define obs_scene_duplicate_resizes oracle_obs_scene_duplicate_resizes

#include "obs-source-duplicate.c.h"

#undef obs_scene_duplicate_resizes
#undef obs_scene_duplicate_keeps_name
#undef obs_scene_duplicate_reuse
#undef obs_scene_duplicate_item_route
#undef obs_scene_duplicate_keeps_unique
#undef obs_scene_duplicate_private
#undef obs_scene_duplicate_unique
#undef obs_source_duplicate_filter_number
#undef obs_source_duplicate_copies_filters
#undef obs_source_duplicate_created_step
#undef obs_source_duplicate_flags_step
#undef obs_source_duplicate_scene_type
#undef obs_source_duplicate_group_step
#undef obs_source_duplicate_scene_step
#undef obs_source_duplicate_type_step

/* The values the C uses, in the order of
 * rust/obs-c-oracle/src/source_duplicate.rs. */
enum {
	ORACLE_SOURCE_DUPLICATE_LAYOUT_COUNT = 25,
};

size_t oracle_source_duplicate_layout(size_t which);

size_t oracle_source_duplicate_layout(size_t which)
{
	const size_t values[ORACLE_SOURCE_DUPLICATE_LAYOUT_COUNT] = {
		OBS_SOURCE_DUPLICATE_TYPE_FILTER,
		OBS_SOURCE_DUPLICATE_TYPE_SCENE,
		OBS_SOURCE_DUPLICATE_DO_NOT_DUPLICATE,
		OBS_SOURCE_DUPLICATE_DUP_REFS,
		OBS_SOURCE_DUPLICATE_DUP_COPY,
		OBS_SOURCE_DUPLICATE_DUP_PRIVATE_REFS,
		OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY,
		OBS_SOURCE_DUPLICATE_NONE,
		OBS_SOURCE_DUPLICATE_REF,
		OBS_SOURCE_DUPLICATE_ASK_SCENE,
		OBS_SOURCE_DUPLICATE_ASK_GROUP,
		OBS_SOURCE_DUPLICATE_SCENE_COPY,
		OBS_SOURCE_DUPLICATE_ASK_FLAGS,
		OBS_SOURCE_DUPLICATE_CREATE,
		OBS_SOURCE_DUPLICATE_COPY_STATE,
		OBS_SCENE_DUPLICATE_ITEM_GROUP,
		OBS_SCENE_DUPLICATE_ITEM_CHILD,
		OBS_SCENE_DUPLICATE_ITEM_REF,
		OBS_SCENE_DUPLICATE_REUSE_NONE,
		OBS_SCENE_DUPLICATE_REUSE_AT_INDEX,
		OBS_SCENE_DUPLICATE_REUSE_COPY,
		sizeof(enum obs_source_duplicate_step),
		sizeof(enum obs_scene_duplicate_item),
		/* the #define above renamed this tag with its function */
		sizeof(enum oracle_obs_scene_duplicate_reuse),
		sizeof(bool),
	};
	return which < ORACLE_SOURCE_DUPLICATE_LAYOUT_COUNT ? values[which] : 0;
}
