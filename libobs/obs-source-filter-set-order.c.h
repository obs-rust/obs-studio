#include "libobs/build/libobs.pch.h"

static size_t find_next_filter(obs_source_t *source, obs_source_t *filter, size_t cur_idx)
{
	uint32_t cur_flags = filter->info.output_flags;
	size_t next_idx;
	obs_source_t *next;

	if (!obs_source_filter_chain_walk_step(OBS_SOURCE_FILTER_CHAIN_ROUTE_UP, cur_idx, source->filters.num,
					       &next_idx))
		return DARRAY_INVALID;

	next = source->filters.array[next_idx];

	if (obs_source_filter_chain_same_kind(cur_flags, next->info.output_flags))
		return next_idx;
	else
		return find_next_filter(source, filter, next_idx);
}

static size_t find_prev_filter(obs_source_t *source, obs_source_t *filter, size_t cur_idx)
{
	uint32_t cur_flags = filter->info.output_flags;
	size_t prev_idx;
	obs_source_t *prev;

	if (!obs_source_filter_chain_walk_step(OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN, cur_idx, source->filters.num,
					       &prev_idx))
		return DARRAY_INVALID;

	prev = source->filters.array[prev_idx];

	if (obs_source_filter_chain_same_kind(cur_flags, prev->info.output_flags))
		return prev_idx;
	else
		return find_prev_filter(source, filter, prev_idx);
}

/* moves filters above/below matching filter types */
static bool move_filter_dir(obs_source_t *source, obs_source_t *filter, enum obs_order_movement movement)
{
	size_t idx;
	size_t to;
	int route;

	idx = da_find(source->filters, &filter, 0);
	if (idx == DARRAY_INVALID)
		return false;

	route = obs_source_filter_chain_move_route((int)movement);

	if (route == OBS_SOURCE_FILTER_CHAIN_ROUTE_UP) {
		size_t next_id = find_next_filter(source, filter, idx);
		if (next_id == DARRAY_INVALID)
			return false;
		da_move_item(source->filters, idx, next_id);

	} else if (route == OBS_SOURCE_FILTER_CHAIN_ROUTE_DOWN) {
		size_t prev_id = find_prev_filter(source, filter, idx);
		if (prev_id == DARRAY_INVALID)
			return false;
		da_move_item(source->filters, idx, prev_id);

	} else if (route == OBS_SOURCE_FILTER_CHAIN_ROUTE_TOP || route == OBS_SOURCE_FILTER_CHAIN_ROUTE_BOTTOM) {
		if (!obs_source_filter_chain_edge_move(route, idx, source->filters.num, &to))
			return false;
		da_move_item(source->filters, idx, to);

	} else if (route == OBS_SOURCE_FILTER_CHAIN_ROUTE_STAY) {
		return false;
	}

	reorder_filter_targets(source);

	return true;
}

void obs_source_filter_set_order(obs_source_t *source, obs_source_t *filter, enum obs_order_movement movement)
{
	bool success;

	if (!obs_source_valid(source, "obs_source_filter_set_order"))
		return;
	if (!obs_ptr_valid(filter, "obs_source_filter_set_order"))
		return;

	pthread_mutex_lock(&source->filter_mutex);
	success = move_filter_dir(source, filter, movement);
	pthread_mutex_unlock(&source->filter_mutex);

	if (success)
		obs_source_dosignal(source, NULL, "reorder_filters");
}
