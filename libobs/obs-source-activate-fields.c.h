#include "libobs/build/libobs.pch.h"

/* What obs_source_activate() and obs_source_deactivate() and their tree
 * callbacks change (the obs-source shim): a live source's show_refs and
 * activate_refs, each step one atomic increment, decrement or load, as the
 * original did it. The caller rejects a NULL source; the tree walk
 * (obs_source_enum_active_tree()) passes the children. */
void obs_source_activate_inc_show_refs(obs_source_t *source);
void obs_source_activate_inc_show_refs(obs_source_t *source)
{
	os_atomic_inc_long(&source->show_refs);
}

void obs_source_activate_dec_show_refs(obs_source_t *source);
void obs_source_activate_dec_show_refs(obs_source_t *source)
{
	os_atomic_dec_long(&source->show_refs);
}

long obs_source_activate_load_show_refs(const obs_source_t *source);
long obs_source_activate_load_show_refs(const obs_source_t *source)
{
	return os_atomic_load_long(&source->show_refs);
}

void obs_source_activate_inc_activate_refs(obs_source_t *source);
void obs_source_activate_inc_activate_refs(obs_source_t *source)
{
	os_atomic_inc_long(&source->activate_refs);
}

void obs_source_activate_dec_activate_refs(obs_source_t *source);
void obs_source_activate_dec_activate_refs(obs_source_t *source)
{
	os_atomic_dec_long(&source->activate_refs);
}

long obs_source_activate_load_activate_refs(const obs_source_t *source);
long obs_source_activate_load_activate_refs(const obs_source_t *source)
{
	return os_atomic_load_long(&source->activate_refs);
}
