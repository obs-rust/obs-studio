/* Test-only oracle: the original libobs/obs-source-color-space.c.h,
 * unmodified, with its two functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <stddef.h>

#define obs_source_color_space_route oracle_obs_source_color_space_route
#define obs_source_color_space_pick oracle_obs_source_color_space_pick

#include "obs-source-color-space.c.h"

#undef obs_source_color_space_pick
#undef obs_source_color_space_route

/* The layouts and values the C uses, in the order of
 * rust/obs-c-oracle/src/source_color_space.rs. */
enum {
	ORACLE_SOURCE_COLOR_SPACE_LAYOUT_COUNT = 17,
};

size_t oracle_source_color_space_layout(size_t which);

size_t oracle_source_color_space_layout(size_t which)
{
	const size_t values[ORACLE_SOURCE_COLOR_SPACE_LAYOUT_COUNT] = {
		sizeof(struct obs_source_color_space_node),
		_Alignof(struct obs_source_color_space_node),
		offsetof(struct obs_source_color_space_node, type),
		offsetof(struct obs_source_color_space_node, output_flags),
		offsetof(struct obs_source_color_space_node, has_data),
		offsetof(struct obs_source_color_space_node, enabled),
		offsetof(struct obs_source_color_space_node, has_filter_parent),
		offsetof(struct obs_source_color_space_node, has_filter_target),
		sizeof(enum obs_source_color_space_ask),
		OBS_SOURCE_COLOR_SPACE_ASK_PARENT,
		OBS_SOURCE_COLOR_SPACE_ASK_TARGET,
		OBS_SOURCE_COLOR_SPACE_ASYNC_PICK,
		OBS_SOURCE_COLOR_SPACE_CALLBACK,
		OBS_SOURCE_COLOR_SPACE_TYPE_FILTER,
		OBS_SOURCE_COLOR_SPACE_VIDEO,
		OBS_SOURCE_COLOR_SPACE_ASYNC,
		sizeof(int),
	};
	return which < ORACLE_SOURCE_COLOR_SPACE_LAYOUT_COUNT ? values[which] : 0;
}
