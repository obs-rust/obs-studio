#include "libobs/build/libobs-ported-service-getters.pch.h"

bool obs_source_get_texcoords_centered(obs_source_t *source)
{
	return source->texcoords_centered;
}

bool obs_source_is_hidden(obs_source_t *source)
{
	return source->temp_removed;
}
