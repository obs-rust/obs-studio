#include "libobs/build/libobs.pch.h"

extern const struct obs_source_info group_info;
extern const struct obs_source_info scene_info;

bool obs_source_is_group(const obs_source_t *source)
{
	return source && strcmp(source->info.id, group_info.id) == 0;
}

bool obs_source_is_scene(const obs_source_t *source)
{
	return source && strcmp(source->info.id, scene_info.id) == 0;
}
