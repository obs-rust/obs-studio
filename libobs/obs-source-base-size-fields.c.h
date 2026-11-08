#include "libobs/build/libobs.pch.h"

struct obs_source_base_size_fields obs_source_base_size_fields(const obs_source_t *source)
{
	return (struct obs_source_base_size_fields){&source->info.type,
						    &source->context.data,
						    &source->info.get_width,
						    &source->info.get_height,
						    &source->filter_parent,
						    &source->filter_target,
						    &source->enabled,
						    &source->async_active,
						    &source->async_rotation,
						    &source->transition_actual_cx,
						    &source->transition_actual_cy,
						    &source->async_width,
						    &source->async_height};
}
