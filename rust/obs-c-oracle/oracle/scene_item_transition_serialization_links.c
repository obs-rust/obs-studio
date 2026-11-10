/* Test-only source collaborators for the scene serialization ABI shim.
 * Like source_transition_links.c, these satisfy COFF archive references in
 * obs-tests-core without linking libobs. Serialization parity uses its own
 * reference-counted collaborators; real source creation is covered by Tier 2.
 */
#include "obs-internal.h"

const char *obs_source_get_id(const obs_source_t *source)
{
	return source ? source->info.id : NULL;
}

obs_source_t *obs_source_create_private(const char *id, const char *name, obs_data_t *settings)
{
	/* The standalone category has no registered source factories. */
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(name);
	UNUSED_PARAMETER(settings);
	return NULL;
}
