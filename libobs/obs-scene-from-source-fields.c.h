#include "libobs/build/libobs.pch.h"

const char *obs_scene_from_source_identifier(const obs_source_t *source);
const char *obs_scene_from_source_identifier(const obs_source_t *source)
{
	return source->info.id;
}

void *obs_scene_from_source_context(const obs_source_t *source);
void *obs_scene_from_source_context(const obs_source_t *source)
{
	return source->context.data;
}
