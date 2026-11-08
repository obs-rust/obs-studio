#include "libobs/build/libobs.pch.h"

extern const struct obs_source_info group_info;
extern const struct obs_source_info scene_info;

bool obs_source_type_is_group(const char *id)
{
	return id && strcmp(id, group_info.id) == 0;
}

bool obs_source_type_is_scene(const char *id)
{
	return id && strcmp(id, scene_info.id) == 0;
}
