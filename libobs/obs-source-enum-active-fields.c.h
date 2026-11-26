#include "libobs/build/libobs.pch.h"

/* What obs_source_enum_active_sources() and obs_source_enum_active_tree()
 * read and call on a source (the obs-source shims, #5729): its context
 * data, whether its type is a transition and has enum_active_sources, and
 * that callback itself. The caller rejects NULL and keeps the source alive. */
bool obs_source_enum_active_has_data(const obs_source_t *source);
bool obs_source_enum_active_has_data(const obs_source_t *source)
{
	return source->context.data != NULL;
}

bool obs_source_enum_active_is_transition(const obs_source_t *source);
bool obs_source_enum_active_is_transition(const obs_source_t *source)
{
	return source->info.type == OBS_SOURCE_TYPE_TRANSITION;
}

bool obs_source_enum_active_has_callback(const obs_source_t *source);
bool obs_source_enum_active_has_callback(const obs_source_t *source)
{
	return source->info.enum_active_sources != NULL;
}

void obs_source_enum_active_call(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param);
void obs_source_enum_active_call(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param)
{
	source->info.enum_active_sources(source->context.data, enum_callback, param);
}
