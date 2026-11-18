#include "libobs/build/libobs.pch.h"

/* obs-source.c's private get_weak(), which this group includes later. */
#define get_weak(source) ((obs_weak_source_t *)source->context.control)

obs_weak_source_t *obs_source_get_weak_source(obs_source_t *source)
{
	if (!source)
		return NULL;

	obs_weak_source_t *weak = get_weak(source);
	obs_weak_source_addref(weak);
	return weak;
}

#undef get_weak
