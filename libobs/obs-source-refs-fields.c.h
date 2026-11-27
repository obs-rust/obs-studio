#include "libobs/build/libobs.pch.h"

/* The source and its control stay C-owned. obs-source's Rust
 * obs_source_addref(), obs_source_release(), obs_source_get_ref() and
 * obs_weak_source_get_source() (#5818) borrow the control's strong count and
 * reach the rest through these hidden adapters (no EXPORT). */

obs_weak_source_t *obs_source_refs_control(obs_source_t *source);
obs_weak_source_t *obs_source_refs_control(obs_source_t *source)
{
	return (obs_weak_source_t *)source->context.control;
}

volatile long *obs_weak_source_refs_field(obs_weak_source_t *weak);
volatile long *obs_weak_source_refs_field(obs_weak_source_t *weak)
{
	return &weak->ref.refs;
}

obs_source_t *obs_weak_source_source_field(const obs_weak_source_t *weak);
obs_source_t *obs_weak_source_source_field(const obs_weak_source_t *weak)
{
	return weak->source;
}

bool obs_source_refs_core_alive(void);
bool obs_source_refs_core_alive(void)
{
	return obs != NULL;
}

void obs_source_refs_destroy(obs_source_t *source);
void obs_source_refs_destroy(obs_source_t *source)
{
	obs_source_destroy(source);
}
