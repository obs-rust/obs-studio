/* Original query and discovered field adapter over the real private layout. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_weak_encoder_references_encoder oracle_obs_weak_encoder_references_encoder
#include "obs-weak-encoder-identity.c.h"
#undef obs_weak_encoder_references_encoder
#include "weak_encoder_identity_fields.inc"

struct weak_encoder_identity_fixture {
	struct obs_weak_encoder weak;
	struct obs_weak_encoder before;
	struct obs_encoder encoders[2];
};

void *oracle_weak_encoder_identity_new(unsigned stored);
void *oracle_weak_encoder_identity_new(unsigned stored)
{
	struct weak_encoder_identity_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->weak.encoder = stored ? &fixture->encoders[stored - 1] : NULL;
		memcpy(&fixture->before, &fixture->weak, sizeof(fixture->weak));
	}
	return fixture;
}
void *oracle_weak_encoder_identity_target(void *handle, unsigned target);
void *oracle_weak_encoder_identity_target(void *handle, unsigned target)
{
	struct weak_encoder_identity_fixture *fixture = handle;
	return target ? &fixture->encoders[target - 1] : NULL;
}
bool oracle_weak_encoder_identity_unchanged(const void *handle);
bool oracle_weak_encoder_identity_unchanged(const void *handle)
{
	const struct weak_encoder_identity_fixture *fixture = handle;
	return memcmp(&fixture->weak, &fixture->before, sizeof(fixture->weak)) == 0;
}
void oracle_weak_encoder_identity_free(void *handle);
void oracle_weak_encoder_identity_free(void *handle)
{
	free(handle);
}
bool oracle_weak_encoder_identity_addresses(const obs_weak_encoder_t *weak);
bool oracle_weak_encoder_identity_addresses(const obs_weak_encoder_t *weak)
{
	return obs_weak_encoder_identity_fields(weak).encoder == &weak->encoder;
}
void oracle_weak_encoder_identity_layout(size_t *layout);
void oracle_weak_encoder_identity_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_weak_encoder_identity_fields);
	layout[1] = _Alignof(struct obs_weak_encoder_identity_fields);
	layout[2] = offsetof(struct obs_weak_encoder_identity_fields, encoder);
}
