/* Original C operations over real private layouts; no replacement reference logic. */
#include <stdlib.h>
#include <string.h>
#define obs_weak_encoder_addref oracle_obs_weak_encoder_addref
#define obs_weak_encoder_release oracle_obs_weak_encoder_release
#define obs_encoder_get_weak_encoder oracle_obs_encoder_get_weak_encoder
#include "libobs/build/libobs.pch.h"
#include "encoder-weak-refs.inc"
#undef obs_weak_encoder_addref
#undef obs_weak_encoder_release
#undef obs_encoder_get_weak_encoder
#include "encoder_weak_refs_fields.inc"

struct encoder_weak_fixture {
	struct obs_encoder encoder;
	struct obs_encoder before;
	struct obs_weak_encoder *weak;
	struct obs_weak_encoder weak_before;
};

void *oracle_encoder_weak_new(long refs, long weak_refs, bool control, bool owner, unsigned char noise);
void *oracle_encoder_weak_new(long refs, long weak_refs, bool control, bool owner, unsigned char noise)
{
	struct encoder_weak_fixture *f = malloc(sizeof(*f));
	if (!f)
		return NULL;
	memset(&f->encoder, noise, sizeof(f->encoder));
	f->weak = bmalloc(sizeof(*f->weak));
	memset(f->weak, noise, sizeof(*f->weak));
	f->weak->ref.refs = refs;
	f->weak->ref.weak_refs = weak_refs;
	f->weak->encoder = owner ? &f->encoder : NULL;
	f->encoder.context.control = control ? (struct obs_weak_object *)f->weak : NULL;
	memcpy(&f->before, &f->encoder, sizeof(f->before));
	memcpy(&f->weak_before, f->weak, sizeof(f->weak_before));
	return f;
}
void *oracle_encoder_weak_handle(void *fixture);
void *oracle_encoder_weak_handle(void *fixture)
{
	return ((struct encoder_weak_fixture *)fixture)->weak;
}
void *oracle_encoder_weak_encoder(void *fixture);
void *oracle_encoder_weak_encoder(void *fixture)
{
	return &((struct encoder_weak_fixture *)fixture)->encoder;
}
long oracle_encoder_weak_count(void *fixture);
long oracle_encoder_weak_count(void *fixture)
{
	return ((struct encoder_weak_fixture *)fixture)->weak->ref.weak_refs;
}
bool oracle_encoder_weak_unchanged(void *fixture);
bool oracle_encoder_weak_unchanged(void *fixture)
{
	struct encoder_weak_fixture *f = fixture;
	f->weak_before.ref.weak_refs = f->weak->ref.weak_refs;
	return memcmp(&f->before, &f->encoder, sizeof(f->before)) == 0 &&
	       memcmp(&f->weak_before, f->weak, sizeof(f->weak_before)) == 0 &&
	       obs_encoder_weak_refs_count(f->weak) == &f->weak->ref.weak_refs &&
	       obs_encoder_weak_refs_control(&f->encoder) == (obs_weak_encoder_t *)f->encoder.context.control;
}
void oracle_encoder_weak_free(void *fixture, bool released);
void oracle_encoder_weak_free(void *fixture, bool released)
{
	struct encoder_weak_fixture *f = fixture;
	if (!released)
		bfree(f->weak);
	free(f);
}
void oracle_encoder_weak_layout(size_t *values);
void oracle_encoder_weak_layout(size_t *values)
{
	values[0] = sizeof(long);
	values[1] = _Alignof(long);
}
