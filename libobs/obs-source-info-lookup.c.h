#include "libobs/build/libobs.pch.h"

struct obs_source_info *get_source_info(const char *id)
{
	const struct obs_core *core = obs_core_texture_activity_core();
	for (size_t i = 0; i < core->source_types.num; i++) {
		struct obs_source_info *info = &core->source_types.array[i];
		if (strcmp(info->id, id) == 0)
			return info;
	}

	return NULL;
}
