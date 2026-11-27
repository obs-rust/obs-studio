#include "libobs/build/libobs.pch.h"

/* What obs_source_remove() reads, writes, signals and calls (the obs-source
 * shim). The caller rejects a NULL source and passes the source it was given
 * (the removed check, the canvas and private reads) or the reference
 * obs_source_get_ref() returned for it (the rest); each call is one read,
 * one write or one libobs call. */
bool obs_source_remove_removed(const obs_source_t *source);
bool obs_source_remove_removed(const obs_source_t *source)
{
	return os_atomic_load_bool(&source->removed);
}

obs_source_t *obs_source_remove_get_ref(obs_source_t *source);
obs_source_t *obs_source_remove_get_ref(obs_source_t *source)
{
	return obs_source_get_ref(source);
}

void obs_source_remove_set_removed(obs_source_t *source);
void obs_source_remove_set_removed(obs_source_t *source)
{
	os_atomic_set_bool(&source->removed, true);
}

void obs_source_remove_signal(obs_source_t *source);
void obs_source_remove_signal(obs_source_t *source)
{
	obs_source_dosignal(source, "source_remove", "remove");
}

bool obs_source_remove_has_canvas(const obs_source_t *source);
bool obs_source_remove_has_canvas(const obs_source_t *source)
{
	return source->canvas != NULL;
}

bool obs_source_remove_private(const obs_source_t *source);
bool obs_source_remove_private(const obs_source_t *source)
{
	return source->context.private;
}

void obs_source_remove_from_canvas(obs_source_t *source);
void obs_source_remove_from_canvas(obs_source_t *source)
{
	obs_canvas_remove_source(source);
}

void obs_source_remove_release(obs_source_t *source);
void obs_source_remove_release(obs_source_t *source)
{
	obs_source_release(source);
}
