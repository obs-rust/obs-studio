#include "libobs/build/libobs.pch.h"

#define get_weak(source) ((obs_weak_source_t *)source->context.control)

void obs_source_addref(obs_source_t *source)
{
	if (!source)
		return;

	obs_ref_addref(&source->context.control->ref);
}

void obs_source_release(obs_source_t *source)
{
	if (!obs && source) {
		blog(LOG_WARNING, "Tried to release a source when the OBS "
				  "core is shut down!");
		return;
	}

	if (!source)
		return;

	obs_weak_source_t *control = get_weak(source);
	if (obs_ref_release(&control->ref)) {
		obs_source_destroy(source);
		obs_weak_source_release(control);
	}
}

obs_source_t *obs_source_get_ref(obs_source_t *source)
{
	if (!source)
		return NULL;

	return obs_weak_source_get_source(get_weak(source));
}

obs_source_t *obs_weak_source_get_source(obs_weak_source_t *weak)
{
	if (!weak)
		return NULL;

	if (obs_weak_ref_get_ref(&weak->ref))
		return weak->source;

	return NULL;
}

#undef get_weak
