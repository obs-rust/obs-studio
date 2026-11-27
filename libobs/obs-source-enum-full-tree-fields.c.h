#include "libobs/build/libobs.pch.h"

/* What obs_source_enum_full_tree() and its hidden stop-source walk read and
 * call on a source (the obs-source shims, #5833): its context data, whether
 * its type is a transition and has enum_all_sources or enum_active_sources,
 * and those callbacks themselves. The caller rejects NULL and keeps the
 * source alive. */
bool obs_source_enum_full_tree_has_data(const obs_source_t *source);
bool obs_source_enum_full_tree_has_data(const obs_source_t *source)
{
	return source->context.data != NULL;
}

bool obs_source_enum_full_tree_is_transition(const obs_source_t *source);
bool obs_source_enum_full_tree_is_transition(const obs_source_t *source)
{
	return source->info.type == OBS_SOURCE_TYPE_TRANSITION;
}

bool obs_source_enum_full_tree_has_enum_all(const obs_source_t *source);
bool obs_source_enum_full_tree_has_enum_all(const obs_source_t *source)
{
	return source->info.enum_all_sources != NULL;
}

bool obs_source_enum_full_tree_has_enum_active(const obs_source_t *source);
bool obs_source_enum_full_tree_has_enum_active(const obs_source_t *source)
{
	return source->info.enum_active_sources != NULL;
}

void obs_source_enum_full_tree_call_enum_all(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param);
void obs_source_enum_full_tree_call_enum_all(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param)
{
	source->info.enum_all_sources(source->context.data, enum_callback, param);
}

void obs_source_enum_full_tree_call_enum_active(obs_source_t *source, obs_source_enum_proc_t enum_callback,
						void *param);
void obs_source_enum_full_tree_call_enum_active(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param)
{
	source->info.enum_active_sources(source->context.data, enum_callback, param);
}
