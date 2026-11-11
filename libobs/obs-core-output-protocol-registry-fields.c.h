#include "libobs/build/libobs.pch.h"

char *const *obs_core_output_protocol_registry_fields(size_t *count);
char *const *obs_core_output_protocol_registry_fields(size_t *count)
{
	/* Reuse the existing read-only core pointer accessor; no second global owner. */
	const struct obs_core *core = obs_core_texture_activity_core();
	*count = core->data.protocols.num;
	return core->data.protocols.array;
}
