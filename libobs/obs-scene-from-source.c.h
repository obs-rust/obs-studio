#include "libobs/build/libobs.pch.h"

extern const struct obs_source_info scene_info;
extern const struct obs_source_info group_info;

obs_scene_t *obs_scene_from_source(const obs_source_t *source)
{
	if (!source || strcmp(source->info.id, scene_info.id) != 0)
		return NULL;

	return source->context.data;
}

obs_scene_t *obs_group_from_source(const obs_source_t *source)
{
	if (!source || strcmp(source->info.id, group_info.id) != 0)
		return NULL;

	return source->context.data;
}
