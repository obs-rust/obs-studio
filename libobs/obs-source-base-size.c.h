#include "libobs/build/libobs.pch.h"

/* The fields obs_source_size_route() reads; has_callback is the size
 * callback of the axis asked for. */
static inline struct obs_source_size_node size_node(const obs_source_t *source, bool has_callback)
{
	const struct obs_source_size_node node = {
		.type = (int)source->info.type,
		.has_data = source->context.data != NULL,
		.has_callback = has_callback,
		.has_filter_parent = source->filter_parent != NULL,
		.enabled = source->enabled,
		.async_active = os_atomic_load_bool(&source->async_active),
		.async_rotation = source->async_rotation,
	};
	return node;
}

uint32_t obs_source_base_width(const obs_source_t *source)
{
	switch (obs_source_size_route(size_node(source, source->info.get_width != NULL))) {
	case OBS_SOURCE_SIZE_TRANSITION:
		return (uint32_t)os_atomic_load_long(&source->transition_actual_cx);
	case OBS_SOURCE_SIZE_CALLBACK:
		return source->info.get_width(source->context.data);
	case OBS_SOURCE_SIZE_ASK_TARGET:
		/* A filter being attached or removed can have a parent and no
		 * target yet (#2172). */
		return source->filter_target ? obs_source_base_width(source->filter_target) : 0;
	case OBS_SOURCE_SIZE_ASYNC:
		return source->async_width;
	case OBS_SOURCE_SIZE_ASYNC_SWAPPED:
		return source->async_height;
	case OBS_SOURCE_SIZE_ZERO:
		break;
	}

	return 0;
}

uint32_t obs_source_base_height(const obs_source_t *source)
{
	switch (obs_source_size_route(size_node(source, source->info.get_height != NULL))) {
	case OBS_SOURCE_SIZE_TRANSITION:
		return (uint32_t)os_atomic_load_long(&source->transition_actual_cy);
	case OBS_SOURCE_SIZE_CALLBACK:
		return source->info.get_height(source->context.data);
	case OBS_SOURCE_SIZE_ASK_TARGET:
		/* A filter being attached or removed can have a parent and no
		 * target yet (#2172). */
		return source->filter_target ? obs_source_base_height(source->filter_target) : 0;
	case OBS_SOURCE_SIZE_ASYNC:
		return source->async_height;
	case OBS_SOURCE_SIZE_ASYNC_SWAPPED:
		return source->async_width;
	case OBS_SOURCE_SIZE_ZERO:
		break;
	}

	return 0;
}

#define data_valid(source, f) (obs_source_valid(source, f) && (source)->context.data)

uint32_t obs_source_get_base_width(obs_source_t *source)
{
	if (!data_valid(source, "obs_source_get_base_width"))
		return 0;

	return obs_source_base_width(source);
}

uint32_t obs_source_get_base_height(obs_source_t *source)
{
	if (!data_valid(source, "obs_source_get_base_height"))
		return 0;

	return obs_source_base_height(source);
}

#undef data_valid
