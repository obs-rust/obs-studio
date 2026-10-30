/* Test-only oracle: the original libobs/obs-source-filter-chain.c.h,
 * unmodified, with its thirteen functions renamed to oracle_* so they can
 * link next to the Rust shims. */
#include <stddef.h>

#define obs_source_filter_chain_compatible oracle_obs_source_filter_chain_compatible
#define obs_source_filter_chain_add_allowed oracle_obs_source_filter_chain_add_allowed
#define obs_source_filter_chain_checks_compatible oracle_obs_source_filter_chain_checks_compatible
#define obs_source_filter_chain_ref_step oracle_obs_source_filter_chain_ref_step
#define obs_source_filter_chain_add_target oracle_obs_source_filter_chain_add_target
#define obs_source_filter_chain_remove_retarget oracle_obs_source_filter_chain_remove_retarget
#define obs_source_filter_chain_move_route oracle_obs_source_filter_chain_move_route
#define obs_source_filter_chain_walk_step oracle_obs_source_filter_chain_walk_step
#define obs_source_filter_chain_same_kind oracle_obs_source_filter_chain_same_kind
#define obs_source_filter_chain_edge_move oracle_obs_source_filter_chain_edge_move
#define obs_source_filter_chain_target_of oracle_obs_source_filter_chain_target_of
#define obs_source_filter_chain_restore_matches oracle_obs_source_filter_chain_restore_matches
#define obs_source_filter_chain_restore_step oracle_obs_source_filter_chain_restore_step

#include "obs-source-filter-chain.c.h"

#undef obs_source_filter_chain_restore_step
#undef obs_source_filter_chain_restore_matches
#undef obs_source_filter_chain_target_of
#undef obs_source_filter_chain_edge_move
#undef obs_source_filter_chain_same_kind
#undef obs_source_filter_chain_walk_step
#undef obs_source_filter_chain_move_route
#undef obs_source_filter_chain_remove_retarget
#undef obs_source_filter_chain_add_target
#undef obs_source_filter_chain_ref_step
#undef obs_source_filter_chain_checks_compatible
#undef obs_source_filter_chain_add_allowed
#undef obs_source_filter_chain_compatible

/* The values the C uses, in the order of
 * rust/obs-c-oracle/src/source_filter_chain.rs. */
enum {
	ORACLE_SOURCE_FILTER_CHAIN_LAYOUT_COUNT = 27,
};

size_t oracle_source_filter_chain_layout(size_t which);

size_t oracle_source_filter_chain_layout(size_t which)
{
	const size_t values[ORACLE_SOURCE_FILTER_CHAIN_LAYOUT_COUNT] = {
		OBS_SOURCE_FILTER_CHAIN_VIDEO,
		OBS_SOURCE_FILTER_CHAIN_AUDIO,
		OBS_SOURCE_FILTER_CHAIN_ASYNC,
		OBS_SOURCE_FILTER_CHAIN_MOVE_UP,
		OBS_SOURCE_FILTER_CHAIN_MOVE_DOWN,
		OBS_SOURCE_FILTER_CHAIN_MOVE_TOP,
		OBS_SOURCE_FILTER_CHAIN_MOVE_BOTTOM,
		OBS_SOURCE_FILTER_CHAIN_REF_ADD,
		OBS_SOURCE_FILTER_CHAIN_REF_RETURN,
		OBS_SOURCE_FILTER_CHAIN_REF_UNLOCK_RETURN,
		OBS_SOURCE_FILTER_CHAIN_TARGET_SOURCE,
		OBS_SOURCE_FILTER_CHAIN_TARGET_FILTER,
		OBS_SOURCE_FILTER_CHAIN_ROUTE_UP,
		OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN,
		OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP,
		OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM,
		OBS_SOURCE_FILTER_CHAIN_ROUTE_RETARGET,
		OBS_SOURCE_FILTER_CHAIN_ROUTE_STAY,
		OBS_SOURCE_FILTER_CHAIN_RESTORE_REUSE,
		OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD,
		OBS_SOURCE_FILTER_CHAIN_RESTORE_LOAD_PRIVATE,
		sizeof(enum obs_source_filter_chain_ref),
		sizeof(enum obs_source_filter_chain_target),
		sizeof(enum obs_source_filter_chain_route),
		sizeof(enum obs_source_filter_chain_restore),
		sizeof(bool),
		sizeof(size_t),
	};
	return which < ORACLE_SOURCE_FILTER_CHAIN_LAYOUT_COUNT ? values[which] : 0;
}
