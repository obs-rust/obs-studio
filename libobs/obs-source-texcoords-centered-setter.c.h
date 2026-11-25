#include "libobs/build/libobs-ported-service-getters.pch.h"

void obs_source_set_texcoords_centered(obs_source_t *source, bool centered)
{
	source->texcoords_centered = centered;
}
