#include "libobs/build/libobs.pch.h"

/* The fields obs_source_get_color_space() reads (the obs-source shims,
 * #5724), no EXPORT. The Rust shell calls data_valid first and passes on
 * only the sources these return; the caller keeps the source alive. */
bool obs_source_get_color_space_data_valid(const obs_source_t *source);
bool obs_source_get_color_space_data_valid(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_get_color_space") && source->context.data;
}

struct obs_source_color_space_node obs_source_get_color_space_node(const obs_source_t *source);
struct obs_source_color_space_node obs_source_get_color_space_node(const obs_source_t *source)
{
	const struct obs_source_color_space_node node = {
		.type = (int)source->info.type,
		.output_flags = source->info.output_flags,
		.has_data = source->context.data != NULL,
		.enabled = source->enabled,
		.has_filter_parent = source->filter_parent != NULL,
		.has_filter_target = source->filter_target != NULL,
	};
	return node;
}

obs_source_t *obs_source_get_color_space_filter_parent(const obs_source_t *source);
obs_source_t *obs_source_get_color_space_filter_parent(const obs_source_t *source)
{
	return source->filter_parent;
}

obs_source_t *obs_source_get_color_space_filter_target(const obs_source_t *source);
obs_source_t *obs_source_get_color_space_filter_target(const obs_source_t *source)
{
	return source->filter_target;
}

int obs_source_get_color_space_async_space(const obs_source_t *source);
int obs_source_get_color_space_async_space(const obs_source_t *source)
{
	return (int)convert_video_space(source->async_format, source->async_trc);
}

/* False, with *space untouched, when the source has no callback. */
bool obs_source_get_color_space_callback(obs_source_t *source, size_t count,
					 const enum gs_color_space *preferred_spaces, enum gs_color_space *space);
bool obs_source_get_color_space_callback(obs_source_t *source, size_t count,
					 const enum gs_color_space *preferred_spaces, enum gs_color_space *space)
{
	assert(source->context.data);
	if (!source->info.video_get_color_space)
		return false;
	*space = source->info.video_get_color_space(source->context.data, count, preferred_spaces);
	return true;
}
