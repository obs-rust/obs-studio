#include "libobs/build/libobs.pch.h"

/* The parent's two counters obs_source_add_active_child() and
 * obs_source_remove_active_child() read (the obs-source shims). Other threads
 * change them atomically (#2081), so each is one atomic load, as the C did.
 * The caller rejects NULL and keeps the source alive. */
long obs_source_active_child_show_refs(const obs_source_t *source);
long obs_source_active_child_show_refs(const obs_source_t *source)
{
	return os_atomic_load_long(&source->show_refs);
}

long obs_source_active_child_activate_refs(const obs_source_t *source);
long obs_source_active_child_activate_refs(const obs_source_t *source)
{
	return os_atomic_load_long(&source->activate_refs);
}
