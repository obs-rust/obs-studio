/* Original extracted C, renamed; real-header fixtures and production adapters. */
#include <stddef.h>
#include <stdlib.h>
#define obs_weak_object_addref oracle_obs_weak_object_addref
#define obs_weak_object_release oracle_obs_weak_object_release
#define obs_object_get_weak_object oracle_obs_object_get_weak_object
#define obs_weak_object_get_object oracle_obs_weak_object_get_object
#include "obs-weak-object-refs.c.h"
#undef obs_weak_object_addref
#undef obs_weak_object_release
#undef obs_object_get_weak_object
#undef obs_weak_object_get_object
#include "weak_object_refs_fields.inc"

struct weak_object_refs_fixture {
	struct obs_context_data object;
	struct obs_weak_object *weak;
};

void *oracle_weak_object_refs_new(long refs, long weak_refs);
void *oracle_weak_object_refs_new(long refs, long weak_refs)
{
	struct weak_object_refs_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		return NULL;
	f->weak = bzalloc(sizeof(*f->weak));
	f->weak->ref.refs = refs;
	f->weak->ref.weak_refs = weak_refs;
	f->weak->object = &f->object;
	f->object.control = f->weak;
	return f;
}
void *oracle_weak_object_refs_weak(void *handle);
void *oracle_weak_object_refs_weak(void *handle)
{
	return ((struct weak_object_refs_fixture *)handle)->weak;
}
void *oracle_weak_object_refs_object(void *handle);
void *oracle_weak_object_refs_object(void *handle)
{
	return &((struct weak_object_refs_fixture *)handle)->object;
}
void oracle_weak_object_refs_counts(void *handle, long *counts);
void oracle_weak_object_refs_counts(void *handle, long *counts)
{
	const struct weak_object_refs_fixture *f = handle;
	counts[0] = f->weak->ref.refs;
	counts[1] = f->weak->ref.weak_refs;
}
void oracle_weak_object_refs_free(void *handle, bool released);
void oracle_weak_object_refs_free(void *handle, bool released)
{
	struct weak_object_refs_fixture *f = handle;
	if (!released)
		bfree(f->weak);
	free(f);
}
void oracle_weak_object_refs_layout(size_t *layout);
void oracle_weak_object_refs_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_weak_object_refs_fields);
	layout[1] = _Alignof(struct obs_weak_object_refs_fields);
	layout[2] = offsetof(struct obs_weak_object_refs_fields, refs);
	layout[3] = offsetof(struct obs_weak_object_refs_fields, weak_refs);
	layout[4] = offsetof(struct obs_weak_object_refs_fields, object);
	layout[5] = sizeof(long);
	layout[6] = _Alignof(long);
}
bool oracle_weak_object_refs_addresses(void *handle);
bool oracle_weak_object_refs_addresses(void *handle)
{
	struct weak_object_refs_fixture *f = handle;
	struct obs_weak_object_refs_fields fields = obs_weak_object_refs_fields(f->weak);
	return fields.refs == &f->weak->ref.refs && fields.weak_refs == &f->weak->ref.weak_refs &&
	       fields.object == &f->object && obs_object_refs_control(&f->object) == f->weak;
}
