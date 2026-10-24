/* Test-only oracle: the original libobs/obs-source-size.c.h, unmodified,
 * with its two functions renamed to oracle_* so they can link next to the
 * Rust shims. */
#include <stddef.h>

#define obs_source_size_route oracle_obs_source_size_route
#define obs_source_size_entry oracle_obs_source_size_entry

#include "obs-source-size.c.h"

#undef obs_source_size_entry
#undef obs_source_size_route

/* The layouts and values the C uses, in the order of
 * rust/obs-c-oracle/src/source_size.rs. */
enum {
	ORACLE_SOURCE_SIZE_LAYOUT_COUNT = 23,
};

size_t oracle_source_size_layout(size_t which);

size_t oracle_source_size_layout(size_t which)
{
	const size_t values[ORACLE_SOURCE_SIZE_LAYOUT_COUNT] = {
		sizeof(struct obs_source_size_node),
		_Alignof(struct obs_source_size_node),
		offsetof(struct obs_source_size_node, type),
		offsetof(struct obs_source_size_node, has_data),
		offsetof(struct obs_source_size_node, has_callback),
		offsetof(struct obs_source_size_node, has_filter_parent),
		offsetof(struct obs_source_size_node, enabled),
		offsetof(struct obs_source_size_node, async_active),
		offsetof(struct obs_source_size_node, async_rotation),
		sizeof(enum obs_source_size_ask),
		OBS_SOURCE_SIZE_TRANSITION,
		OBS_SOURCE_SIZE_CALLBACK,
		OBS_SOURCE_SIZE_ASK_TARGET,
		OBS_SOURCE_SIZE_ASYNC,
		OBS_SOURCE_SIZE_ASYNC_SWAPPED,
		OBS_SOURCE_SIZE_ZERO,
		sizeof(enum obs_source_size_start),
		OBS_SOURCE_SIZE_ENTRY_SELF,
		OBS_SOURCE_SIZE_ENTRY_FIRST_FILTER,
		OBS_SOURCE_SIZE_TYPE_FILTER,
		OBS_SOURCE_SIZE_TYPE_TRANSITION,
		sizeof(long),
		sizeof(int),
	};
	return which < ORACLE_SOURCE_SIZE_LAYOUT_COUNT ? values[which] : 0;
}
