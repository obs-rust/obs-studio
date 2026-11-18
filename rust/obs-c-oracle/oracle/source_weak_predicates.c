/* Original predicates and real-layout fixtures. Sources remain allocated even
 * when the strong count is expired; identity never compares freed pointers. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_weak_source_expired oracle_obs_weak_source_expired
#define obs_weak_source_references_source oracle_obs_weak_source_references_source
#include "obs-internal.h"
#include "../legacy/obs-source-weak-predicates.c.h"
#undef obs_weak_source_references_source
#undef obs_weak_source_expired
#include "source_weak_predicates_fields.inc"

struct source_weak_predicates_fixture {
	struct obs_weak_source weak;
	struct obs_weak_source before;
	struct obs_source sources[2];
};

void *oracle_source_weak_predicates_new(long owners, long weak_owners, unsigned stored);
void *oracle_source_weak_predicates_new(long owners, long weak_owners, unsigned stored)
{
	struct source_weak_predicates_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		os_atomic_set_long(&fixture->weak.ref.refs, owners);
		os_atomic_set_long(&fixture->weak.ref.weak_refs, weak_owners);
		fixture->weak.source = stored ? &fixture->sources[stored - 1] : NULL;
		memcpy(&fixture->before, &fixture->weak, sizeof(fixture->weak));
	}
	return fixture;
}

void *oracle_source_weak_predicates_target(void *handle, unsigned target);
void *oracle_source_weak_predicates_target(void *handle, unsigned target)
{
	struct source_weak_predicates_fixture *fixture = handle;
	return target ? &fixture->sources[target - 1] : NULL;
}

bool oracle_source_weak_predicates_unchanged(const void *handle);
bool oracle_source_weak_predicates_unchanged(const void *handle)
{
	const struct source_weak_predicates_fixture *fixture = handle;
	return memcmp(&fixture->weak, &fixture->before, sizeof(fixture->weak)) == 0;
}

bool oracle_source_weak_predicates_addresses(const obs_weak_source_t *weak);
bool oracle_source_weak_predicates_addresses(const obs_weak_source_t *weak)
{
	return obs_source_weak_predicates_refs_field(weak) == &weak->ref.refs &&
	       obs_source_weak_predicates_source_field(weak) == weak->source;
}

void oracle_source_weak_predicates_long_layout(size_t *layout);
void oracle_source_weak_predicates_long_layout(size_t *layout)
{
	layout[0] = sizeof(long);
	layout[1] = _Alignof(long);
}

void oracle_source_weak_predicates_free(void *handle);
void oracle_source_weak_predicates_free(void *handle)
{
	free(handle);
}
