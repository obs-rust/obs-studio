/* Original query, real layout, and production adapter: no stand-in structs. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_weak_object_expired oracle_obs_weak_object_expired
#include "obs-weak-object-expired.c.h"
#undef obs_weak_object_expired
#include "weak_object_expired_fields.inc"

struct weak_object_expired_fixture {
	struct obs_weak_object weak;
	struct obs_weak_object before;
	struct obs_context_data object;
};

void *oracle_weak_object_expired_new(long owners, long weak_refs, bool stored);
void *oracle_weak_object_expired_new(long owners, long weak_refs, bool stored)
{
	struct weak_object_expired_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->weak.ref.refs = owners;
		fixture->weak.ref.weak_refs = weak_refs;
		fixture->weak.object = stored ? &fixture->object : NULL;
		memcpy(&fixture->before, &fixture->weak, sizeof(fixture->weak));
	}
	return fixture;
}
bool oracle_weak_object_expired_unchanged(const void *handle);
bool oracle_weak_object_expired_unchanged(const void *handle)
{
	const struct weak_object_expired_fixture *fixture = handle;
	return memcmp(&fixture->weak, &fixture->before, sizeof(fixture->weak)) == 0;
}
void oracle_weak_object_expired_free(void *handle);
void oracle_weak_object_expired_free(void *handle)
{
	free(handle);
}
bool oracle_weak_object_expired_address(const obs_weak_object_t *weak);
bool oracle_weak_object_expired_address(const obs_weak_object_t *weak)
{
	return obs_weak_object_expired_field(weak) == &weak->ref.refs;
}
void oracle_weak_object_expired_layout(size_t *layout);
void oracle_weak_object_expired_layout(size_t *layout)
{
	layout[0] = sizeof(long);
	layout[1] = _Alignof(long);
}
void oracle_weak_object_expired_store(void *handle, long owners);
void oracle_weak_object_expired_store(void *handle, long owners)
{
	struct weak_object_expired_fixture *fixture = handle;
	os_atomic_set_long(&fixture->weak.ref.refs, owners);
	memcpy(&fixture->before, &fixture->weak, sizeof(fixture->weak));
}
