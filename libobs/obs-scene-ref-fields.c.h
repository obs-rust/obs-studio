#include "libobs/build/libobs.pch.h"

bool obs_scene_ref_acquire_source(struct obs_source *source)
{
	return obs_source_get_ref(source) != NULL;
}

void obs_scene_ref_release_source(struct obs_source *source)
{
	obs_source_release(source);
}
